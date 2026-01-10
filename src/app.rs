use std::{
    env,
    path::PathBuf,
    process::Command,
};

use anyhow::{anyhow, Context, Result};
use url::Url;

use crate::{
    cache::Cache,
    crawl::{self, CrawlOptions},
    http::HttpOptions,
    urlspec::{SourceSpec, UrlPattern},
    util::{is_url_like, split_comma_separated},
};

#[derive(Debug, Clone)]
struct GgOptions {
    refresh: bool,
    cache_dir: Option<PathBuf>,
    parallelism: Option<usize>,
    max_depth: Option<usize>,
    use_sitemap: bool,
    timeout_secs: Option<u64>,
    connect_timeout_secs: Option<u64>,
    max_body_mib: Option<usize>,
    user_agent: Option<String>,
    cmd_override: Option<String>,
    print_paths: bool,
    force_crawl: bool,
    force_page: bool,
}

impl Default for GgOptions {
    fn default() -> Self {
        Self {
            refresh: false,
            cache_dir: None,
            parallelism: None,
            max_depth: None,
            use_sitemap: true,
            timeout_secs: None,
            connect_timeout_secs: None,
            max_body_mib: None,
            user_agent: None,
            cmd_override: None,
            print_paths: false,
            force_crawl: false,
            force_page: false,
        }
    }
}

pub async fn run() -> Result<()> {
    let argv: Vec<String> = env::args().skip(1).collect();
    let (opts, remaining) = parse_gg_flags(argv)?;

    if remaining.is_empty() {
        print_help();
        return Err(anyhow!("missing URL"));
    }

    let first_url_idx = remaining
        .iter()
        .position(|t| is_url_like(t))
        .ok_or_else(|| anyhow!("missing URL"))?;

    let host_part = &remaining[..first_url_idx];
    let url_part = &remaining[first_url_idx..];

    let (host_cmd, host_args) = resolve_host_invocation(host_part, opts.cmd_override.clone())?;

    let cache = Cache::new(opts.cache_dir.clone())?;

    let mut http_opts = HttpOptions::default();
    if let Some(ua) = opts.user_agent.clone() {
        http_opts.user_agent = ua;
    }
    if let Some(secs) = opts.timeout_secs {
        http_opts.timeout = std::time::Duration::from_secs(secs);
    }
    if let Some(secs) = opts.connect_timeout_secs {
        http_opts.connect_timeout = std::time::Duration::from_secs(secs);
    }
    if let Some(mib) = opts.max_body_mib {
        http_opts.max_body_bytes = mib * 1024 * 1024;
    }

    let parallelism = opts
        .parallelism
        .unwrap_or_else(default_parallelism)
        .clamp(1, 512);

    let crawl_opts = CrawlOptions {
        http: http_opts,
        parallelism,
        max_depth: opts.max_depth,
        use_sitemap: opts.use_sitemap,
    };

    // Parse URL arguments into source specs.
    let mut sources: Vec<SourceSpec> = Vec::new();
    for tok in url_part {
        for piece in split_comma_separated(tok) {
            sources.push(parse_source(&piece, opts.force_crawl, opts.force_page)?);
        }
    }

    // Resolve sources into local file/dir paths.
    let mut local_targets: Vec<PathBuf> = Vec::new();

    // Shared client for single-page fetches.
    let client_all = crate::http::build_client_all(&crawl_opts.http)?;

    for spec in sources {
        match spec {
            SourceSpec::Page(url) => {
                let url_for_err = url.clone();
                let path = crawl::ensure_page_cached(
                    &cache,
                    &client_all,
                    &crawl_opts,
                    url,
                    opts.refresh,
                )
                .await
                .with_context(|| format!("failed to fetch {url_for_err}"))?;
                local_targets.push(path);
            }
            SourceSpec::CrawlRoot(root) => {
                let root_for_err = root.clone();
                let manifest = crawl::ensure_subtree_cached(&cache, &crawl_opts, root, opts.refresh)
                    .await
                    .with_context(|| format!("failed to crawl {root_for_err}"))?;
                // For a crawl root, pass the directory itself to the host command.
                let dir = cache.subtree_dir(&root_for_err)?;
                if manifest.pages.is_empty() {
                    // Still pass dir; user can see emptiness.
                }
                local_targets.push(dir);
            }
            SourceSpec::Pattern(pat) => {
                let manifest =
                    crawl::ensure_subtree_cached(&cache, &crawl_opts, pat.root.clone(), opts.refresh)
                    .await
                    .with_context(|| format!("failed to crawl {root}", root = pat.root))?;

                // Fast path: a whole-subtree pattern like .../**/*.
                if pat.is_subtree_pattern() {
                    local_targets.push(cache.subtree_dir(&pat.root)?);
                    continue;
                }

                for page in &manifest.pages {
                    if pat.matches_url_string(&page.url) {
                        local_targets.push(cache.root().join(&page.cache_path));
                    }
                }
            }
        }
    }

    // Deduplicate targets (stable order).
    local_targets = dedupe_paths(local_targets);

    if opts.print_paths {
        for p in &local_targets {
            println!("{}", p.display());
        }
        return Ok(());
    }

    // Execute the host command.
    let mut cmd = Command::new(&host_cmd);
    cmd.args(&host_args);
    cmd.args(local_targets.iter().map(|p| p.as_os_str()));

    let status = cmd
        .status()
        .with_context(|| format!("failed to execute host command: {host_cmd}"))?;

    match status.code() {
        Some(code) => std::process::exit(code),
        None => {
            // Terminated by signal.
            std::process::exit(128);
        }
    }
}

