use std::sync::Arc;

use lazy_static::lazy_static;
use reqwest::Client;
use scraper::{Html, Selector};
use tokio::sync::{mpsc, Semaphore};
use url::Url;

const MAX_TASK: usize = 16;

lazy_static! {
    static ref URL: Url = Url::parse("https://quotes.toscrape.com/").unwrap();
    static ref CLIENT: Client = {
        use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
        let mut headers = HeaderMap::new();
        let user_agent = HeaderValue::from_static(
            r"Mozilla/5.0 (X11; Linux x86_64; rv:84.0) Gecko/20100101 Firefox/84.0",
        );
        headers.insert(USER_AGENT, user_agent);
        Client::builder().default_headers(headers).build().unwrap()
    };
}

#[allow(dead_code)]
#[derive(Debug)]
struct Quote {
    text: String,
    author: String,
    tags: Vec<String>,
}

async fn download_quote_html(idx: usize) -> reqwest::Result<String> {
    let page_url = URL.join(&format!("page/{}/", idx)).unwrap();
    let res = CLIENT.get(page_url).send().await?;
    let html = res.text().await?;
    Ok(html)
}

fn parse_quote_html(page: Html) -> Vec<Quote> {
    lazy_static! {
        static ref QUOTE: Selector = Selector::parse(r#".quote"#).unwrap();
        static ref TEXT: Selector = Selector::parse(r#".text"#).unwrap();
        static ref AUTHOR: Selector = Selector::parse(r#".author"#).unwrap();
        static ref TAG: Selector = Selector::parse(r#".tag"#).unwrap();
    }
    page.select(&QUOTE)
        .map(|quote| Quote {
            text: quote.select(&TEXT).next().unwrap().inner_html(),
            author: quote.select(&AUTHOR).next().unwrap().inner_html(),
            tags: quote.select(&TAG).map(|e| e.inner_html()).collect(),
        })
        .collect()
}

#[tokio::main]
async fn main() {
    let pool = Arc::new(Semaphore::new(MAX_TASK));
    let (tx, mut rx) = mpsc::unbounded_channel::<Quote>();

    for page in 1..20 {
        let pool = Arc::clone(&pool);
        let tx = tx.clone();
        tokio::spawn(async move {
            let _permit = pool.acquire().await.unwrap();
            let text = download_quote_html(page).await.unwrap();
            let html = Html::parse_document(&text);
            let quotes = parse_quote_html(html);
            for quote in quotes.into_iter() {
                tx.send(quote).unwrap();
            }
        });
    }
    drop(tx);

    while let Some(quote) = rx.recv().await {
        println!("{:?}", quote);
    }
}
