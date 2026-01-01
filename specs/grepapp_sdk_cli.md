# grep.app SDK + gg CLI Specification

## Summary

Provide a Rust SDK and a `gg` CLI that use the public `https://grep.app/api/search` endpoint to search GitHub repositories by pattern. The SDK must expose a typed, async API for building queries, fetching pages concurrently, and parsing snippets into line matches. The CLI must feel familiar to ripgrep users, with fast output, clear errors, and optional JSON output.

## Assumptions

- The API endpoint is `https://grep.app/api/search` and accepts query parameters such as `q`, `page`, `regexp`, `words`, `case`, `f.repo.pattern`, `f.path.pattern`, and `f.lang`.
- A single page returns up to 10 hits.
- The API returns HTML snippets with `<mark>` tags around matches and line numbers inside `div.lineno`.

## Acceptance Criteria

1. The SDK exposes a `GrepAppClient` and `SearchQuery` that can build query parameters for pattern, regex, whole-words, case sensitivity, repo/path filters, and language filters.
2. The SDK parses API JSON into structured results with `repo`, `path`, `branch`, and `LineMatch` entries, including line numbers and match ranges.
3. The SDK supports concurrent fetching across multiple pages with a configurable `max_pages` and `concurrency` option.
4. The CLI accepts a positional pattern and prints matches grouped by repo/file by default (use `--flat` for legacy `repo/path:line:content` format).
5. The CLI supports `--json` output with one JSON object per matched line.
6. The CLI respects `--no-color` by avoiding ANSI highlighting, and defaults to color when stdout is a TTY.
7. The CLI respects `--max-pages` and stops at that boundary even if more results are available.
8. The CLI handles HTTP errors or invalid responses with a non-zero exit code and a clear error message.
9. The CLI supports repo/path/language filters and passes them through to the API.
10. The CLI handles zero results by printing nothing and exiting successfully.
11. The CLI supports `--limit` to cap the number of output lines (matches + context).
12. The CLI supports `-C/--context` to include N lines of context around matches (limited to snippet lines).
13. The CLI supports `gg langs` to list available language filter values.
14. The CLI supports `--matched-repos` to return deduplicated repositories containing matches.
15. The SDK exposes the language list for programmatic use.

## Error Handling

- Network errors return a typed SDK error and a non-zero CLI exit code.
- HTTP non-200 responses return a typed SDK error containing status code and URL.
- Invalid JSON or malformed snippets surface as parse errors.

## Performance Expectations

- The CLI uses concurrent page fetching to reduce latency.
- The SDK avoids unnecessary allocations when parsing snippet HTML.

## Test Mapping

1. `test_query_param_building`
2. `test_parse_snippet_to_line_matches`
3. `test_concurrent_page_fetching_respects_limits`
4. `test_cli_default_output_format`
5. `test_cli_json_output`
6. `test_cli_no_color`
7. `test_cli_max_pages`
8. `test_cli_http_error_handling`
9. `test_cli_filters_pass_through`
10. `test_cli_zero_results`
11. `test_cli_limit`
12. `test_cli_context_lines`
13. `test_cli_langs_output`
14. `test_cli_matched_repos`