fn parse_source(s: &str, force_crawl: bool, force_page: bool) -> Result<SourceSpec> {
    if !force_page && UrlPattern::has_glob(s) {
        return Ok(SourceSpec::Pattern(UrlPattern::new(s)?));
    }

    let url = Url::parse(s).with_context(|| format!("invalid URL: {s}"))?;

    if force_crawl && !force_page {
        return Ok(SourceSpec::CrawlRoot(url));
    }

    if !force_page && s.trim_end().ends_with('/') {
        return Ok(SourceSpec::CrawlRoot(url));
    }

    Ok(SourceSpec::Page(url))
}

fn default_parallelism() -> usize {
    let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    // Empirically, crawling tends to be network-bound; use more than core count.
    (cores * 8).max(16)
}

fn dedupe_paths(mut paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = std::collections::HashSet::new();
    paths.retain(|p| seen.insert(p.clone()));
    paths
}

fn resolve_host_invocation(host_part: &[String], cmd_override: Option<String>) -> Result<(String, Vec<String>)> {
    if let Some(cmd) = cmd_override {
        return Ok((cmd, host_part.to_vec()));
    }

    if host_part.is_empty() {
        return Ok(("rg".to_string(), Vec::new()));
    }

    let first = &host_part[0];

    if first == "--" {
        return Ok(("rg".to_string(), host_part[1..].to_vec()));
    }

    if first.starts_with('-') {
        return Ok(("rg".to_string(), host_part.to_vec()));
    }

    // Heuristic: if the first token resolves to an executable in PATH, treat it as the host command.
    if is_executable_in_path(first) {
        return Ok((first.clone(), host_part[1..].to_vec()));
    }

    // Otherwise: treat everything as args to default `rg`.
    Ok(("rg".to_string(), host_part.to_vec()))
}

fn is_executable_in_path(cmd: &str) -> bool {
    // Absolute or relative path with a separator.
    if cmd.contains(std::path::MAIN_SEPARATOR) {
        return std::fs::metadata(cmd).is_ok();
    }

    let path = match env::var_os("PATH") {
        Some(p) => p,
        None => return false,
    };

    for dir in env::split_paths(&path) {
        let candidate = dir.join(cmd);
        if candidate.is_file() {
            return true;
        }
    }

    false
}

