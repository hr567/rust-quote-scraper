use std::sync::{Arc, LazyLock};

use reqwest::Client;
use scraper::{Html, Selector};
use tokio::sync::{mpsc, Semaphore};
use url::Url;

const MAX_TASK: usize = 8;

static URL: LazyLock<Url> = LazyLock::new(|| Url::parse("https://quotes.toscrape.com/").unwrap());
static CLIENT: LazyLock<Client> = LazyLock::new(|| {
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
    let page_url = URL.join(&format!("page/{idx}/")).unwrap();
    let res = CLIENT.get(page_url).send().await?;
    let html = res.text().await?;
    Ok(html)
}

static QUOTE: LazyLock<Selector> = LazyLock::new(|| Selector::parse(r#".quote"#).unwrap());
static TEXT: LazyLock<Selector> = LazyLock::new(|| Selector::parse(r#".text"#).unwrap());
static AUTHOR: LazyLock<Selector> = LazyLock::new(|| Selector::parse(r#".author"#).unwrap());
static TAG: LazyLock<Selector> = LazyLock::new(|| Selector::parse(r#".tag"#).unwrap());
fn parse_quote_html(html: &str) -> Vec<Quote> {
    Html::parse_document(html)
        .select(&QUOTE)
        .map(|quote| Quote {
            text: quote.select(&TEXT).next().unwrap().inner_html(),
            author: quote.select(&AUTHOR).next().unwrap().inner_html(),
            tags: quote.select(&TAG).map(|e| e.inner_html()).collect(),
        })
        .collect()
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let pool = Arc::new(Semaphore::new(MAX_TASK));
    let (tx, mut rx) = mpsc::channel(MAX_TASK);

    for page in 1..12 {
        let pool = Arc::clone(&pool);
        let tx = tx.clone();
        tokio::spawn(async move {
            let _permit = pool.acquire().await.unwrap();
            let text = download_quote_html(page).await.unwrap();
            tx.send(text).await.unwrap();
        });
    }
    drop(tx);

    let mut quotes = Vec::new();
    while let Some(html) = rx.recv().await {
        quotes.extend(parse_quote_html(&html));
    }
    println!("{quotes:?}");
}
