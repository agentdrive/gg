use std::{
    collections::{HashSet, VecDeque},
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::task::JoinSet;
use url::Url;
use regex::Regex;

use crate::{
    cache::Cache,
    http::{self, HttpOptions},
    sitemap,
    util::{host_variants, now_unix_secs, strip_fragment},
};

use html_to_markdown_rs::{
    convert_with_metadata, convert_with_visitor,
    metadata::LinkMetadata,
    options::{CodeBlockStyle, ConversionOptions, HeadingStyle},
    visitor::{HtmlVisitor, NodeContext, VisitResult},
    MetadataConfig,
};

#[derive(Debug, Clone)]
pub struct CrawlOptions {
    pub parallelism: usize,
    pub max_depth: Option<usize>,
    pub use_sitemap: bool,
    pub http: HttpOptions,
}

impl Default for CrawlOptions {
    fn default() -> Self {
        let cpu = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        Self {
            parallelism: (cpu * 8).clamp(8, 256),
            max_depth: None,
            use_sitemap: true,
            http: HttpOptions::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlManifest {
    pub version: u32,
    pub root_url: String,
    pub generated_at: i64,
    pub pages: Vec<PageEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageEntry {
    pub url: String,
    pub cache_path: String,
    pub status: u16,
    pub content_type: Option<String>,
    pub fetched_at: i64,
    pub bytes: usize,
    pub markdown_bytes: usize,
    pub error: Option<String>,
}

#[derive(Debug)]
struct PageFetch {
    final_url: Url,
    status: u16,
    content_type: Option<String>,
    bytes: usize,
    markdown_bytes: usize,
    cache_path: Option<String>,
    links: Vec<Url>,
    error: Option<String>,
}

/// Ensure a single page is present in the cache. Returns the local Markdown path.
pub async fn ensure_page_cached(
    cache: &Cache,
    client: &Client,
    opts: &CrawlOptions,
    url: Url,
    refresh: bool,
) -> Result<PathBuf> {
    let path = cache.page_path(&url)?;
    if !refresh && cache.is_cached_file(&path) {
        return Ok(path);
    }

    let fetch = fetch_and_convert_page(client, opts, url.clone(), false, cache).await?;
    if let Some(rel) = fetch.cache_path {
        return Ok(cache.root().join(rel));
    }

    Err(anyhow!(
        "failed to cache page as markdown: {url} (status {})",
        fetch.status
    ))
}

/// Ensure a subtree is crawled and cached, returning a manifest.
///
/// The manifest is stored under `<subtree>/.gg/manifest.json`.
pub async fn ensure_subtree_cached(
    cache: &Cache,
    opts: &CrawlOptions,
    root: Url,
    refresh: bool,
) -> Result<CrawlManifest> {
    let manifest_path = cache.manifest_path_for_subtree(&root)?;
    if !refresh && manifest_path.is_file() {
        if let Ok(m) = read_manifest(&manifest_path) {
            // Basic sanity check; if it fails, we recrawl.
            if m.root_url == root.as_str() {
                return Ok(m);
            }
        }
    }

    let allowed_hosts: HashSet<String> = root
        .host_str()
        .map(|h| host_variants(h).into_iter().collect())
        .unwrap_or_default();

    let client = http::build_client_internal(&opts.http, allowed_hosts.clone())?;

    // Optionally seed from sitemap(s).
    let mut seeds: Vec<Url> = Vec::new();
    if opts.use_sitemap {
        // Keep sitemap fetch smaller than full pages.
        let max = (opts.http.max_body_bytes / 2).max(1024 * 1024);
        if let Ok(urls) = sitemap::discover_sitemap_urls(&client, &root, max).await {
            seeds = urls;
        }
    }

    let prefix = path_prefix(&root);

    let mut seen: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<(Url, usize)> = VecDeque::new();

    // Always include the root URL.
    seen.insert(canonical_key(&root));
    queue.push_back((root.clone(), 0));

    for u in seeds {
        if is_allowed_child(&u, &allowed_hosts, &prefix) {
            let k = canonical_key(&u);
            if seen.insert(k) {
                queue.push_back((u, 0));
            }
        }
    }

    // Conversion options tuned for search/indexing: no wrapping, ATX headings.
    let conv_options = ConversionOptions {
        heading_style: HeadingStyle::Atx,
        code_block_style: CodeBlockStyle::Backticks,
        extract_metadata: false,
        wrap: false,
        strip_newlines: true,
        whitespace_mode: html_to_markdown_rs::WhitespaceMode::Normalized,
        strip_tags: vec![
            "img".to_string(),
            "svg".to_string(),
            "picture".to_string(),
            "source".to_string(),
        ],
        preprocessing: html_to_markdown_rs::options::PreprocessingOptions {
            enabled: true,
            preset: html_to_markdown_rs::options::PreprocessingPreset::default(),
            remove_navigation: true,
            remove_forms: true,
        },
        ..Default::default()
    };

    // Metadata config: we only need links for crawling.
    let md_cfg = MetadataConfig {
        extract_document: false,
        extract_headers: false,
        extract_links: true,
        extract_images: false,
        extract_structured_data: false,
        max_structured_data_size: 0,
    };

    let generated_at = now_unix_secs();

    // Ensure the .gg directory exists.
    if let Some(parent) = manifest_path.parent() {
        fs::create_dir_all(parent).ok();
    }

    let mut joinset: JoinSet<Result<(usize, PageFetch)>> = JoinSet::new();
    let mut pages: Vec<PageEntry> = Vec::new();

    while !queue.is_empty() || !joinset.is_empty() {
        while joinset.len() < opts.parallelism && !queue.is_empty() {
            let (url, depth) = queue.pop_front().unwrap();
            let client = client.clone();
            let cache = cache.clone();
            let opts = opts.clone();
            let conv_options = conv_options.clone();
            let md_cfg = md_cfg.clone();
            joinset.spawn(async move {
                let f = fetch_and_convert_page_with_options(
                    &client,
                    &opts,
                    url,
                    true,
                    &cache,
                    Some(conv_options),
                    Some(md_cfg),
                )
                .await?;
                Ok((depth, f))
            });
        }

        if let Some(res) = joinset.join_next().await {
            let (depth, pf) = res.context("crawl task panicked")??;

            // Record manifest entry for pages that produced Markdown.
            if let Some(rel) = &pf.cache_path {
                pages.push(PageEntry {
                    url: pf.final_url.as_str().to_string(),
                    cache_path: rel.clone(),
                    status: pf.status,
                    content_type: pf.content_type.clone(),
                    fetched_at: now_unix_secs(),
                    bytes: pf.bytes,
                    markdown_bytes: pf.markdown_bytes,
                    error: pf.error.clone(),
                });
            }

            let next_depth = depth.saturating_add(1);
            if let Some(max) = opts.max_depth {
                if next_depth > max {
                    continue;
                }
            }

            for u in pf.links {
                if !is_allowed_child(&u, &allowed_hosts, &prefix) {
                    continue;
                }
                let k = canonical_key(&u);
                if seen.insert(k) {
                    queue.push_back((u, next_depth));
                }
            }
        }
    }

    let manifest = CrawlManifest {
        version: 1,
        root_url: root.as_str().to_string(),
        generated_at,
        pages,
    };

    write_manifest(&manifest_path, &manifest)?;
    Ok(manifest)
}

async fn fetch_and_convert_page(
    client: &Client,
    opts: &CrawlOptions,
    url: Url,
    extract_links: bool,
    cache: &Cache,
) -> Result<PageFetch> {
    fetch_and_convert_page_with_options(client, opts, url, extract_links, cache, None, None).await
}

async fn fetch_and_convert_page_with_options(
    client: &Client,
    opts: &CrawlOptions,
    url: Url,
    extract_links: bool,
    cache: &Cache,
    conv_options: Option<ConversionOptions>,
    md_cfg: Option<MetadataConfig>,
) -> Result<PageFetch> {
    let fetch = http::fetch_limited(client, url.clone(), opts.http.max_body_bytes).await?;

    let final_url = fetch.final_url.clone();
    let status = fetch.status.as_u16();
    let content_type = fetch.content_type.clone();
    let bytes_len = fetch.body.len();

    let is_html = http::is_probably_html(content_type.as_deref(), &fetch.body);

    if !is_html {
        return Ok(PageFetch {
            final_url,
            status,
            content_type,
            bytes: bytes_len,
            markdown_bytes: 0,
            cache_path: None,
            links: Vec::new(),
            error: Some("non-HTML content".to_string()),
        });
    }

    let html = String::from_utf8_lossy(&fetch.body).to_string();

    let mut links_out: Vec<Url> = Vec::new();
    let mut markdown: String = String::new();
    let mut md_err: Option<String> = None;

    if extract_links {
        let cfg = md_cfg.unwrap_or(MetadataConfig {
            extract_document: false,
            extract_headers: false,
            extract_links: true,
            extract_images: false,
            extract_structured_data: false,
            max_structured_data_size: 0,
        });
        match convert_with_metadata(&html, conv_options.clone(), cfg) {
            Ok((_md, meta)) => {
                links_out = resolve_links(&final_url, meta.links);
                match convert_with_code_visitor(&html, conv_options) {
                    Ok(md) => markdown = sanitize_markdown(&md),
                    Err(e) => md_err = Some(format!("markdown conversion failed: {e}")),
                }
            }
            Err(e) => {
                md_err = Some(format!("markdown conversion failed: {e}"));
            }
        }
    } else {
        match convert_with_code_visitor(&html, conv_options) {
            Ok(md) => markdown = sanitize_markdown(&md),
            Err(e) => md_err = Some(format!("markdown conversion failed: {e}")),
        }
    }

    // Cache markdown if present.
    let mut cache_rel: Option<String> = None;
    let mut md_bytes = 0usize;
    if md_err.is_none() {
        // Always ensure a trailing newline for POSIX tools.
        if !markdown.ends_with('\n') {
            markdown.push('\n');
        }
        md_bytes = markdown.len();

        let path = cache.page_path(&final_url)?;
        cache.write_atomic(&path, markdown.as_bytes())?;

        // Store relative to cache root.
        let rel = path
            .strip_prefix(cache.root())
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();
        cache_rel = Some(rel);
    }

    // Treat HTTP error status as error but still keep markdown.
    let mut error: Option<String> = md_err;
    if fetch.status.is_client_error() || fetch.status.is_server_error() {
        let status_err = format!("HTTP status {}", status);
        error = Some(match error {
            Some(e) => format!("{status_err}; {e}"),
            None => status_err,
        });
    }

    Ok(PageFetch {
        final_url,
        status,
        content_type,
        bytes: bytes_len,
        markdown_bytes: md_bytes,
        cache_path: cache_rel,
        links: links_out,
        error,
    })
}

fn image_md_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"!\[[^\]]*\]\([^)]+\)").unwrap())
}

fn img_tag_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)<img[^>]*>").unwrap())
}