fn parse_gg_flags(argv: Vec<String>) -> Result<(GgOptions, Vec<String>)> {
    let mut opts = GgOptions::default();
    let mut remaining: Vec<String> = Vec::new();

    let mut i = 0;
    while i < argv.len() {
        let t = &argv[i];

        if t == "--" {
            remaining.extend(argv[i + 1..].iter().cloned());
            break;
        }

        match t.as_str() {
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            "-V" | "--version" => {
                println!("gg {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            "--refresh" => {
                opts.refresh = true;
                i += 1;
            }
            "--cache-dir" => {
                let v = argv
                    .get(i + 1)
                    .ok_or_else(|| anyhow!("--cache-dir requires a value"))?;
                opts.cache_dir = Some(PathBuf::from(v));
                i += 2;
            }
            "--parallelism" => {
                let v = argv
                    .get(i + 1)
                    .ok_or_else(|| anyhow!("--parallelism requires a value"))?;
                opts.parallelism = Some(v.parse::<usize>().context("invalid --parallelism")?);
                i += 2;
            }
            "--max-depth" => {
                let v = argv
                    .get(i + 1)
                    .ok_or_else(|| anyhow!("--max-depth requires a value"))?;
                let n = v.parse::<usize>().context("invalid --max-depth")?;
                opts.max_depth = Some(n);
                i += 2;
            }
            "--no-sitemap" => {
                opts.use_sitemap = false;
                i += 1;
            }
            "--sitemap" => {
                opts.use_sitemap = true;
                i += 1;
            }
            "--timeout" => {
                let v = argv
                    .get(i + 1)
                    .ok_or_else(|| anyhow!("--timeout requires a value"))?;
                opts.timeout_secs = Some(v.parse::<u64>().context("invalid --timeout")?);
                i += 2;
            }
            "--connect-timeout" => {
                let v = argv
                    .get(i + 1)
                    .ok_or_else(|| anyhow!("--connect-timeout requires a value"))?;
                opts.connect_timeout_secs =
                    Some(v.parse::<u64>().context("invalid --connect-timeout")?);
                i += 2;
            }
            "--max-body-mib" => {
                let v = argv
                    .get(i + 1)
                    .ok_or_else(|| anyhow!("--max-body-mib requires a value"))?;
                opts.max_body_mib = Some(v.parse::<usize>().context("invalid --max-body-mib")?);
                i += 2;
            }
            "--user-agent" => {
                let v = argv
                    .get(i + 1)
                    .ok_or_else(|| anyhow!("--user-agent requires a value"))?;
                opts.user_agent = Some(v.to_string());
                i += 2;
            }
            "--cmd" => {
                let v = argv
                    .get(i + 1)
                    .ok_or_else(|| anyhow!("--cmd requires a value"))?;
                opts.cmd_override = Some(v.to_string());
                i += 2;
            }
            "--print-paths" => {
                opts.print_paths = true;
                i += 1;
            }
            "--crawl" => {
                opts.force_crawl = true;
                i += 1;
            }
            "--page" => {
                opts.force_page = true;
                i += 1;
            }
            _ => {
                remaining.push(t.clone());
                i += 1;
            }
        }
    }

    Ok((opts, remaining))
}

fn print_help() {
    let help = r#"gg - filesystem-like interface to the web (Rust)

USAGE:
  gg [GG_FLAGS] [HOST_CMD [HOST_ARGS...]] URL_OR_GLOB [URL_OR_GLOB ...]

DATA SOURCES:
  - A single URL (no globs) fetches just that page and caches it as Markdown.
  - A URL ending with '/' is treated as a crawl root (subtree crawl).
  - A URL containing glob characters (* ? [) is treated as a pattern; gg crawls
    the pattern's root and then selects only matching pages.
  - A single argument may be a comma-separated list of URLs.

DEFAULT HOST COMMAND:
  If HOST_CMD is omitted, gg defaults to 'rg'.

GG FLAGS:
  --refresh               Re-fetch / re-crawl even if cache exists
  --cache-dir <DIR>       Override cache directory (also: GG_CACHE_DIR)
  --parallelism <N>       Concurrent fetches while crawling
  --max-depth <N>         Limit crawl depth (0-based); omitted = unlimited
  --no-sitemap            Disable sitemap seeding
  --timeout <SECS>        Request timeout
  --connect-timeout <SECS>Connect timeout
  --max-body-mib <N>      Maximum bytes per HTML page (MiB)
  --user-agent <UA>       Override User-Agent
  --cmd <CMD>             Force host command (disambiguation)
  --print-paths           Print resolved local paths instead of running command
  --crawl                 Force subtree crawl for non-glob URLs
  --page                  Force single-page mode even if URL ends with '/'

EXAMPLES:
  gg -i "pattern" https://example.com/docs/**/*
  gg tree https://example.com/docs/**/*
  gg cat https://example.com/docs/getting-started
"#;
    eprintln!("{help}");
}
