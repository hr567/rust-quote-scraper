use std::sync::Arc;

use once_cell::sync::Lazy;
use reqwest::Client;
use scraper::{Html, Selector};
use tokio::sync::{mpsc, Semaphore};
use url::Url;

const MAX_TASK: usize = 8;

static URL: Lazy<Url> = Lazy::new(|| Url::parse("https://quotes.toscrape.com/").unwrap());
static CLIENT: Lazy<Client> = Lazy::new(|| {
    use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
    let mut headers = HeaderMap::new();
    let user_agent = HeaderValue::from_static(
        r"Mozilla/5.0 (X11; Linux x86_64; rv:84.0) Gecko/20100101 Firefox/84.0",
    );
    headers.insert(USER_AGENT, user_agent);
    Client::builder().default_headers(headers).build().unwrap()
});

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

static QUOTE: Lazy<Selector> = Lazy::new(|| Selector::parse(r#".quote"#).unwrap());
static TEXT: Lazy<Selector> = Lazy::new(|| Selector::parse(r#".text"#).unwrap());
static AUTHOR: Lazy<Selector> = Lazy::new(|| Selector::parse(r#".author"#).unwrap());
static TAG: Lazy<Selector> = Lazy::new(|| Selector::parse(r#".tag"#).unwrap());
fn parse_quote_html(page: Html) -> Vec<Quote> {
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