fn footer_heading_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^#{1,6}\s*footer\b").unwrap())
}

fn link_only_line_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\s*(\[[^\]]+\]\([^)]+\)\s*)+$").unwrap())
}

fn junk_only_line_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[\[\]\(\)\{\}\|\\/\-_.*•·\s]+$").unwrap())
}

fn copyright_line_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^(©|\(c\))\s+.*\b(19|20)\d{2}\b.*$").unwrap())
}

pub fn sanitize_markdown_for_test(input: &str) -> String {
    sanitize_markdown(input)
}

fn sanitize_markdown(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_code = false;
    let mut in_svg = false;
    let mut prev_blank = false;
    let mut skipping_frontmatter = false;
    let mut frontmatter_checked = false;
    let mut in_footer = false;
    let mut saw_content = false;
    let mut saw_heading = false;
    let mut in_trailing_links = false;

    for raw_line in input.lines() {
        let line = raw_line.trim_end();
        let trimmed = line.trim_start();

        if !frontmatter_checked {
            frontmatter_checked = true;
            if trimmed == "---" {
                skipping_frontmatter = true;
                continue;
            }
        }

        if skipping_frontmatter {
            if trimmed == "---" {
                skipping_frontmatter = false;
            }
            continue;
        }

        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_code = !in_code;
            out.push_str(line);
            out.push('\n');
            prev_blank = false;
            continue;
        }

        if !in_code {
            if trimmed.contains("<svg") {
                in_svg = true;
            }
            if in_svg {
                if trimmed.contains("</svg>") {
                    in_svg = false;
                }
                continue;
            }
        }

        if !in_code && footer_heading_regex().is_match(trimmed) {
            in_footer = true;
            continue;
        }

        if !in_code && footer_heading_regex().is_match(line) {
            in_footer = true;
            continue;
        }

        if in_footer {
            if trimmed.starts_with('#') {
                in_footer = false;
            } else {
                continue;
            }
        }

        if !in_code && trimmed.starts_with('#') {
            saw_heading = true;
        }

        if !in_code && !saw_content {
            if trimmed == "---" || trimmed == "***" || trimmed == "___" {
                continue;
            }
            if trimmed.is_empty() {
                continue;
            }
        }

        let mut cleaned = line.to_string();
        if !in_code {
            cleaned = image_md_regex().replace_all(&cleaned, "").to_string();
            cleaned = img_tag_regex().replace_all(&cleaned, "").to_string();
        }

        if !in_code {
            let trimmed = cleaned.trim();
            if !saw_heading && link_only_line_regex().is_match(trimmed) {
                continue;
            }
            if in_trailing_links {
                if trimmed.starts_with('#') {
                    in_trailing_links = false;
                } else if link_only_line_regex().is_match(trimmed) || trimmed.is_empty() {
                    continue;
                } else {
                    in_trailing_links = false;
                }
            }
            if !in_trailing_links && link_only_line_regex().is_match(trimmed) {
                in_trailing_links = true;
                continue;
            }
            if copyright_line_regex().is_match(trimmed) {
                continue;
            }
            if junk_only_line_regex().is_match(trimmed) {
                continue;
            }
            if trimmed == "---" || trimmed == "***" || trimmed == "___" {
                continue;
            }
            if trimmed.eq_ignore_ascii_case("copy")
                || trimmed.eq_ignore_ascii_case("copy page")
                || trimmed.eq_ignore_ascii_case("copied")
            {
                continue;
            }

            if trimmed.contains("[SVG Image]") {
                cleaned = cleaned.replace("[SVG Image]", "");
                if cleaned.trim().is_empty() {
                    continue;
                }
            }
        }

        if cleaned.trim().is_empty() {
            if !prev_blank {
                out.push('\n');
                prev_blank = true;
            }
            continue;
        }

        prev_blank = false;
        saw_content = true;
        out.push_str(cleaned.trim_end());
        out.push('\n');
    }

    out
}

