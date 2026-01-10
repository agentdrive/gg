use anyhow::{anyhow, Context, Result};
use bytes::BytesMut;
use futures_util::StreamExt;
use reqwest::{header, redirect, Client, StatusCode};
use std::{collections::HashSet, sync::Arc, time::Duration};
use url::Url;

#[derive(Debug, Clone)]
pub struct HttpOptions {
    pub user_agent: String,
    pub timeout: Duration,
    pub connect_timeout: Duration,
    pub max_body_bytes: usize,
}

impl Default for HttpOptions {
    fn default() -> Self {
        Self {
            user_agent: format!("gg/{}", env!("CARGO_PKG_VERSION")),
            timeout: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(10),
            max_body_bytes: 32 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HttpFetch {
    pub requested: Url,
    pub final_url: Url,
    pub status: StatusCode,
    pub content_type: Option<String>,
    pub body: Vec<u8>,
}

pub fn build_client_all(opts: &HttpOptions) -> Result<Client> {
    let c = Client::builder()
        .user_agent(opts.user_agent.clone())
        .timeout(opts.timeout)
        .connect_timeout(opts.connect_timeout)
        .redirect(redirect::Policy::limited(10))
        .brotli(true)
        .gzip(true)
        .deflate(true)
        .build()
        .context("failed to build HTTP client")?;
    Ok(c)
}

pub fn build_client_internal(opts: &HttpOptions, allowed_hosts: HashSet<String>) -> Result<Client> {
    let allowed_hosts = Arc::new(allowed_hosts);

    let policy = redirect::Policy::custom(move |attempt| {
        if attempt.previous().len() >= 10 {
            return attempt.stop();
        }
        if let Some(host) = attempt.url().host_str() {
            let host_l = host.to_ascii_lowercase();
            if allowed_hosts.contains(&host_l) {
                return attempt.follow();
            }
        }
        attempt.stop()
    });

    let c = Client::builder()
        .user_agent(opts.user_agent.clone())
        .timeout(opts.timeout)
        .connect_timeout(opts.connect_timeout)
        .redirect(policy)
        .brotli(true)
        .gzip(true)
        .deflate(true)
        .build()
        .context("failed to build HTTP client")?;
    Ok(c)
}

pub async fn fetch_limited(client: &Client, url: Url, max_bytes: usize) -> Result<HttpFetch> {
    let requested = url.clone();
    let resp = client
        .get(url)
        .header(header::ACCEPT, "text/html,application/xhtml+xml;q=0.9,*/*;q=0.1")
        .send()
        .await
        .with_context(|| format!("HTTP request failed: {requested}"))?;

    let status = resp.status();
    let final_url = resp.url().clone();
    let content_type = resp
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let mut stream = resp.bytes_stream();
    let mut buf = BytesMut::new();

    while let Some(item) = stream.next().await {
        let chunk = item.context("failed while streaming response body")?;
        if buf.len() + chunk.len() > max_bytes {
            return Err(anyhow!(
                "response body too large (>{} bytes) for {final_url}",
                max_bytes
            ));
        }
        buf.extend_from_slice(&chunk);
    }

    Ok(HttpFetch {
        requested,
        final_url,
        status,
        content_type,
        body: buf.to_vec(),
    })
}

pub fn is_probably_html(content_type: Option<&str>, body: &[u8]) -> bool {
    if let Some(ct) = content_type {
        let ct_l = ct.to_ascii_lowercase();
        if ct_l.contains("text/html") || ct_l.contains("application/xhtml+xml") {
            return true;
        }
        // Some sites send `text/plain` for HTML. Fall through to sniffing.
    }

    // Sniff first couple KB.
    let head = &body[..body.len().min(2048)];
    let head_l = String::from_utf8_lossy(head).to_ascii_lowercase();
    head_l.contains("<html") || head_l.contains("<!doctype html")
}
