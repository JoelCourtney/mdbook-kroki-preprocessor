//! An mdbook preprocessor that lets you embed diagrams from any of [Kroki's](https://kroki.io)
//! endpoints into your book.

use anyhow::{Result, anyhow, Context};
use mdbook::preprocess::{Preprocessor, PreprocessorContext};
use mdbook::book::{Book, BookItem, Chapter};
use serde::Serialize;
use std::sync::Mutex;
use std::sync::Arc;
use pulldown_cmark::{Parser, CowStr, Tag, LinkType, Event, CodeBlockKind};
use pulldown_cmark_to_cmark::cmark;
use std::path::PathBuf;

fn main() {
    mdbook_preprocessor_boilerplate::run(
        KrokiPreprocessor,
        "An mdbook preprocessor for rendering kroki diagrams"
    );
}

pub struct KrokiPreprocessor;

impl Preprocessor for KrokiPreprocessor {
    fn name(&self) -> &'static str {
        "kroki-preprocessor"
    }

    fn run(&self, ctx: &PreprocessorContext, book: Book) -> Result<Book> {
        let src = &ctx.config.book.src;

        let book = Arc::new(Mutex::new(book));
        let mut book_lock = book.lock().map_err(|_| anyhow!("cound not lock book in run"))?;

        let mut index_stack = Vec::new();
        let diagrams = extract_diagrams(&mut book_lock.sections, &mut index_stack)?;

        std::mem::drop(book_lock);

        let runtime = tokio::runtime::Runtime::new()?;
        runtime.block_on(async {
            let results = futures::future::join_all(
                diagrams.into_iter().map(|diag| diag.resolve(book.clone(), src))
            ).await;
            for result in results {
                result?;
            }
            Ok(()) as Result<()>
        })?;

        Ok(Arc::try_unwrap(book).map_err(|_| anyhow!("failed to unwrap arc"))?.into_inner()?)
    }

    fn supports_renderer(&self, renderer: &str) -> bool {
        renderer == "html"
    }
}

fn extract_diagrams<'a>(items: impl IntoIterator<Item=&'a mut BookItem> + 'a, indices: &mut Vec<usize>) -> Result<Vec<Diagram>> {
    let mut diagrams = Vec::new();
    indices.push(0);
    for (index, item) in items.into_iter().enumerate() {
        if let BookItem::Chapter(ref mut chapter) = item {
            *indices.last_mut().unwrap() = index;
            diagrams.extend(
                parse_and_replace(chapter, &indices)?
            );
            diagrams.extend(extract_diagrams(&mut chapter.sub_items, indices)?);
        }
    }
    indices.pop();
    Ok(diagrams)
}

#[derive(PartialEq,Eq)]
enum ParserState {
    InImage,
    InCode(String),
    InPre,
    Out
}

#[derive(Debug)]
struct Diagram {
    diagram_type: String,
    replace_text: String,
    indices: Vec<usize>,
    content: String,
    is_path: bool
}

impl Diagram {
    async fn resolve(self, book: Arc<Mutex<Book>>, src: &PathBuf) -> Result<()> {
        let request_body = KrokiRequestBody {
            diagram_source: if self.is_path {
                let mut path = src.clone();
                let mut book_lock = book.lock().map_err(|_| anyhow!("could not lock book"))?;
                let chapter = get_chapter(&mut book_lock.sections, &self.indices)?;
                path.push(chapter.source_path.clone().ok_or(anyhow!("no path for chapter"))?);
                std::mem::drop(book_lock);
                path.pop();
                path.push(self.content);
                std::fs::read_to_string(path.clone()).context(format!("attempting to read: {:?}", path))?
            } else {
                self.content
            },
            diagram_type: self.diagram_type,
            output_format: "svg"
        };
        let svg = get_svg(request_body).await?;
        let mut book_lock = book.lock().map_err(|_| anyhow!("could not lock book"))?;
        let chapter = get_chapter(&mut book_lock.sections, &self.indices)?;
        chapter.content = chapter.content.replace(&self.replace_text, &svg).to_string();

        Ok(())
    }
}

