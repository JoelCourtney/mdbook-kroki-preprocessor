use std::{sync::Arc, path::PathBuf};
use tokio::sync::Mutex;
use anyhow::{Result, Context, anyhow, bail};
use mdbook::book::{Book, Chapter, BookItem};
use serde::Serialize;

#[derive(Debug)]
pub(crate) struct Diagram {
    pub diagram_type: String,
    pub replace_text: String,
    pub indices: Vec<usize>,
    pub content: String,
    pub is_path: bool
}

impl Diagram {
    pub async fn resolve(self, book: Arc<Mutex<Book>>, src: &PathBuf, endpoint: &String) -> Result<()> {
        let request_body = KrokiRequestBody {
            diagram_source: if self.is_path {
                let mut path = PathBuf::new();
                if !self.content.starts_with('/') {
                    path = src.clone();
                    let mut book_lock = book.lock().await;
                    let chapter = get_chapter(&mut book_lock.sections, &self.indices)?;
                    path.push(chapter.source_path.clone().ok_or(anyhow!("no path for chapter"))?);
                    std::mem::drop(book_lock);
                    path.pop();
                }
                path.push(self.content);
                std::fs::read_to_string(path.clone()).context(format!("attempting to read: {:?}", path))?
            } else {
                self.content
            },
            diagram_type: self.diagram_type,
            output_format: "svg"
        };
        let svg = get_svg(request_body, endpoint).await?;
        let mut book_lock = book.lock().await;
        let chapter = get_chapter(&mut book_lock.sections, &self.indices)?;
        chapter.content = chapter.content.replace(&self.replace_text, &svg).to_string();

        Ok(())
    }
}

#[derive(Serialize, Debug)]
struct KrokiRequestBody {
    diagram_source: String,
    diagram_type: String,
    output_format: &'static str
}

fn get_chapter<'a>(mut items: &'a mut Vec<BookItem>, indices: &Vec<usize>) -> Result<&'a mut Chapter> {
    for index in &indices[..indices.len()-1] {
        let item = items.into_iter().nth(*index).ok_or(anyhow!("index disappeared"))?;
        match item {
            BookItem::Chapter(ref mut chapter) => items = &mut chapter.sub_items,
            _ => bail!("indexed book item wasn't a chapter")
        }
    }
    match items.into_iter().nth(*indices.last().unwrap()).ok_or(anyhow!("chapter not found"))? {
        BookItem::Chapter(ref mut chapter) => Ok(chapter),
        _ => bail!("indexed book item wasn't a chapter")
    }
}

async fn get_svg(request_body: KrokiRequestBody, endpoint: &String) -> Result<String> {
    let client = reqwest::Client::new();
    let mut result = client.post(endpoint)
        .body(serde_json::to_string(&request_body)?)
        .send().await?.error_for_status()?.text().await?;
    let start_index = result.find("<svg").ok_or(anyhow!("didn't find '<svg' in kroki response: {}", result))?;
    result.replace_range(..start_index, "");
    result.insert_str(0, "<pre>");
    result.push_str("</pre>");
    Ok(result)
}