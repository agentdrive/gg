use std::{fs, io::Write, path::{Path, PathBuf}};

use anyhow::{anyhow, Context, Result};
use blake3::Hasher;
use directories::ProjectDirs;
use url::Url;

#[derive(Debug, Clone)]
pub struct Cache {
    root: PathBuf,
}

impl Cache {
    pub fn new(root_override: Option<PathBuf>) -> Result<Self> {
        let root = if let Some(r) = root_override {
            r
        } else if let Ok(env) = std::env::var("GG_CACHE_DIR") {
            PathBuf::from(env)
        } else {
            let proj = ProjectDirs::from("dev", "gg", "gg")
                .ok_or_else(|| anyhow!("unable to determine default cache directory"))?;
            proj.cache_dir().to_path_buf()
        };

        fs::create_dir_all(&root).with_context(|| format!("failed to create cache dir: {}", root.display()))?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Base directory for a site, e.g. `.../sites/https/example.com`.
    pub fn site_dir(&self, url: &Url) -> Result<PathBuf> {
        let scheme = url.scheme();
        let host = url.host_str().ok_or_else(|| anyhow!("URL has no host: {url}"))?;
        let host_dir = host_port_dirname(url, host);
        Ok(self.root.join("sites").join(scheme).join(host_dir))
    }

    /// Directory root for a crawled subtree (URL ending in '/'), e.g.
    /// `.../sites/https/example.com/docs` for `https://example.com/docs/`.
    pub fn subtree_dir(&self, root: &Url) -> Result<PathBuf> {
        let mut dir = self.site_dir(root)?;
        let path = root.path();
        if path == "/" {
            return Ok(dir);
        }
        for seg in path.trim_matches('/').split('/') {
            if seg.is_empty() {
                continue;
            }
            dir = dir.join(sanitize_component(seg));
        }
        Ok(dir)
    }

    /// File path for a single page URL. For URLs ending with '/', returns the
    /// corresponding `index.md` inside the subtree directory.
    pub fn page_path(&self, url: &Url) -> Result<PathBuf> {
        let site_dir = self.site_dir(url)?;
        let path = url.path();

        if path == "/" || path.is_empty() {
            return Ok(site_dir.join("index.md"));
        }

        let ends_with_slash = path.ends_with('/');
        let segments: Vec<&str> = path.trim_matches('/').split('/').filter(|s| !s.is_empty()).collect();

        let mut dir = site_dir;
        if ends_with_slash {
            for seg in segments {
                dir = dir.join(sanitize_component(seg));
            }
            return Ok(dir.join("index.md"));
        }

        if segments.is_empty() {
            return Ok(dir.join("index.md"));
        }

        for seg in &segments[..segments.len() - 1] {
            dir = dir.join(sanitize_component(seg));
        }

        let last = segments[segments.len() - 1];
        let mut base = strip_html_ext(last);
        if base.is_empty() {
            base = "index".to_string();
        }
        let mut filename = sanitize_component(&base);

        if let Some(q) = url.query() {
            let mut h = Hasher::new();
            h.update(q.as_bytes());
            let digest = h.finalize();
            let short = &digest.to_hex()[..8];
            filename.push_str("__q");
            filename.push_str(short);
        }

        filename.push_str(".md");
        Ok(dir.join(filename))
    }

    pub fn manifest_path_for_subtree(&self, root: &Url) -> Result<PathBuf> {
        let dir = self.subtree_dir(root)?;
        Ok(dir.join(".gg").join("manifest.json"))
    }

    pub fn is_cached_file(&self, path: &Path) -> bool {
        path.is_file()
    }

    pub fn write_atomic(&self, path: &Path, bytes: &[u8]) -> Result<()> {
        let parent = path.parent().ok_or_else(|| anyhow!("path has no parent: {}", path.display()))?;
        fs::create_dir_all(parent).with_context(|| format!("failed to create dir: {}", parent.display()))?;

        let file_name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("out.md");
        let tmp_name = format!(".{file_name}.tmp");
        let tmp_path = parent.join(tmp_name);

        {
            let mut f = fs::File::create(&tmp_path)
                .with_context(|| format!("failed to create temp file: {}", tmp_path.display()))?;
            f.write_all(bytes)
                .with_context(|| format!("failed to write temp file: {}", tmp_path.display()))?;
            f.flush().ok();
        }

        fs::rename(&tmp_path, path).with_context(|| {
            format!(
                "failed to replace {} with temp file {}",
                path.display(),
                tmp_path.display()
            )
        })?;
        Ok(())
    }
}

fn host_port_dirname(url: &Url, host: &str) -> String {
    let host_l = host.to_ascii_lowercase();
    let scheme = url.scheme();
    let default_port = match scheme {
        "http" => Some(80),
        "https" => Some(443),
        _ => None,
    };

    let explicit_port = url.port();
    if let (Some(p), Some(def)) = (explicit_port, default_port) {
        if p != def {
            return format!("{host_l}_port{p}");
        }
    } else if let Some(p) = explicit_port {
        return format!("{host_l}_port{p}");
    }

    host_l
}

fn strip_html_ext(s: &str) -> String {
    for ext in [".html", ".htm", ".xhtml"] {
        if s.to_ascii_lowercase().ends_with(ext) {
            return s[..s.len() - ext.len()].to_string();
        }
    }
    s.to_string()
}

pub fn sanitize_component(s: &str) -> String {
    // Keep a conservative character set; percent-encode the rest.
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        match b {
            b'a'..=b'z'
            | b'A'..=b'Z'
            | b'0'..=b'9'
            | b'-'
            | b'_' 
            | b'.' => out.push(b as char),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}