#[derive(Debug)]
struct CodeBlockVisitor {
    code_block_style: CodeBlockStyle,
    default_language: String,
}

impl HtmlVisitor for CodeBlockVisitor {
    fn visit_code_block(&mut self, _ctx: &NodeContext, lang: Option<&str>, code: &str) -> VisitResult {
        let raw = lang.unwrap_or("").trim().to_ascii_lowercase();
        let lang = match raw.as_str() {
            "ts" => "typescript".to_string(),
            "js" => "javascript".to_string(),
            "py" => "python".to_string(),
            "sh" | "shell" => "bash".to_string(),
            _ => raw,
        };

        let lang = if !lang.is_empty() {
            lang
        } else if !self.default_language.is_empty() {
            self.default_language.clone()
        } else {
            String::new()
        };

        let fence = if self.code_block_style == CodeBlockStyle::Tildes {
            "~~~"
        } else {
            "```"
        };
        let mut out = String::new();
        out.push_str(fence);
        if !lang.is_empty() {
            out.push_str(&lang);
        }
        out.push('\n');
        out.push_str(code.trim_matches('\n'));
        out.push('\n');
        out.push_str(fence);
        out.push('\n');
        VisitResult::Custom(out)
    }
}

fn convert_with_code_visitor(html: &str, options: Option<ConversionOptions>) -> Result<String> {
    let options = options.unwrap_or_default();
    let visitor = CodeBlockVisitor {
        code_block_style: options.code_block_style,
        default_language: options.code_language.clone(),
    };
    let handle = std::rc::Rc::new(std::cell::RefCell::new(visitor));
    Ok(convert_with_visitor(html, Some(options), Some(handle))?)
}

