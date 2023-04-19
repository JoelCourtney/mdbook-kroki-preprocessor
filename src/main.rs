//! An mdbook preprocessor that lets you embed diagrams from any of [Kroki's](https://kroki.io)
//! diagram types into your book.
//! 
//! # Setup
//! 
//! First install this preprocessor with `cargo install mdbook-kroki-preprocessor`.
//! 
//! Then add the preprocessor to your `book.toml`:
//! 
//! ```toml
//! [book]
//! authors = ["You"]
//! language = "en"
//! multilingual = false
//! src = "src"
//! title = "example"
//!
//! [preprocessor.kroki-preprocessor]
//! ```
//! 
//! # Usage
//! 
//! There are two ways to use Kroki in your book. First is a fenced code block:
//! 
//! ``````markdown
//! ```kroki-mermaid
//! graph TD
//!   A[ Anyone ] -->|Can help | B( Go to github.com/yuzutech/kroki )
//!   B --> C{ How to contribute? }
//!   C --> D[ Reporting bugs ]
//!   C --> E[ Sharing ideas ]
//!   C --> F[ Advocating ]
//! ```
//! ``````
//! 
//! The code block's language has to be `kroki-<diagram type>`.
//! 
//! The other method is to use an image tag, for diagrams contents that are too big to put inline
//! in the markdown (such as for excalidraw):
//! 
//! ```markdown
//! ![Excalidraw example](kroki-excalidraw:example.excalidraw)
//! ```
//! 
//! The title field can be anything, but the source field needs to start with `kroki-<diagram type>:`.
//! Both relative and absolute paths are supported. Relative paths are relative to the current markdown
//! source file, *not* the root of the mdbook.
//! 
//! The preprocessor will collect all Kroki diagrams of both types, send requests out in parallel
//! to the appropriate Kroki API endpoint, and replace their SVG contents back into the markdown.
//! 
//! # Endpoint Configuration
//! 
//! If you'd like to use a self-managed instance of Kroki, you can configure the preprocessor to
//! use a different endpoint:
//! 
//! ```toml
//! [preprocessor.kroki-preprocessor]
//! endpoint = "http://localhost:8000"
//! ```
//! 
//! The preprocessor will add a trailing slash if needed. The default is "https://kroki.io/".
//! 
//! # Other
//! 
//! This preprocessor only supports HTML rendering.

mod diagram;

use std::path::PathBuf;
use anyhow::{Result, anyhow, bail};
use mdbook::preprocess::{Preprocessor, PreprocessorContext};
use mdbook::book::{Book, BookItem, Chapter};
use std::sync::Arc;
use pulldown_cmark::{Parser, CowStr, Tag, LinkType, Event, CodeBlockKind, Options};
use pulldown_cmark_to_cmark::cmark;
use tokio::sync::Mutex;
use diagram::Diagram;
use crate::diagram::DiagramFormat;

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

    fn run(&self, ctx: &PreprocessorContext, mut book: Book) -> Result<Book> {
        let endpoint = if let Some(config) = ctx.config.get_preprocessor(self.name()) {
            match config.get("endpoint") {
                Some(toml::value::Value::String(value)) => {
                    let mut url = value.clone();
                    if !url.ends_with("/") {
                        url.push_str("/");
                    }
                    url
                }
                None => "https://kroki.io/".to_string(),
                Some(_) => bail!("endpoint must be a string")
            }
        } else {
            "https://kroki.io/".to_string()
        };
        let src = &ctx.config.book.src;

        let diagrams = extract_diagrams(&mut book.sections, ctx.config.build.build_dir.clone())?;

        let book = Arc::new(Mutex::new(book));

        let runtime = tokio::runtime::Runtime::new()?;
        runtime.block_on(async {
            let results = futures::future::join_all(
                diagrams.into_iter().map(|diagram| diagram.resolve(src, &endpoint))
            ).await;
            for result in results {
                result?;
            }
            Ok(()) as Result<()>
        })?;

        Ok(Arc::try_unwrap(book).map_err(|_| anyhow!("failed to unwrap arc"))?.into_inner())
    }

    fn supports_renderer(&self, renderer: &str) -> bool {
        renderer == "html"
    }
}

/// Recursively scans all chapters for diagrams.
/// 
/// Uses `parse_and_replace` to pull out the diagrams.
fn extract_diagrams<'a>(items: impl IntoIterator<Item=&'a mut BookItem> + 'a, build_dir: PathBuf) -> Result<Vec<Diagram>> {
    let mut diagrams = Vec::new();
    for (index, item) in items.into_iter().enumerate() {
        if let BookItem::Chapter(ref mut chapter) = item {
            diagrams.extend(
                parse_and_replace(chapter, build_dir.clone())?
            );
            diagrams.extend(extract_diagrams(&mut chapter.sub_items, build_dir.clone())?);
        }
    }
    Ok(diagrams)
}

