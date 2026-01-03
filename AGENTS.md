# AGENTS.md

## INDEX
Other relevant files that must be read before the start of a conversation turn:
* `.agents/EXECPLAN.md`

## PURPOSE
This repository provides a Rust SDK and a `gg` CLI for searching GitHub repositories via the public `https://grep.app` API. It aims to feel like ripgrep while operating over remote code.

## STYLE GUIDANCE
- Keep library APIs small and strongly typed.
- Avoid panics in library code; return `GrepAppError` instead.
- Prefer pure functions for parsing (easy unit tests).
- Use `cargo fmt` to keep formatting consistent.
- CLI errors must be clear and exit non-zero.

## TECHNICAL INFO
- Language: Rust (edition 2024)
- Runtime: `tokio` (multi-threaded)
- HTTP: `reqwest` (rustls)
- CLI: `clap`
- Parsing: `serde`, `serde_json`, `regex`, `html-escape`, `once_cell`
- Errors: `thiserror`

## ORGANIZATION

```
.
├── AGENTS.md
├── EXECPLAN.md
├── Cargo.toml
├── README.md
├── specs/
│   └── grepapp_sdk_cli.md
├── tests/
│   └── unit/
│       └── test_grepapp_sdk_cli.py
└── src/
    ├── lib.rs
    ├── client.rs
    ├── error.rs
    ├── models.rs
    ├── query.rs
    ├── snippet.rs
    └── bin/
        └── gg.rs
```

Critical files and entrypoints:
- `src/lib.rs`: public SDK exports.
- `src/client.rs`: HTTP client + search orchestration.
- `src/snippet.rs`: HTML snippet parsing into line matches.
- `src/bin/gg.rs`: CLI entrypoint.
- `EXECPLAN.md`: living execution plan; must be updated during changes.

## GUIDANCE FOR HIGH-QUALITY, TESTABLE CHANGES
- Add unit tests for parsing and query building in the module where logic lives.
- Avoid network calls in tests; use fixed HTML/JSON fixtures.
- Validate changes with `cargo fmt` and `cargo test`.
- Update `EXECPLAN.md` Progress, Artifacts, and Change notes with each milestone.

## LEARNINGS + REPEATABLE ACTIONS
- The grep.app API returns hits with `repo` and `path` as strings, not nested objects.
- Each API page returns 10 hits; the service appears to cap useful paging at 100 pages.
- Snippets are HTML tables with line numbers inside `div.lineno` and matches wrapped in `<mark>`.
- Snippets include non-matching lines; parse them so CLI can provide context output.
- `total_matches` arrives as a string and may include a trailing `+` (e.g., `100+`).
- Always parse snippets into plain text + match ranges before formatting output.
- Default matching behavior should mirror ripgrep: regex by default with a single explicit literal escape hatch (`-F/--fixed-strings`).
- Avoid adding compatibility flags unless explicitly requested (require a named downstream consumer and a removal plan).
- Lock grep.app request semantics with non-network unit tests for query-param mapping (`regexp` vs `words`) when changing query flags/defaults.

## MISTAKES TO AVOID
- Don’t assume local file paths: always include repo + path in output.
- Don’t fetch unbounded pages; use `max_pages` limits.
- Don’t forget to update `EXECPLAN.md` after changes or validation runs.
- Don’t add “compatibility” flags preemptively; they expand docs/tests surface area and become long-term maintenance.
- Don’t accidentally commit agent artifacts; ensure `.codex/` and `.claude/` are gitignored.