fn get_chapter<'a>(mut items: &'a mut Vec<BookItem>, indices: &Vec<usize>) -> Result<&'a mut Chapter> {
    for index in &indices[..indices.len()-1] {
        let item = items.into_iter().nth(*index).ok_or(anyhow!("index disappeared"))?;
        match item {
            BookItem::Chapter(ref mut chapter) => items = &mut chapter.sub_items,
            _ => return Err(anyhow!("wasn't a chapter"))
        }
    }
    match items.into_iter().nth(*indices.last().unwrap()).ok_or(anyhow!("chapter not found"))? {
        BookItem::Chapter(ref mut chapter) => Ok(chapter),
        _ => Err(anyhow!("wasn't a chapter"))
    }
}

async fn get_svg(request_body: KrokiRequestBody) -> Result<String> {
    let client = reqwest::Client::new();
    let mut result = client.post("https://kroki.io/")
        .body(serde_json::to_string(&request_body)?)
        .send().await?.text().await?;
    let start_index = result.find("<svg").ok_or(anyhow!("didn't find <svg"))?;
    result.replace_range(..start_index, "");
    result.insert_str(0, "<pre>");
    result.push_str("</pre>");
    Ok(result)
}

#[derive(Serialize, Debug)]
struct KrokiRequestBody {
    diagram_source: String,
    diagram_type: String,
    output_format: &'static str
}

fn parse_and_replace(chapter: &mut Chapter, indices: &Vec<usize>) -> Result<Vec<Diagram>> {
    let text = &mut chapter.content;

    let mut buffer = String::with_capacity(text.len());

    let mut state = ParserState::Out;

    let mut diagrams = Vec::new();

    let events = Parser::new(text).map(|e| {
        match e {
            Event::Html(ref tag) if tag.as_ref() == "<pre>" => {
                state = ParserState::InPre;
                e
            },
            Event::Html(ref tag) if tag.as_ref() == "</pre>" => {
                state = ParserState::Out;
                e
            },
            Event::Start(Tag::Image(LinkType::Inline, ref url, _)) => {
                if url.starts_with("kroki-") {
                    let colon_index = url.find("://").expect("didn't find :// after kroki");
                    let diagram_type = &url[6..colon_index];
                    let path = &url[colon_index+3..];

                    state = ParserState::InImage;
                    diagrams.push(Diagram {
                        diagram_type: diagram_type.to_string().to_lowercase(),
                        replace_text: format!("%%kroki-diagram-{}%%", diagrams.len()),
                        indices: indices.clone(),
                        content: path.to_string(),
                        is_path: true
                    });
                    Event::Start(Tag::Paragraph)
                } else {
                    e
                }
            }
            Event::Text(_) if state == ParserState::InImage => {
                Event::Text(CowStr::Boxed(format!("%%kroki-diagram-{}%%", diagrams.len() - 1).into_boxed_str()))
            }
            Event::End(Tag::Image(..)) if state == ParserState::InImage => {
                state = ParserState::Out;
                Event::End(Tag::Paragraph)
            }
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(ref lang))) if state != ParserState::InPre => {
                if lang.starts_with("kroki-") {
                    let diagram_type = &lang[6..];
                    state = ParserState::InCode(diagram_type.to_string());
                    Event::Start(Tag::Paragraph)
                } else {
                    e
                }
            }
            Event::Text(content) => {
                match state {
                    ParserState::InCode(ref diagram_type) => {
                        let replace_text = format!("%%kroki-diagram-{}%%", diagrams.len());
                        diagrams.push(Diagram {
                            diagram_type: diagram_type.clone().to_lowercase(),
                            replace_text: replace_text.clone(),
                            indices: indices.clone(),
                            content: content.to_string(),
                            is_path: false
                        });
                        Event::Text(CowStr::Boxed(replace_text.into_boxed_str()))
                    }
                    _ => Event::Text(content)
                }
            }
            Event::End(Tag::CodeBlock(..)) if matches!(state, ParserState::InCode(..)) => {
                state = ParserState::Out;
                Event::End(Tag::Paragraph)
            }
            e => e
        }
    });

    cmark(events, &mut buffer)?;

    *text = buffer;
    Ok(diagrams)
}





