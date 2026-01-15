#![doc = include_str!("../README.md")]

use anyhow::{Result, anyhow, bail};
use futures::Future;
use md_kroki::MdKroki;
use mdbook_preprocessor::book::{Book, BookItem, Chapter};
use mdbook_preprocessor::{Preprocessor, PreprocessorContext};
use std::path::PathBuf;
use std::pin::Pin;

const NAME: &str = "kroki-preprocessor";

fn main() -> Result<()> {
    mdbook_preprocessor_boilerplate::run(
        KrokiPreprocessor,
        "An mdbook preprocessor for rendering kroki diagrams",
    )
}

pub struct KrokiPreprocessor;

impl Preprocessor for KrokiPreprocessor {
    fn name(&self) -> &'static str {
        NAME
    }

    fn run(&self, ctx: &PreprocessorContext, mut book: Book) -> Result<Book> {
        let endpoint = if let Some(mut url) = ctx
            .config
            .get::<String>(&format!("preprocessor.{NAME}.endpoint"))?
        {
            if !url.ends_with('/') {
                url.push('/');
            }
            url
        } else {
            "https://kroki.io/".to_string()
        };

        let source_root = &ctx.config.book.src;
        let book_root = ctx.root.clone();

        let renderer_factory = move |chapter_path: Option<PathBuf>| {
            let source_root = source_root.clone();
            let book_root = book_root.clone();
            let chapter_parent_path = chapter_path.map(|mut p| {
                p.pop();
                p
            });
            MdKroki::builder()
            .endpoint(endpoint.clone())
            .path_and_root_resolver(move |mut path, root: Option<&str>| {
                let full_path = match root {
                    Some("system") => {
                        if path.is_relative() {
                            bail!("cannot use relative path with root=\"system\"");
                        }
                        path
                    }
                    Some("book") => {
                        if path.is_absolute() {
                            path = path.strip_prefix("/")?.into();
                        }
                        book_root.join(path)
                    }
                    Some("source" | "src") => {
                        if path.is_absolute() {
                            path = path.strip_prefix("/")?.into();
                        }
                        book_root.join(&source_root).join(path)
                    }
                    None | Some("this" | ".") => {
                        if path.is_absolute() {
                            bail!(r#"cannot use absolute path without setting `root` attribute to "system", "book", or "source""#);
                        }
                        book_root
                            .join(&source_root)
                            .join(
                            chapter_parent_path.as_deref().ok_or_else(|| anyhow!("cannot use local relative file references in chapters with no source path."))?
                            )
                            .join(path)
                    }
                    Some(other) => bail!("unrecognized root type: {other}")
                };

                Ok(std::fs::read_to_string(full_path)?)
            })
            .build()
        };

        let mut index_stack = vec![];
        let render_futures =
            extract_render_futures(&mut book.items, &mut index_stack, &renderer_factory);

        let rendered_files = tokio::runtime::Runtime::new()
            .expect("tokio runtime")
            .block_on(async { futures::future::join_all(render_futures.into_iter()).await })
            .into_iter()
            .collect::<Result<Vec<RenderedFile>>>()?;

        for file in rendered_files {
            let chapter = get_chapter(&mut book.items, &file.indices);
            chapter.content = file.content;
        }

        Ok(book)
    }

    fn supports_renderer(&self, renderer: &str) -> Result<bool> {
        Ok(renderer == "html")
    }
}

/// Recursively scans all chapters and turns their contents into
/// rendered file futures.
fn extract_render_futures<'a>(
    items: impl IntoIterator<Item = &'a mut BookItem> + 'a,
    indices: &mut Vec<usize>,
    renderer_factory: &'a impl Fn(Option<PathBuf>) -> MdKroki,
) -> Vec<Pin<Box<dyn Future<Output = Result<RenderedFile>> + 'a>>> {
    let mut files = Vec::new();
    indices.push(0);
    for (index, item) in items.into_iter().enumerate() {
        if let BookItem::Chapter(chapter) = item {
            let chapter_source = chapter.source_path.clone();
            let chapter_content = chapter.content.split_off(0);
            *indices.last_mut().unwrap() = index;
            let indices_clone = indices.clone();
            files.extend(extract_render_futures(
                &mut chapter.sub_items,
                indices,
                renderer_factory,
            ));
            files.push(Box::pin(async move {
                let renderer = renderer_factory(chapter_source);
                let render_future = renderer.render(chapter_content);
                let new_content = render_future.await?;
                Ok(RenderedFile {
                    indices: indices_clone,
                    content: new_content,
                })
            }));
        }
    }
    indices.pop();
    files
}

/// Recovers a mutable reference to a book chapter given a path of indices.
fn get_chapter<'a>(mut items: &'a mut Vec<BookItem>, indices: &[usize]) -> &'a mut Chapter {
    for index in &indices[..indices.len() - 1] {
        let item = items.get_mut(*index).expect("index disappeared");
        match item {
            BookItem::Chapter(chapter) => items = &mut chapter.sub_items,
            _ => panic!("indexed book item wasn't a chapter"),
        }
    }
    match items
        .get_mut(*indices.last().unwrap())
        .expect("chapter not found")
    {
        BookItem::Chapter(chapter) => chapter,
        _ => panic!("indexed book item wasn't a chapter"),
    }
}

/// The result of rendering a file through kroki.
struct RenderedFile {
    indices: Vec<usize>,
    content: String,
}