/// Listens on the cmark pulldown parser and replaces kroki diagrams
/// in the text with "%%kroki-diagram-N%%", which will be replaced again
/// later when the diagram is rendered.
fn parse_and_replace(chapter: &mut Chapter, build_dir: PathBuf) -> Result<Vec<Diagram>> {
    let text = &mut chapter.content;

    let mut buffer = String::with_capacity(text.len());

    let mut state = ParserState::Out;

    let mut diagrams = Vec::new();

    let events = Parser::new_ext(text, Options::all()).map(|e| {
        Ok(match e {
            Event::Html(ref tag) if tag.as_ref() == "<pre>" => {
                state = ParserState::InPre;
                e
            },
            Event::Html(ref tag) if tag.as_ref() == "</pre>" => {
                state = ParserState::Out;
                e
            },
            Event::Start(Tag::Image(LinkType::Inline, ref url, ref title)) => {
                if url.starts_with("kroki-") {
                    if let Some(colon_index) = url.find(":") {
                        let diagram_type = &url[6..colon_index];
                        let path = &url[colon_index+1..];

                        let diagram_filename = format!("kroki-diagram-{}.svg", diagrams.len());

                        state = ParserState::InImage(diagram_filename.clone());

                        let mut output_path = dbg!(build_dir.clone());
                        output_path.push(chapter.path.clone().unwrap().with_file_name(diagram_filename.clone()));
                        diagrams.push(Diagram {
                            diagram_type: diagram_type.to_string().to_lowercase(),
                            diagram_format: DiagramFormat::default(),
                            output_path,
                            chapter_path: chapter.source_path.clone().unwrap(),
                            content: path.to_string(),
                            is_path: true
                        });
                        Event::Start(Tag::Image(LinkType::Inline, CowStr::from(diagram_filename), title.clone()))
                    } else {
                        e
                    }
                } else {
                    e
                }
            }
            Event::End(Tag::Image(..)) => {
                let result = match state {
                    ParserState::InImage(ref diagram_filename) => {
                        Event::End(Tag::Image(LinkType::Inline, CowStr::from(diagram_filename.clone()), CowStr::from("")))
                    }
                    _ => e
                };
                if let ParserState::InCode(..) = state {
                    state = ParserState::Out;
                }
                result
            }
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(ref lang))) if state != ParserState::InPre => {
                if lang.starts_with("kroki-") {
                    let diagram_type = &lang[6..];
                    let diagram_filename = format!("kroki-diagram-{}.svg", diagrams.len());
                    let mut output_path = build_dir.clone();
                    output_path.push(chapter.path.clone().unwrap().with_file_name(diagram_filename.clone()));
                    dbg!(&output_path);
                    state = ParserState::InCode(diagram_type.to_string(), diagram_filename, output_path);
                    Event::Start(Tag::Image(LinkType::Inline, CowStr::from(""), CowStr::from("")))
                } else {
                    e
                }
            }
            Event::Text(content) => {
                match state {
                    ParserState::InCode(ref diagram_type, _, ref output_path) => {
                        diagrams.push(Diagram {
                            diagram_type: diagram_type.clone().to_lowercase(),
                            diagram_format: DiagramFormat::default(),
                            output_path: output_path.clone(),
                            chapter_path: chapter.source_path.clone().unwrap(),
                            content: content.to_string(),
                            is_path: false,
                        });
                        Event::Text(CowStr::from(""))
                    }
                    _ => Event::Text(content)
                }
            }
            e@Event::End(Tag::CodeBlock(..)) => {
                let result = match state {
                    ParserState::InCode(_, ref diagram_filename, _) => {
                        Event::End(Tag::Image(LinkType::Inline, CowStr::from(diagram_filename.clone()), CowStr::from("")))
                    }
                    _ => e
                };
                if let ParserState::InCode(..) = state {
                    state = ParserState::Out;
                }
                result
            }
            e => e
        }
    )}).collect::<Result<Vec<Event>>>()?;

    cmark(events.iter(), &mut buffer)?;

    *text = buffer;
    Ok(diagrams)
}

#[derive(PartialEq,Eq)]
enum ParserState {
    InImage(String),
    InCode(String, String, PathBuf),
    InPre,
    Out
}
