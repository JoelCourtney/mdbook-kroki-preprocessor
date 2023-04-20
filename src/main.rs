#![doc = include_str!("../README.md")]

mod diagram;

use crate::diagram::{DiagramContent, PathRoot};
use anyhow::Context;
use anyhow::{anyhow, bail, Result};
use diagram::Diagram;
use mdbook::book::{Book, BookItem, Chapter};
use mdbook::preprocess::{Preprocessor, PreprocessorContext};
use pulldown_cmark::{CodeBlockKind, CowStr, Event, LinkType, Options, Parser, Tag};
use pulldown_cmark_to_cmark::cmark;
use sscanf::sscanf;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use xmltree::Element;

fn main() {
    mdbook_preprocessor_boilerplate::run(
        KrokiPreprocessor,
        "An mdbook preprocessor for rendering kroki diagrams",
    );
}

pub struct KrokiPreprocessor;

impl Preprocessor for KrokiPreprocessor {
    fn name(&self) -> &'static str {
        "kroki-preprocessor"
    }

    fn run(&self, ctx: &PreprocessorContext, mut book: Book) -> Result<Book> {
        let endpoint = if let Some(v) = ctx
            .config
            .get_preprocessor(self.name())
            .and_then(|config| config.get("endpoint"))
        {
            if let Some(s) = v.as_str() {
                let mut url = s.to_string();
                if !url.ends_with('/') {
                    url.push('/');
                }
                url
            } else {
                bail!("endpoint must be a string")
            }
        } else {
            "https://kroki.io/".to_string()
        };

        let src = &ctx.config.book.src;

        let mut index_stack = Vec::new();
        let diagrams = extract_diagrams(&mut book.sections, &mut index_stack)?;

        let book = Arc::new(Mutex::new(book));

        tokio::runtime::Runtime::new()
            .expect("tokio runtime")
            .block_on(async {
                futures::future::try_join_all(
                    diagrams
                        .into_iter()
                        .map(|diagram| diagram.resolve(ctx, book.clone(), src, &endpoint)),
                )
                .await?;
                Ok(()) as Result<()>
            })?;

        Ok(Arc::try_unwrap(book)
            .map_err(|_| anyhow!("failed to unwrap arc"))
            .expect("book arc should only have one reference at end")
            .into_inner())
    }

    fn supports_renderer(&self, renderer: &str) -> bool {
        renderer == "html"
    }
}

/// Recursively scans all chapters for diagrams.
///
/// Uses `parse_and_replace` to pull out the diagrams.
fn extract_diagrams<'a>(
    items: impl IntoIterator<Item = &'a mut BookItem> + 'a,
    indices: &mut Vec<usize>,
) -> Result<Vec<Diagram>> {
    let mut diagrams = Vec::new();
    indices.push(0);
    for (index, item) in items.into_iter().enumerate() {
        if let BookItem::Chapter(ref mut chapter) = item {
            *indices.last_mut().unwrap() = index;
            diagrams.extend(parse_and_replace(chapter, indices)?);
            diagrams.extend(extract_diagrams(&mut chapter.sub_items, indices)?);
        }
    }
    indices.pop();
    Ok(diagrams)
}

