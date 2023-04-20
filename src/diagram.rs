use anyhow::{anyhow, bail, Context, Result};
use mdbook::book::{Book, BookItem, Chapter};
use mdbook::preprocess::PreprocessorContext;
use serde::Serialize;
use std::{path::Path, path::PathBuf, sync::Arc};
use tokio::sync::Mutex;

#[derive(Debug)]
pub(crate) struct Diagram {
    pub diagram_type: String,
    pub output_format: String,
    pub replace_text: String,
    pub indices: Vec<usize>,
    pub content: DiagramContent,
}

#[derive(Debug)]
pub enum DiagramContent {
    Raw(String),
    Path { kind: PathRoot, path: PathBuf },
}

#[derive(Debug)]
pub enum PathRoot {
    System,
    Book,
    Source,
    This,
}

impl Diagram {
    pub async fn resolve(
        self,
        ctx: &PreprocessorContext,
        book: Arc<Mutex<Book>>,
        src: &Path,
        endpoint: &String,
    ) -> Result<()> {
        let diagram_source = match self.content {
            DiagramContent::Raw(s) => s,
            DiagramContent::Path { kind, path } => {
                let full_path = match kind {
                    PathRoot::System => path,
                    PathRoot::Book => ctx.root.join(path),
                    PathRoot::Source => ctx.root.join(src).join(path),
                    PathRoot::This => {
                        let mut book_lock = book.lock().await;
                        let chapter = get_chapter(&mut book_lock.sections, &self.indices)?;
                        ctx.root
                            .join(src)
                            .join(
                                chapter
                                    .source_path
                                    .clone()
                                    .ok_or(anyhow!("no path for chapter"))?
                                    .parent()
                                    .ok_or(anyhow!("chapter path has no parent"))?,
                            )
                            .join(path)
                    }
                };
                std::fs::read_to_string(&full_path)
                    .context(format!("attempting to read: {:?}", full_path))?
            }
        };
        let request_body = KrokiRequestBody {
            diagram_source,
            diagram_type: self.diagram_type,
            output_format: self.output_format,
        };

        let svg = get_svg(request_body, endpoint).await?;
        let mut book_lock = book.lock().await;
        let chapter = get_chapter(&mut book_lock.sections, &self.indices)?;
        chapter.content = chapter.content.replace(&self.replace_text, &svg);

        Ok(())
    }
}

#[derive(Serialize, Debug)]
struct KrokiRequestBody {
    diagram_source: String,
    diagram_type: String,
    output_format: String,
}

fn get_chapter<'a>(
    mut items: &'a mut Vec<BookItem>,
    indices: &Vec<usize>,
) -> Result<&'a mut Chapter> {
    for index in &indices[..indices.len() - 1] {
        let item = items.get_mut(*index).ok_or(anyhow!("index disappeared"))?;
        match item {
            BookItem::Chapter(ref mut chapter) => items = &mut chapter.sub_items,
            _ => bail!("indexed book item wasn't a chapter"),
        }
    }
    match items
        .get_mut(*indices.last().unwrap())
        .ok_or(anyhow!("chapter not found"))?
    {
        BookItem::Chapter(ref mut chapter) => Ok(chapter),
        _ => bail!("indexed book item wasn't a chapter"),
    }
}

async fn get_svg(request_body: KrokiRequestBody, endpoint: &String) -> Result<String> {
    let client = reqwest::Client::new();
    let mut result = client
        .post(endpoint)
        .body(serde_json::to_string(&request_body)?)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    let start_index = result
        .find("<svg")
        .ok_or(anyhow!("didn't find '<svg' in kroki response: {}", result))?;
    result.replace_range(..start_index, "");
    result.insert_str(0, "<pre>");
    result.push_str("</pre>");
    Ok(result)
}
