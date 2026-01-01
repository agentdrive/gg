# grepapp

Rust SDK + CLI for searching GitHub repositories through `https://grep.app`.

## CLI (gg)

Run directly from the repo:

```bash
cargo run --bin gg -- "TODO" --max-pages 1
```

Common options:

```bash
# Print supported language names
cargo run --bin gg -- langs

# Regex search
cargo run --bin gg -- "serde_json::from_str" -r --max-pages 2

# Repo + path filters
cargo run --bin gg -- "TODO" --repo "rust-lang/.*" --path "src/.*"

# Language filter (repeat or comma-separated). Common values: TypeScript, JavaScript, Python, Rust, C++, C, Zig, C#, JSX, TSX, Swift. Use `gg langs` to list valid names.
cargo run --bin gg -- "TODO" --lang Rust --lang Go
cargo run --bin gg -- "TODO" --lang Rust,Go

# JSON output
cargo run --bin gg -- "TODO" --json --max-pages 1

# JSON includes `is_match` to distinguish context lines when -C/--context is used.

# Limit output lines
cargo run --bin gg -- "TODO" --limit 20 --max-pages 2

# Context lines (limited to snippet lines returned by grep.app)
cargo run --bin gg -- "TODO" -C 2 --max-pages 1

# Ignore case
cargo run --bin gg -- "todo" -i --max-pages 1

# Group by repo and file (default)
cargo run --bin gg -- "TODO" --heading --max-pages 1

# Flat output (repo/path:line:content)
cargo run --bin gg -- "TODO" --flat --max-pages 1

# Emit only matched repository names (deduplicated)
cargo run --bin gg -- "TODO" --matched-repos --max-pages 1
```

Install locally:

```bash
cargo install --path .
```

## SDK usage

```rust
use grepapp::{GrepAppClient, SearchOptions, SearchQuery};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = GrepAppClient::new();
    let langs = grepapp::languages()?;
    println!("Supported languages: {}", langs.len());
    let query = SearchQuery::new("TODO")
        .repo_filter("rust-lang/.*")
        .case_sensitive(true);
    let options = SearchOptions::default()
        .max_pages(2)
        .concurrency(4)
        .timeout(Duration::from_secs(15));

    let result = client.search(&query, &options).await?;
    println!("Total matches: {}", result.total);
    Ok(())
}
```

## Notes

- Each API page returns up to 10 hits; `--max-pages` defaults to 1 and is capped at 100.
- Match highlighting is derived from `grep.app` snippet HTML and rendered locally.
- The CLI defaults to grouped output (`--heading`) for easier scanning.
