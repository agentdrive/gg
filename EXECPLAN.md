# Build Rust grep.app SDK + gg CLI

This ExecPlan is a living document. The sections `Progress`, `Surprises & Discoveries`, `Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work proceeds.

`PLANS.md` was not found at the repository root when this ExecPlan was created; this document follows `.agents/EXECPLAN.md` guidance.

## Purpose / Big Picture

Deliver a Rust SDK and a `gg` CLI that can search millions of GitHub repositories through the public `grep.app` API. A user should be able to run a command like `gg "TODO" --repo "rust-lang/.*"` and immediately see matching lines (similar to ripgrep output), or consume the SDK in code to fetch parsed matches quickly and concurrently. Success is demonstrated by running `cargo test` and then running `cargo run --bin gg -- "TODO" --max-pages 1` to see formatted results without errors.

## Progress

- [x] (2026-01-01 15:18Z) Read AGENTS.md and `.agents/EXECPLAN.md`; inspected grep.app API sample response and grepgithub baseline script.
- [x] (2026-01-01 15:20Z) Create spec + test stubs and initialize ExecPlan artifacts.
- [x] (2026-01-01 15:30Z) Implement Rust SDK (client, models, parsing, concurrency) and gg CLI.
- [x] (2026-01-01 15:30Z) Update AGENTS.md and README with repo purpose, usage, and structure.
- [x] (2026-01-01 15:30Z) Run formatting/tests and record validation artifacts.
- [ ] (2026-01-01 18:30Z) Add language catalog + CLI command, default grouped output/max-pages updates, matched-repos output, docs/tests updates, and rerun validation.

## Surprises & Discoveries

- Observation: The current grep.app API response uses `repo` and `path` as strings in hits (not `repo.raw`/`path.raw` as older scripts expect).
  Evidence: `curl -s 'https://grep.app/api/search?q=TODO&page=1' | jq '.hits.hits[0] | keys'` returns `repo`, `path`, `content` etc.
- Observation: HTML snippet parsing via a DOM parser returned no rows for the test fragment; regex-based extraction proved reliable for the API snippet format.
  Evidence: `snippet::tests::parses_snippet_with_marks` failed until switching to regex-based `<tr>`/`<pre>` parsing.
- Observation: `total_matches` arrives as a string and may include a trailing `+` (e.g., `"100+"`).
  Evidence: `curl -s 'https://grep.app/api/search?q=TODO&page=1' | jq -r '.hits.hits[0].total_matches'` returned `100+`.
- Observation: grep.app snippets include non-matching lines, which can be used for context output.
  Evidence: Snippet parsing now retains non-marked lines for `gg -C` context rendering.
- Observation: grep.app API responses include a `facets.lang` list of language names; GitHub Linguist provides a superset list that matches these labels.
  Evidence: `curl -s 'https://grep.app/api/search?q=todo&page=1' | jq -r '.facets.lang.buckets | map(.val)[:8][]'` matches Linguist names (e.g., `Python`, `C++`, `Go`).

## Decision Log

- Decision: Build a single Rust crate named `grepapp` with a library + `gg` binary (`src/bin/gg.rs`) instead of a multi-crate workspace.
  Rationale: Keeps the repo minimal and easy to consume; Cargo supports library + binary in one package.
  Date/Author: 2026-01-01 / Codex

- Decision: Parse HTML snippets into line numbers + match ranges and use ANSI coloring in CLI rather than printing raw HTML.
  Rationale: Produces grep-like output and decouples SDK data from UI formatting.
  Date/Author: 2026-01-01 / Codex

- Decision: Use regex-based snippet extraction (`<tr>`, `<pre>`, `<div class="lineno">`) instead of a DOM parser.
  Rationale: The API snippet format is stable and regex parsing avoided empty-node issues in tests.
  Date/Author: 2026-01-01 / Codex

- Decision: Use GitHub Linguist language list as the canonical filter values for `--lang`, exposed via `gg langs`.
  Rationale: grep.app facet values align with Linguist naming; storing the list locally avoids network calls and keeps SDK/CLI deterministic.
  Date/Author: 2026-01-01 / Codex

- Decision: Default CLI output to grouped headings and `--max-pages 1`, with `--flat` for legacy output format.
  Rationale: Matches requested defaults while keeping a single-flag opt-out for flat output.
  Date/Author: 2026-01-01 / Codex

- Decision: Implement `--limit` as a CLI output cap (matches + context) while keeping `--max-pages` as an API pagination bound.
  Rationale: Provides predictable output size without changing API semantics.
  Date/Author: 2026-01-01 / Codex

- Decision: Provide context lines by parsing non-matching snippet lines and filtering around matches client-side.
  Rationale: The public API only returns snippet blocks; local filtering is the safest way to show context.
  Date/Author: 2026-01-01 / Codex

## Outcomes & Retrospective

Delivered a Rust SDK and `gg` CLI backed by the grep.app API, with concurrent page fetching, snippet parsing, and ripgrep-like output. Added specs, docs, and tests; `cargo fmt` and `cargo test` complete cleanly. Remaining work is optional polish (e.g., richer output formatting or retry policy tweaks).

## Context and Orientation

The repository now contains a Rust library crate (`Cargo.toml`, `src/lib.rs`) plus the SDK and CLI implementation:

- `src/lib.rs` plus supporting modules (`client.rs`, `query.rs`, `models.rs`, `snippet.rs`).
- `src/bin/gg.rs` CLI entrypoint using the library.
- `README.md` for usage.
- `specs/grepapp_sdk_cli.md` and `tests/unit/test_grepapp_sdk_cli.py` as specification artifacts.
- `AGENTS.md` updated to reflect the new codebase.

Key terms:

- “grep.app API”: a public HTTPS endpoint at `https://grep.app/api/search` that accepts query parameters such as `q` and `page` and returns JSON with hits and HTML snippets.
- “snippet”: HTML table of matched lines returned by the API; must be parsed to plain text with match ranges.

