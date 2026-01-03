use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "gg", version, about = "Grep GitHub via grep.app", long_about = None)]
pub(crate) struct Cli {
    /// Pattern to search for
    pub(crate) pattern: Option<String>,

    /// Treat pattern as a literal string (not a regex)
    #[arg(short = 'F', long = "fixed-strings", conflicts_with = "word_regexp")]
    pub(crate) fixed_strings: bool,

    /// Match whole words only
    #[arg(short = 'w', long = "word-regexp", conflicts_with = "fixed_strings")]
    pub(crate) word_regexp: bool,

    /// Ignore case distinctions
    #[arg(short = 'i', long = "ignore-case")]
    pub(crate) ignore_case: bool,

    /// Filter by repository (regex)
    #[arg(long = "repo")]
    pub(crate) repo: Option<String>,

    /// Filter by file path (regex)
    #[arg(long = "path")]
    pub(crate) path: Option<String>,

    /// Filter by language (repeat or comma-separated). Common values: TypeScript, JavaScript, Python, Rust, C++, C, Zig, C#, JSX, TSX, Swift. Use `gg langs` for the full list.
    #[arg(long = "lang", value_delimiter = ',')]
    pub(crate) languages: Vec<String>,

    /// Maximum number of pages to fetch (10 results per page, hard cap 100)
    #[arg(long = "max-pages", default_value_t = 1)]
    pub(crate) max_pages: u32,

    /// Maximum concurrent requests
    #[arg(long = "concurrency", default_value_t = 8)]
    pub(crate) concurrency: usize,

    /// Request timeout in seconds
    #[arg(long = "timeout", default_value_t = 20)]
    pub(crate) timeout_secs: u64,

    /// Emit JSON objects per output line
    #[arg(long = "json", conflicts_with_all = ["matched_repos", "flat"])]
    pub(crate) json: bool,

    /// Emit unique repository names that contain matches
    #[arg(long = "matched-repos", conflicts_with_all = ["json", "flat"])]
    pub(crate) matched_repos: bool,

    /// Include N lines of context around matches (limited to snippet lines)
    #[arg(short = 'C', long = "context", default_value_t = 0)]
    pub(crate) context: usize,

    /// Limit number of output lines (matches + context)
    #[arg(long = "limit")]
    pub(crate) limit: Option<usize>,

    /// Disable ANSI colors
    #[arg(long = "no-color")]
    pub(crate) no_color: bool,

    /// Group results by repo and file (default).
    #[arg(long = "heading", default_value_t = true)]
    pub(crate) heading: bool,

    /// Disable grouped output (flat repo/path:line:content)
    #[arg(long = "flat", conflicts_with_all = ["json", "matched_repos"])]
    pub(crate) flat: bool,

    /// Override API base URL (for tests)
    #[arg(long = "base-url", default_value = "https://grep.app", hide = true)]
    pub(crate) base_url: String,
}

#[cfg(test)]
mod tests {
    use super::Cli;
    use clap::Parser;

    #[test]
    fn fixed_strings_conflicts_with_word_regexp() {
        let res = Cli::try_parse_from(["gg", "-F", "-w", "TODO"]);
        assert!(res.is_err());
    }
}