pub fn convert_with_code_visitor_for_test(html: &str, options: Option<ConversionOptions>) -> Result<String> {
    convert_with_code_visitor(html, options)
}

fn resolve_links(base: &Url, links: Vec<LinkMetadata>) -> Vec<Url> {
    let mut out = Vec::new();
    for l in links {
        let href = l.href.trim();
        if href.is_empty() {
            continue;
        }

        let abs = if let Ok(u) = Url::parse(href) {
            u
        } else {
            match base.join(href) {
                Ok(u) => u,
                Err(_) => continue,
            }
        };

        let abs = strip_fragment(abs);

        match abs.scheme() {
            "http" | "https" => out.push(abs),
            _ => {}
        }
    }
    out
}

fn canonical_key(url: &Url) -> String {
    let mut u = url.clone();
    u.set_fragment(None);
    u.as_str().to_string()
}

fn path_prefix(root: &Url) -> String {
    let mut p = root.path().to_string();
    if !p.ends_with('/') {
        p.push('/');
    }
    p
}

fn is_allowed_child(url: &Url, allowed_hosts: &HashSet<String>, prefix: &str) -> bool {
    let host = match url.host_str() {
        Some(h) => h.to_ascii_lowercase(),
        None => return false,
    };
    if !allowed_hosts.contains(&host) {
        return false;
    }

    let path = url.path();
    // Accept either exact prefix directory or any child under it.
    if prefix == "/" {
        return true;
    }
    let prefix_no_slash = prefix.trim_end_matches('/');
    path == prefix_no_slash || path.starts_with(prefix)
}

fn read_manifest(path: &Path) -> Result<CrawlManifest> {
    let bytes = fs::read(path).with_context(|| format!("failed to read manifest: {}", path.display()))?;
    let m: CrawlManifest = serde_json::from_slice(&bytes).context("failed to parse manifest JSON")?;
    Ok(m)
}

fn write_manifest(path: &Path, manifest: &CrawlManifest) -> Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).ok();
    }
    let bytes = serde_json::to_vec_pretty(manifest).context("failed to serialize manifest")?;
    fs::write(path, bytes).with_context(|| format!("failed to write manifest: {}", path.display()))?;
    Ok(())
}