## Plan of Work

Add a spec first, then implement the Rust SDK and CLI. Extend `Cargo.toml` with async HTTP, JSON parsing, snippet parsing, and CLI dependencies. Implement a `GrepAppClient` with methods to build query parameters and fetch pages concurrently. Implement snippet parsing to extract line numbers and match ranges. Add a CLI (`gg`) that wires arguments into `SearchQuery` and outputs matches in a ripgrep-like format with optional JSON and grouped output. Update `AGENTS.md` and `README.md` to reflect purpose, structure, and usage. Finally, run formatting and tests, and capture outputs in `Artifacts and Notes`.

## Concrete Steps

Run these commands from the repository root (`/Users/tomasroda/tools/grepapp`):

  1) Create spec + test stubs
     - `mkdir -p specs tests/unit`
     - Write `specs/grepapp_sdk_cli.md` and `tests/unit/test_grepapp_sdk_cli.py`

  2) Update Cargo and source tree
     - Edit `Cargo.toml` to add dependencies (`tokio`, `reqwest`, `serde`, `serde_json`, `clap`, `regex`, `html-escape`, `once_cell`, `thiserror`, `futures`).
     - Create `src/bin/gg.rs` and modules under `src/` for client, models, parsing, and output.

  3) Update docs
     - Edit `AGENTS.md` and add `README.md`.

  4) Validate
     - `cargo fmt`
     - `cargo test`
     - `cargo run --bin gg -- "TODO" --max-pages 1`

## Validation and Acceptance

Acceptance is met when:

- `cargo test` completes with no failures.
- `cargo run --bin gg -- "TODO" --max-pages 1` outputs at least one match line in `repo/path:line:content` form (or `--json` emits JSON lines) without crashing.
- The library exposes a `GrepAppClient` and `SearchQuery` that can be used to fetch results programmatically.

## Idempotence and Recovery

All steps are additive and safe to re-run. If a build fails after dependency changes, re-run `cargo check` to surface missing imports, then retry `cargo fmt` and `cargo test`. If HTTP calls fail due to network or rate limits, re-run the CLI with `--max-pages 1` to confirm basic functionality.

## Artifacts and Notes

  cargo test (2026-01-01):
    running 2 tests
    test query::tests::builds_query_pairs_with_filters ... ok
    test snippet::tests::parses_snippet_with_marks ... ok

  cargo clippy -- -D warnings (2026-01-01):
    Finished with no warnings.

  cargo test (2026-01-01, after context/limit updates):
    running 2 tests
    test query::tests::builds_query_pairs_with_filters ... ok
    test snippet::tests::parses_snippet_with_marks ... ok

  cargo run --bin gg -- "TODO" --max-pages 1 --no-color (2026-01-01):
    JetBrains/kotlin/.../package_root.kt:2:    val property: T get() = TODO()
    git/git/sequencer.c:783:    * TODO: merge_switch_to_result will update index/working tree;

  cargo run --bin gg -- "TODO" --max-pages 1 --no-color --limit 5 -C 1 (2026-01-01):
    JetBrains/kotlin/.../package_root.kt:2:    val property: T get() = TODO()
    JetBrains/kotlin/.../package_root.kt:3:    fun function(value: T): T = value

## Interfaces and Dependencies

Public library surface:

- `pub struct GrepAppClient` in `src/lib.rs` with:
  - `pub fn new() -> Self`
  - `pub fn with_base_url(base_url: Url) -> Self`
  - `pub fn with_timeout(self, timeout: Duration) -> Self`
  - `pub async fn search(&self, query: &SearchQuery, options: &SearchOptions) -> Result<SearchResult, GrepAppError>`
  - `pub async fn search_page(&self, query: &SearchQuery, page: u32) -> Result<SearchPage, GrepAppError>`
- `pub struct SearchQuery` with fields for `pattern`, `regex`, `whole_words`, `case_sensitive`, `repo_filter`, `path_filter`, `languages`.
- `pub struct SearchOptions` with `max_pages`, `concurrency`, `timeout: Option<Duration>`.
- `pub struct SearchResult` containing `total`, `hits: Vec<SearchHit>`.
- `pub struct SearchHit` containing `repo`, `path`, `branch`, `lines: Vec<LineMatch>`.
- `pub struct LineMatch` containing `line_number`, `line_text`, `match_ranges`.
- `pub enum GrepAppError` for HTTP, JSON, and snippet parsing failures.

Dependencies: `tokio`, `reqwest`, `serde`, `serde_json`, `clap`, `regex`, `html-escape`, `once_cell`, `thiserror`, `futures`.

---

Change note (2026-01-01): Created initial ExecPlan for SDK + CLI delivery, based on `.agents/EXECPLAN.md` template.
Change note (2026-01-01): Marked spec/test stubs as completed in Progress after creating `specs/grepapp_sdk_cli.md` and `tests/unit/test_grepapp_sdk_cli.py`.
Change note (2026-01-01): Updated plan sections to reflect implemented SDK/CLI, regex-based snippet parsing, API quirks, and recorded validation outputs for `cargo test` and `gg` execution.
Change note (2026-01-01): Added clippy validation and updated client pagination to satisfy clippy warnings.
Change note (2026-01-01): Added CLI limit/context decisions, updated snippet parsing to keep non-match lines, and recorded validation reruns.
