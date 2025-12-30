use anyhow::Context;
use std::sync::{Arc, Mutex};

use scraper::{Html, Selector};
use tokio::fs;
use url::Url;

use crate::request;

#[derive(Clone, Debug)]
pub struct Everia {
    url: Url,
    out_folder: String,
    task: u32,
    retry: u32,
}

impl Everia {
    pub fn parse_name(url: &str) -> String {
        let mut url: Vec<&str> = url.split("/").collect();
        url.pop();
        urlencoding::decode(url.pop().unwrap()).unwrap().to_string()
    }
    pub fn new(url: &str, out_folder: Option<&str>) -> anyhow::Result<Self> {
        let out_folder = match out_folder {
            Some(_as) => _as.to_string(),
            None => Everia::parse_name(url),
        };
        println!("out_folder {}", out_folder);
        Ok(Self {
            url: Url::parse(url).context("innvalid url")?,
            out_folder,
            task: 8,
            retry: 30,
        })
    }

    /// parse post link from page
    pub fn collect_posts_link(&self, body: &str) -> anyhow::Result<Vec<String>> {
        let document = Html::parse_document(body);
        // select id="content"
        let selector = Selector::parse("#content").unwrap();
        let selected = document.select(&selector);

        let selector = Selector::parse("a").unwrap();
        let mut links = Vec::new();
        if let Some(document) = Iterator::last(selected) {
            let selected = document.select(&selector);
            for document in selected {
                if document.attr("rel").is_none() {
                    continue;
                }
                if let Some(href) = document.attr("href") {
                    links.push(href.to_string());
                }
            }
        }

        Ok(links)
    }

    pub async fn collect_image_link_from_post(
        &self,
        post_url: &str,
    ) -> anyhow::Result<Vec<String>> {
        let client = request::client()?;
        let response = client.get(post_url).send().await?;
        let text = response.text().await?;
        let document = Html::parse_document(&text);
        let selector = Selector::parse("div.entry-content").unwrap();
        let selected = document.select(&selector);

        let mut image_links = Vec::new();
        if let Some(document) = Iterator::last(selected) {
            let selector = Selector::parse("img").unwrap();
            let selected = document.select(&selector);
            for sel in selected {
                if let Some(link) = sel.attr("data-lazy-src") {
                    image_links.push(link.to_string());
                }
            }
        }

        Ok(image_links)
    }

    /// this function use to create sub folder for each post.
    /// and return folder path likes `./abc/folder/`
    pub async fn create_folder_from_url(&self, post_url: &str) -> String {
        let mut folder_name: Vec<&str> = post_url.split("/").collect();
        folder_name.pop();
        let folder_name = folder_name.pop().unwrap_or("None");
        let folder_name = urlencoding::decode(folder_name).unwrap().to_string();
        let path = format!("{}/{}/", self.out_folder, folder_name);
        let _ = fs::create_dir_all(&path).await;
        path
    }

    /// Download all images from post
    pub async fn download_posts(&self, post_url: &str) -> anyhow::Result<()> {
        let out_folder = self.create_folder_from_url(post_url).await;
        let image_links = self.collect_image_link_from_post(post_url).await?;
        let mut tasks = Vec::new();

        for img in image_links {
            let out_folder = out_folder.clone();
            let img = img.clone();

            let mut retry = self.retry;
            tasks.push(tokio::spawn(async move {
                let img_name = img.split("/").last().unwrap();
                'request: loop {
                    if let Ok(client) = request::client()
                        && let Ok(response) = client.get(&img).send().await
                        && response.status() == 200
                        && let Ok(content) = response.bytes().await
                    {
                        let _ = fs::write(format!("{}/{}", out_folder, img_name), content).await;
                    } else if retry > 0 {
                        retry -= 1;
                        continue 'request;
                    }
                    break 'request;
                }
            }));
        }

        for task in tasks {
            let _ = task.await;
        }
        Ok(())
    }

    pub async fn collect_posts_per_page(&self, url: &str) -> anyhow::Result<Vec<String>> {
        let client = request::client()?;
        let response = client.get(url).send().await?;
        if !response.status().is_redirection() {
            let post_links = self.collect_posts_link(response.text().await?.as_str())?;
            Ok(post_links)
        } else {
            Err(anyhow::anyhow!("page is invalid"))
        }
    }

    pub async fn collect_posts(&self) -> Vec<String> {
        let mut page = 0;
        let mut all_post = Vec::new();
        loop {
            page += 1;
            let url = if page != 1 {
                format!("{}page/{}/", self.url, page)
            } else {
                self.url.clone().to_string()
            };
            print!("fetching {} ", url);
            match self.collect_posts_per_page(&url).await {
                Ok(mut posts) => {
                    println!("- PASS");
                    all_post.append(&mut posts);
                }
                Err(_) => break,
            }
        }
        println!("- Done");
        all_post
    }

    pub async fn download(&self) {
        let posts = self.collect_posts().await;
        let posts = Arc::new(Mutex::new(posts));
        let mut threads = Vec::new();

        for _ in 0..self.task {
            let posts = posts.clone();
            let self_instance = self.clone();
            threads.push(tokio::task::spawn(async move {
                loop {
                    let (post, post_len) = {
                        if let Ok(mut posts) = posts.lock() {
                            (posts.pop(), posts.len())
                        } else {
                            continue;
                        }
                    };
                    if let Some(post) = post {
                        println!("[{}] downloading {}", post_len, post);
                        let _ = self_instance.download_posts(&post).await;
                        println!("[{}] - Done", post_len);
                    } else {
                        break;
                    }
                }
            }));
        }

        for th in threads {
            let _ = th.await;
        }
    }
}
