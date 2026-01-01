import pytest


@pytest.mark.skip(reason="Not implemented")
def test_query_param_building():
    """SDK builds query params for pattern, regex, words, case, repo/path/lang filters."""
    pass


@pytest.mark.skip(reason="Not implemented")
def test_parse_snippet_to_line_matches():
    """SDK parses HTML snippets into line numbers and match ranges."""
    pass


@pytest.mark.skip(reason="Not implemented")
def test_concurrent_page_fetching_respects_limits():
    """SDK respects max_pages/concurrency when fetching results."""
    pass


@pytest.mark.skip(reason="Not implemented")
def test_cli_default_output_format():
    """CLI outputs repo/path:line:content by default."""
    pass


@pytest.mark.skip(reason="Not implemented")
def test_cli_json_output():
    """CLI outputs JSON objects per matched line with --json."""
    pass


@pytest.mark.skip(reason="Not implemented")
def test_cli_no_color():
    """CLI disables ANSI colors with --no-color."""
    pass


@pytest.mark.skip(reason="Not implemented")
def test_cli_max_pages():
    """CLI stops fetching after max_pages boundary."""
    pass


@pytest.mark.skip(reason="Not implemented")
def test_cli_http_error_handling():
    """CLI returns non-zero exit on HTTP errors with a clear message."""
    pass


@pytest.mark.skip(reason="Not implemented")
def test_cli_filters_pass_through():
    """CLI passes repo/path/lang filters to the API."""
    pass


@pytest.mark.skip(reason="Not implemented")
def test_cli_zero_results():
    """CLI exits successfully with no output when there are zero results."""
    pass


@pytest.mark.skip(reason="Not implemented")
def test_cli_limit():
    """CLI limits the number of output lines with --limit."""
    pass


@pytest.mark.skip(reason="Not implemented")
def test_cli_context_lines():
    """CLI includes context lines around matches with -C/--context."""
    pass
