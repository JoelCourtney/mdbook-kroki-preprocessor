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

use anyhow::{anyhow, bail, Result};
use diagram::Diagram;
use mdbook::book::{Book, BookItem, Chapter};
use mdbook::preprocess::{Preprocessor, PreprocessorContext};
use pulldown_cmark::{CodeBlockKind, CowStr, Event, LinkType, Options, Parser, Tag};
use pulldown_cmark_to_cmark::cmark;
use std::sync::Arc;
use tokio::sync::Mutex;

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
        let endpoint = if let Some(config) = ctx.config.get_preprocessor(self.name()) {
            match config.get("endpoint") {
                Some(v) => {
                    if let Some(s) = v.as_str() {
                        let mut url = s.to_string();
                        if !url.ends_with('/') {
                            url.push('/');
                        }
                        url
                    } else {
                        bail!("endpoint must be a string")
                    }
                }
                None => "https://kroki.io/".to_string(),
            }
        } else {
            "https://kroki.io/".to_string()
        };
        let src = &ctx.config.book.src;

        let mut index_stack = Vec::new();
        let diagrams = extract_diagrams(&mut book.sections, &mut index_stack)?;

        let book = Arc::new(Mutex::new(book));

        let runtime = tokio::runtime::Runtime::new()?;
        runtime.block_on(async {
            let results = futures::future::join_all(
                diagrams
                    .into_iter()
                    .map(|diagram| diagram.resolve(book.clone(), src, &endpoint)),
            )
            .await;
            for result in results {
                result?;
            }
            Ok(()) as Result<()>
        })?;

        Ok(Arc::try_unwrap(book)
            .map_err(|_| anyhow!("failed to unwrap arc"))?
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
                    state = ParserState::InPre;
                    e
                }
                Event::Html(ref tag) if tag.as_ref() == "</pre>" => {
                    state = ParserState::Out;
                    e
                }
                Event::Start(Tag::Image(LinkType::Inline, ref url, _)) => {
                    if url.starts_with("kroki-") {
                        if let Some(colon_index) = url.find(':') {
                            let diagram_type = &url[6..colon_index];
                            let path = &url[colon_index + 1..];

                            state = ParserState::InImage;
                            diagrams.push(Diagram {
                                diagram_type: diagram_type.to_string().to_lowercase(),
                                replace_text: format!("%%kroki-diagram-{}%%", diagrams.len()),
                                indices: indices.to_vec(),
                                content: path.to_string(),
                                is_path: true,
                            });
                            Event::Start(Tag::Paragraph)
                        } else {
                            e
                        }
                    } else {
                        e
                    }
                }
                Event::Text(_) if state == ParserState::InImage => Event::Text(CowStr::Boxed(
                    format!("%%kroki-diagram-{}%%", diagrams.len() - 1).into_boxed_str(),
                )),
                Event::End(Tag::Image(..)) if state == ParserState::InImage => {
                    state = ParserState::Out;
                    Event::End(Tag::Paragraph)
                }
                Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(ref lang)))
                    if state != ParserState::InPre =>
                {
                    if lang.starts_with("kroki-") {
                        let diagram_type = &lang[6..];
                        state = ParserState::InCode(diagram_type.to_string());
                        Event::Start(Tag::Paragraph)
                    } else {
                        e
                    }
                }
                Event::Text(content) => match state {
                    ParserState::InCode(ref diagram_type) => {
                        let replace_text = format!("%%kroki-diagram-{}%%", diagrams.len());
                        diagrams.push(Diagram {
                            diagram_type: diagram_type.clone().to_lowercase(),
                            replace_text: replace_text.clone(),
                            indices: indices.to_vec(),
                            content: content.to_string(),
                            is_path: false,
                        });
                        Event::Text(CowStr::Boxed(replace_text.into_boxed_str()))
                    }
                    _ => Event::Text(content),
                },
                Event::End(Tag::CodeBlock(..)) if matches!(state, ParserState::InCode(..)) => {
                    state = ParserState::Out;
                    Event::End(Tag::Paragraph)
                }
                e => e,
            })
        })
        .collect::<Result<Vec<Event>>>()?;

    cmark(events.iter(), &mut buffer)?;

    *text = buffer;
    Ok(diagrams)
}

#[derive(PartialEq, Eq)]
enum ParserState {
    InImage,
    InCode(String),
    InPre,
    Out,
}