/// Listens on the cmark pulldown parser and replaces kroki diagrams
/// in the text with "%%kroki-diagram-N%%", which will be replaced again
/// later when the diagram is rendered.
fn parse_and_replace(chapter: &mut Chapter, indices: &[usize]) -> Result<Vec<Diagram>> {
    let text = &mut chapter.content;

    let mut buffer = String::with_capacity(text.len());

    let mut state = ParserState::Out;

    let mut diagrams = Vec::new();

    let events = Parser::new_ext(text, Options::all())
        .map(|e| {
            Ok(match e {
                Event::Html(ref tag) if tag.as_ref() == "<pre>" => {
                    state = match state {
                        ParserState::InPre(n) => ParserState::InPre(n+1),
                        _ => ParserState::InPre(1)
                    };
                    vec![e]
                }
                Event::Html(ref tag) if tag.as_ref() == "</pre>" => {
                    match &state {
                        ParserState::InPre(n@2..) => { state = ParserState::InPre(n-1) }
                        ParserState::InPre(1) => { state = ParserState::Out }
                        _ => {}
                    };
                    vec![e]
                }
                _ if matches!(state, ParserState::InPre(_)) => vec![e],
                Event::Html(ref tag) if tag.as_ref().starts_with("<kroki") => {
                    let xml = if !tag.contains("/>") {
                        state = ParserState::InKrokiTag;
                        tag.to_string() + "</kroki>"
                    } else {
                        tag.to_string()
                    };
                    let element = Element::parse(xml.as_bytes())?;
                    let mut path: PathBuf = element.attributes.get("path")
                        .ok_or(anyhow!("src tag required"))?.parse()?;
                    let path_root = match element.attributes.get("root").map(|s| s.as_str()) {
                        Some("system") => {
                            if path.is_relative() {
                                bail!("cannot use relative path with root=\"system\"");
                            }
                            PathRoot::System
                        },
                        Some("book") => {
                            if path.is_absolute() {
                                path = path.strip_prefix("/")?.into();
                            }
                            PathRoot::Book
                        },
                        Some("source" | "src") => {
                            if path.is_absolute() {
                                path = path.strip_prefix("/")?.into();
                            }
                            PathRoot::Source
                        },
                        None | Some("this" | ".") => {
                            if path.is_absolute() {
                                bail!(r#"cannot use absolute path without setting `root` attribute to "system", "book", or "source""#);
                            }
                            PathRoot::This
                        }
                        Some(other) => bail!("unrecognized root type: {other}")
                    };
                    let diagram_type = element.attributes.get("type").ok_or(anyhow!("missing type tag"))?.clone();
                    let replace_text = format!("%%kroki-diagram-{}%%", diagrams.len());
                    diagrams.push(Diagram {
                        diagram_type,
                        output_format: "svg".to_string(),
                        replace_text: replace_text.clone(),
                        indices: indices.to_vec(),
                        content: DiagramContent::Path {
                            kind: path_root,
                            path
                        }
                    });
                    vec![Event::Text(CowStr::Boxed(replace_text.into_boxed_str()))]
                }
                Event::Html(ref tag) if tag.contains("</kroki>") => {
                    state = ParserState::Out;
                    vec![]
                }
                Event::Start(Tag::Image(LinkType::Inline, ref url, _)) => {
                    if let Ok((diagram_type, path)) = sscanf!(url, "kroki-{str}:{PathBuf}") {
                        state = ParserState::InImage;
                        diagrams.push(Diagram {
                            diagram_type: diagram_type.to_lowercase(),
                            output_format: "svg".to_string(),
                            replace_text: format!("%%kroki-diagram-{}%%", diagrams.len()),
                            indices: indices.to_vec(),
                            content: DiagramContent::Path {
                                kind: if path.is_absolute() { PathRoot::System } else { PathRoot::This },
                                path
                            },
                        });
                        vec![Event::Start(Tag::Paragraph)]
                    } else {
                        vec![e]
                    }
                }
                Event::Text(_) if state == ParserState::InImage => vec![Event::Text(CowStr::Boxed(
                    format!("%%kroki-diagram-{}%%", diagrams.len() - 1).into_boxed_str(),
                ))],
                Event::End(Tag::Image(..)) if state == ParserState::InImage => {
                    state = ParserState::Out;
                    vec![Event::End(Tag::Paragraph)]
                }
                Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(ref lang))) => {
                    if let Ok(diagram_type) = sscanf!(lang, "kroki-{String}") {
                        state = ParserState::InCode(diagram_type);
                        vec![Event::Start(Tag::Paragraph)]
                    } else {
                        vec![e]
                    }
                }
                Event::Text(content) => match state {
                    ParserState::InCode(ref diagram_type) => {
                        let replace_text = format!("%%kroki-diagram-{}%%", diagrams.len());
                        diagrams.push(Diagram {
                            diagram_type: diagram_type.clone().to_lowercase(),
                            output_format: "svg".to_string(),
                            replace_text: replace_text.clone(),
                            indices: indices.to_vec(),
                            content: DiagramContent::Raw(content.to_string())
                        });
                        vec![Event::Text(CowStr::Boxed(replace_text.into_boxed_str()))]
                    }
                    _ => vec![Event::Text(content)],
                },
                Event::End(Tag::CodeBlock(..)) if matches!(state, ParserState::InCode(_)) => {
                    state = ParserState::Out;
                    vec![Event::End(Tag::Paragraph)]
                }
                e => vec![e],
            })
        })
        .collect::<Result<Vec<Vec<Event>>>>()
        .with_context(|| format!("error occurred while processing chapter {} ({:?})", chapter.name, chapter.source_path))?
        .into_iter()
        .flatten();

    cmark(events, &mut buffer)?;

    *text = buffer;
    Ok(diagrams)
}

#[derive(PartialEq, Eq)]
enum ParserState {
    InImage,
    InKrokiTag,
    InCode(String),
    InPre(usize),
    Out,
}
