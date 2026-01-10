use anyhow::{anyhow, Context, Result};
use regex::Regex;
use url::Url;

use crate::util::is_url_like;

#[derive(Debug, Clone)]
pub enum SourceSpec {
    /// Fetch a single HTML page and convert to Markdown.
    Page(Url),
    /// Crawl an entire subtree rooted at `root`.
    CrawlRoot(Url),
    /// Crawl `root` (if needed) and then select only URLs matching the pattern.
    Pattern(UrlPattern),
}

#[derive(Debug, Clone)]
pub struct UrlPattern {
    pub original: String,
    pub root: Url,
    regex: Regex,
}

impl UrlPattern {
    pub fn new(pattern: &str) -> Result<Self> {
        if !contains_glob(pattern) {
            return Err(anyhow!("pattern has no glob characters"));
        }
        let root = pattern_root(pattern)?;
        let regex = compile_glob_url_regex(pattern)?;
        Ok(Self {
            original: pattern.to_string(),
            root,
            regex,
        })
    }

    pub fn has_glob(s: &str) -> bool {
        contains_glob(s)
    }

    pub fn is_subtree_pattern(&self) -> bool {
        let p = self.original.as_str();
        p.ends_with("/**/*") || p.ends_with("/**/*.*") || p.ends_with("/**")
    }

    pub fn matches_url_string(&self, url: &str) -> bool {
        match Url::parse(url) {
            Ok(u) => self.matches(&u),
            Err(_) => false,
        }
    }

    pub fn matches(&self, url: &Url) -> bool {
        // Match against the URL string without fragment.
        let mut u = url.clone();
        u.set_fragment(None);
        self.regex.is_match(u.as_str())
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SourceParseOpts {
    /// If true, treat non-glob URLs as crawl roots.
    pub force_crawl: bool,
    /// If true, treat URLs that end with `/` as single pages (override default).
    pub force_page: bool,
}

/// Parse a single URL/token into a `SourceSpec`.
pub fn parse_source_token(token: &str, opts: SourceParseOpts) -> Result<SourceSpec> {
    let t = token.trim();
    if !is_url_like(t) {
        return Err(anyhow!("not a URL: {t}"));
    }

    if contains_glob(t) {
        return Ok(SourceSpec::Pattern(UrlPattern::new(t)?));
    }

    let url = Url::parse(t).with_context(|| format!("invalid URL: {t}"))?;

    if opts.force_page {
        return Ok(SourceSpec::Page(url));
    }

    // Heuristic: URLs ending with '/' represent a directory root to crawl.
    if opts.force_crawl || t.ends_with('/') {
        Ok(SourceSpec::CrawlRoot(url))
    } else {
        Ok(SourceSpec::Page(url))
    }
}

fn contains_glob(s: &str) -> bool {
    s.contains('*') || s.contains('?') || s.contains('[')
}

/// Determine the crawl root of a glob URL by taking everything up to the last '/' before the
/// first glob character.
fn pattern_root(pattern: &str) -> Result<Url> {
    let first_glob = pattern
        .char_indices()
        .find(|(_, c)| matches!(c, '*' | '?' | '['))
        .map(|(i, _)| i)
        .context("glob pattern is missing wildcard")?;

    let upto = &pattern[..first_glob];
    let slash_pos = upto
        .rfind('/')
        .ok_or_else(|| anyhow!("glob pattern has no '/' before wildcard"))?;

    let mut root_str = &pattern[..=slash_pos];
    // Ensure the root parses as a valid URL.
    // `Url::parse` expects the path to be present; scheme+host-only is fine but we normalize to '/'.
    if root_str.ends_with("://") {
        root_str = pattern;
    }

    Url::parse(root_str).with_context(|| format!("failed to parse crawl root from pattern: {root_str}"))
}

/// Convert a glob URL pattern into a regex that matches full URLs.
///
/// Supported glob syntax:
/// - `*` matches any characters except '/'
/// - `**` matches any characters (including '/')
/// - `?` matches a single character except '/'
/// - Character classes like `[abc]` are passed through (best-effort)
fn compile_glob_url_regex(pattern: &str) -> Result<Regex> {
    let mut out = String::with_capacity(pattern.len() * 2);
    out.push('^');

    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '*' => {
                if chars.peek() == Some(&'*') {
                    chars.next();
                    out.push_str(".*");
                } else {
                    out.push_str("[^/]*");
                }
            }
            '?' => out.push_str("[^/]{1}"),
            '[' => {
                // Best effort: copy until closing ']' without interpreting.
                out.push('[');
                for nc in chars.by_ref() {
                    out.push(nc);
                    if nc == ']' {
                        break;
                    }
                }
            }
            // Escape regex metacharacters.
            '.' | '+' | '(' | ')' | '|' | '^' | '$' | '{' | '}' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }

    out.push('$');
    Regex::new(&out).with_context(|| format!("failed to compile regex from pattern: {pattern}"))
}
