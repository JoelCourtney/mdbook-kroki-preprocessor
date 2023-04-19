use std::{sync::Arc, path::PathBuf};
use std::str::FromStr;
use tokio::sync::Mutex;
use anyhow::{Result, Context, anyhow, bail};
use mdbook::book::{Book, Chapter, BookItem};
use serde::Serialize;

#[derive(Debug)]
pub(crate) struct Diagram {
    pub diagram_type: String,
    pub diagram_format: DiagramFormat,
    pub output_path: PathBuf,
    pub chapter_path: PathBuf,
    pub content: String,
    pub is_path: bool
}

#[derive(Default, Debug)]
pub(crate) enum DiagramFormat {
    #[default] Svg,
    Png,
    Jpeg,
    Pdf
}

impl FromStr for DiagramFormat {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        use DiagramFormat::*;

        match s {
            "svg" => Ok(Svg),
            "png" => Ok(Png),
            "jpeg" => Ok(Jpeg),
            "pdf" => Ok(Pdf),
            _ => bail!("output format must be svg, png, jpeg, or pdf")
        }
    }
}

impl ToString for DiagramFormat {
    fn to_string(&self) -> String {
        use DiagramFormat::*;

        match self {
            Svg => "svg",
            Png => "png",
            Jpeg => "jpeg",
            Pdf => "pdf"
        }.to_string()
    }
}

impl Diagram {
    pub async fn resolve(self, src: &PathBuf, endpoint: &String) -> Result<()> {
        let request_body = KrokiRequestBody {
            diagram_source: if self.is_path {
                let mut path = PathBuf::new();
                if !self.content.starts_with('/') {
                    path = src.clone();
                    // let chapter = get_chapter(&mut book_lock.sections, &self.indices)?;
                    // path.push(chapter.source_path.clone().ok_or(anyhow!("no path for chapter"))?);
                    path.push(self.chapter_path);
                    path.pop();
                }
                path.push(self.content);
                std::fs::read_to_string(path.clone()).context(format!("attempting to read: {:?}", path))?
            } else {
                self.content
            },
            diagram_type: self.diagram_type,
            output_format: self.diagram_format.to_string()
        };
        let file_contents = get_rendered_file(request_body, endpoint).await?;
        std::fs::write(self.output_path, file_contents)?;

        Ok(())
    }
}

#[derive(Serialize, Debug)]
struct KrokiRequestBody {
    diagram_source: String,
    diagram_type: String,
    output_format: String
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

async fn get_rendered_file(request_body: KrokiRequestBody, endpoint: &String) -> Result<String> {
    let client = reqwest::Client::new();
    let mut result = client.post(endpoint)
        .body(serde_json::to_string(&request_body)?)
        .send().await?.error_for_status()?.text().await?;
    // dbg!(&result);
    // let start_index = result.find("<svg").ok_or(anyhow!("didn't find '<svg' in kroki response: {}", result))?;
    // let start_index = 0;
    // result.replace_range(..start_index, "");
    // result.insert_str(0, "<pre>");
    // result.push_str("</pre>");
    Ok(result)
}
