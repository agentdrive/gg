use anyhow::{anyhow, Context, Result};
use flate2::read::GzDecoder;
use quick_xml::events::Event;
use quick_xml::Reader;
use reqwest::Client;
use std::collections::{HashSet, VecDeque};
use std::io::Read;
use url::Url;

use crate::http;

#[derive(Debug, Default)]
struct ParsedSitemap {
    urls: Vec<Url>,
    child_sitemaps: Vec<Url>,
}

/// Attempt to discover and parse a site's sitemap(s), returning all URLs found.
///
/// This is used as a *seed* for crawling so that pages not reachable via
/// in-page links can still be included.
pub async fn discover_sitemap_urls(client: &Client, base: &Url, max_bytes: usize) -> Result<Vec<Url>> {
    let origin = origin_url(base)?;

    let candidates = [
        "sitemap.xml",
        "sitemap_index.xml",
        "sitemap-index.xml",
        "sitemap.xml.gz",
        "sitemap_index.xml.gz",
        "sitemap-index.xml.gz",
    ];

    let mut root_sitemaps = Vec::new();
    for name in candidates {
        let url = origin.join(name).with_context(|| format!("bad sitemap url: {name}"))?;
        let resp = match http::fetch_limited(client, url.clone(), max_bytes).await {
            Ok(r) => r,
            Err(_) => continue,
        };
        if resp.status.as_u16() == 404 {
            continue;
        }
        if !resp.status.is_success() {
            continue;
        }
        root_sitemaps.push((url, resp.body));
        // Use the first sitemap that exists; many sites have multiple, but fetching all can be expensive.
        break;
    }

    let mut out = Vec::new();
    let mut seen_sitemaps: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<Url> = VecDeque::new();

    for (url, _) in &root_sitemaps {
        queue.push_back(url.clone());
    }

    // We'll refetch the root sitemap URLs too, to keep logic uniform.
    while let Some(sm_url) = queue.pop_front() {
        let key = sm_url.as_str().to_string();
        if !seen_sitemaps.insert(key) {
            continue;
        }

        let resp = match http::fetch_limited(client, sm_url.clone(), max_bytes).await {
            Ok(r) => r,
            Err(_) => continue,
        };
        if !resp.status.is_success() {
            continue;
        }

        let bytes = maybe_gunzip(&resp.body)?;
        let parsed = parse_sitemap_xml(&bytes)?;
        out.extend(parsed.urls);
        for child in parsed.child_sitemaps {
            queue.push_back(child);
        }
    }

    Ok(out)
}

fn origin_url(base: &Url) -> Result<Url> {
    let host = base
        .host_str()
        .ok_or_else(|| anyhow!("base URL has no host: {base}"))?;
    let scheme = base.scheme();
    Url::parse(&format!("{scheme}://{host}/")).with_context(|| format!("failed to build origin for {base}"))
}

fn maybe_gunzip(bytes: &[u8]) -> Result<Vec<u8>> {
    // Quick sniff for gzip magic bytes.
    if bytes.len() >= 2 && bytes[0] == 0x1F && bytes[1] == 0x8B {
        let mut d = GzDecoder::new(bytes);
        let mut out = Vec::new();
        d.read_to_end(&mut out).context("failed to gunzip sitemap")?;
        return Ok(out);
    }
    Ok(bytes.to_vec())
}

fn parse_sitemap_xml(bytes: &[u8]) -> Result<ParsedSitemap> {
    let mut reader = Reader::from_reader(bytes);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut parsed = ParsedSitemap::default();

    enum Ctx {
        None,
        Url,
        Sitemap,
    }

    let mut ctx = Ctx::None;
    let mut in_loc = false;
    let mut loc = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = e.local_name();
                match name.as_ref() {
                    b"url" => ctx = Ctx::Url,
                    b"sitemap" => ctx = Ctx::Sitemap,
                    b"loc" => {
                        in_loc = true;
                        loc.clear();
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(e)) => {
                if in_loc {
                    loc.push_str(&e.unescape().unwrap_or_default());
                }
            }
            Ok(Event::End(e)) => {
                let name = e.local_name();
                match name.as_ref() {
                    b"loc" => {
                        in_loc = false;
                        let u = loc.trim();
                        if !u.is_empty() {
                            if let Ok(url) = Url::parse(u) {
                                match ctx {
                                    Ctx::Url => parsed.urls.push(url),
                                    Ctx::Sitemap => parsed.child_sitemaps.push(url),
                                    Ctx::None => {}
                                }
                            }
                        }
                    }
                    b"url" | b"sitemap" => ctx = Ctx::None,
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow!("sitemap XML parse error: {e}")),
            _ => {}
        }
        buf.clear();
    }

    Ok(parsed)
}
