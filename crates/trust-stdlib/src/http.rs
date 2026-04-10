//! HTTP：`fetch` / `fetchText` 的 Rust 实现（与生成代码中的旧 `__trust_fetch*` 语义一致）。

#[derive(Clone)]
pub struct FetchInit {
    pub method: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
}

pub async fn fetch_text(url: String) -> String {
    reqwest::get(url.as_str())
        .await
        .expect("HTTP GET failed")
        .text()
        .await
        .expect("read response body failed")
}

pub async fn fetch(url: String, init: Option<FetchInit>) -> reqwest::Response {
    let client = reqwest::Client::new();
    let req = match init {
        None => client.get(url.as_str()),
        Some(i) => {
            let method =
                reqwest::Method::from_bytes(i.method.as_bytes()).expect("invalid HTTP method");
            let mut rb = client.request(method, url.as_str());
            for (k, v) in i.headers {
                rb = rb.header(k, v);
            }
            if let Some(b) = i.body {
                rb = rb.body(b);
            }
            rb
        }
    };
    req.send().await.expect("HTTP request failed")
}
