use std::time::{SystemTime, UNIX_EPOCH};

use url::Url;

pub fn now_unix_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

pub fn is_url_like(s: &str) -> bool {
    let s = s.trim();
    s.starts_with("https://") || s.starts_with("http://")
}

pub fn split_comma_separated(s: &str) -> Vec<String> {
    s.split(',')
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .map(|p| p.to_string())
        .collect()
}

pub fn strip_fragment(mut url: Url) -> Url {
    url.set_fragment(None);
    url
}

pub fn host_variants(host: &str) -> Vec<String> {
    let h = host.to_ascii_lowercase();
    if let Some(rest) = h.strip_prefix("www.") {
        vec![h.clone(), rest.to_string()]
    } else {
        vec![h.clone(), format!("www.{h}")]
    }
}
