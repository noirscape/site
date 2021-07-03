use chrono::prelude::*;
use color_eyre::eyre::{eyre, Result, WrapErr};
use glob::glob;
use std::{cmp::Ordering, path::PathBuf};
use tokio::fs;

pub mod frontmatter;

#[derive(Eq, PartialEq, Debug, Clone)]
pub struct Post {
    pub front_matter: frontmatter::Data,
    pub link: String,
    pub body_html: String,
    pub date: DateTime<FixedOffset>,
    pub reading_time: String,
}

impl Into<jsonfeed::Item> for Post {
    fn into(self) -> jsonfeed::Item {
        let mut result = jsonfeed::Item::builder()
            .title(self.front_matter.title)
            .content_html(self.body_html)
            .id(format!("https://noirscape.dev/{}", self.link))
            .url(format!("https://noirscape.dev/{}", self.link))
            .date_published(self.date.to_rfc3339())
            .author(
                jsonfeed::Author::new()
                    .name("Techpriest")
                    .url("https://noirscape.dev")
                    .avatar("https://noirscape.dev/static/img/avatar.png"),
            );

        let mut tags: Vec<String> = vec![];

        if let Some(series) = self.front_matter.series {
            tags.push(series);
        }

        if let Some(mut meta_tags) = self.front_matter.tags {
            tags.append(&mut meta_tags);
        }

        if tags.len() != 0 {
            result = result.tags(tags);
        }

        if let Some(image_url) = self.front_matter.image {
            result = result.image(image_url);
        }

        result.build().unwrap()
    }
}

impl Ord for Post {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(&other).unwrap()
    }
}

impl PartialOrd for Post {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.date.cmp(&other.date))
    }
}

impl Post {
    pub fn detri(&self) -> String {
        self.date.format("M%m %d %Y").to_string()
    }
}

trait ConvertDisplaySeconds {
    fn convert_seconds(&self) -> String;
}

impl ConvertDisplaySeconds for u64 {
    fn convert_seconds(&self) -> String {
        let minutes: u64 = self / 60;
        return String::from(format!("{} minutes", minutes));
    }
} 

async fn read_post(dir: &str, fname: PathBuf) -> Result<Post> {
    let body = fs::read_to_string(fname.clone())
        .await
        .wrap_err_with(|| format!("can't read {:?}", fname))?;
    let (front_matter, content_offset) = frontmatter::Data::parse(body.clone().as_str())
        .wrap_err_with(|| format!("can't parse frontmatter of {:?}", fname))?;
    let body = &body[content_offset..];
    let date = NaiveDate::parse_from_str(&front_matter.clone().date, "%Y-%m-%d")
        .map_err(|why| eyre!("error parsing date in {:?}: {}", fname, why))?;
    let link = format!("{}/{}", dir, fname.file_stem().unwrap().to_str().unwrap());
    let body_html = crate::app::markdown::render(&body)
        .wrap_err_with(|| format!("can't parse markdown for {:?}", fname))?;
    let date: DateTime<FixedOffset> =
        DateTime::<Utc>::from_utc(NaiveDateTime::new(date, NaiveTime::from_hms(0, 0, 0)), Utc)
            .with_timezone(&Utc)
            .into();
    let reading_time: String = estimated_read_time::text(body, &estimated_read_time::Options::new().wpm(225)).seconds().convert_seconds();

    Ok(Post {
        front_matter,
        link,
        body_html,
        date,
        reading_time,
    })
}

pub async fn load(dir: &str) -> Result<Vec<Post>> {
    let futs = glob(&format!("{}/*.md", dir))?
        .filter_map(Result::ok)
        .map(|fname| read_post(dir, fname));

    let mut result: Vec<Post> = futures::future::join_all(futs)
        .await
        .into_iter()
        .map(Result::unwrap)
        .collect();

    if result.len() == 0 {
        Err(eyre!("no posts loaded"))
    } else {
        result.sort();
        result.reverse();
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use color_eyre::eyre::Result;

    #[tokio::test]
    async fn blog() {
        let _ = pretty_env_logger::try_init();
        load("blog").await.expect("posts to load");
    }

    #[tokio::test]
    async fn gallery() -> Result<()> {
        let _ = pretty_env_logger::try_init();
        load("gallery").await?;
        Ok(())
    }
}
