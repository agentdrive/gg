use crate::error::GrepAppError;
use crate::models::LineMatch;
use html_escape::decode_html_entities;
use once_cell::sync::Lazy;
use regex::Regex;
use std::ops::Range;

const MARK_START: &str = "__GG_MARK_START__";
const MARK_END: &str = "__GG_MARK_END__";

static TR_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?s)<tr[^>]*>.*?</tr>").expect("tr regex"));
static LINENO_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"<div[^>]*class=\"lineno\"[^>]*>(\d+)</div>"#).expect("lineno regex")
});
static PRE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?s)<pre>(.*?)</pre>").expect("pre regex"));
static MARK_START_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"<mark[^>]*>").expect("mark start regex"));
static MARK_END_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"</mark>").expect("mark end regex"));
static TAG_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"<[^>]+>").expect("tag regex"));

pub(crate) fn parse_snippet(snippet: &str) -> Result<Vec<LineMatch>, GrepAppError> {
    let mut lines = Vec::new();

    for tr_match in TR_RE.find_iter(snippet) {
        let tr_html = tr_match.as_str();
        let line_num = LINENO_RE
            .captures(tr_html)
            .and_then(|caps| caps.get(1))
            .and_then(|m| m.as_str().parse::<usize>().ok());
        let line_num = match line_num {
            Some(num) => num,
            None => continue,
        };
        let mut html = match PRE_RE.captures(tr_html).and_then(|caps| caps.get(1)) {
            Some(m) => m.as_str().to_string(),
            None => continue,
        };
        html = MARK_START_RE.replace_all(&html, MARK_START).to_string();
        html = MARK_END_RE.replace_all(&html, MARK_END).to_string();
        html = TAG_RE.replace_all(&html, "").to_string();
        let decoded = decode_html_entities(&html).to_string();
        let (line, match_ranges) = extract_matches(&decoded);
        lines.push(LineMatch {
            line_number: line_num,
            line,
            match_ranges,
        });
    }

    Ok(lines)
}

fn extract_matches(input: &str) -> (String, Vec<Range<usize>>) {
    let mut line = String::new();
    let mut ranges = Vec::new();
    let mut idx = 0;
    let mut pos = 0;
    let mut in_mark = false;
    let mut range_start = 0;

    while pos < input.len() {
        if input[pos..].starts_with(MARK_START) {
            in_mark = true;
            range_start = idx;
            pos += MARK_START.len();
            continue;
        }
        if input[pos..].starts_with(MARK_END) {
            if in_mark {
                ranges.push(range_start..idx);
            }
            in_mark = false;
            pos += MARK_END.len();
            continue;
        }
        let ch = input[pos..].chars().next().unwrap();
        line.push(ch);
        pos += ch.len_utf8();
        idx = line.len();
    }
    if in_mark {
        ranges.push(range_start..idx);
    }
    (line, ranges)
}

#[cfg(test)]
mod tests {
    use super::parse_snippet;

    #[test]
    fn parses_snippet_with_marks() {
        let snippet = r#"<table class="highlight-table">
<tr data-line="1"><td><div class="lineno">1</div></td><td><div class="highlight"><pre>let <mark>foo</mark> = 1;</pre></div></td></tr>
<tr data-line="2"><td><div class="lineno">2</div></td><td><div class="highlight"><pre>no match</pre></div></td></tr>
</table>"#;

        let lines = parse_snippet(snippet).expect("snippet parsed");
        assert_eq!(lines.len(), 2);
        let first = &lines[0];
        assert_eq!(first.line_number, 1);
        assert_eq!(first.line, "let foo = 1;");
        assert_eq!(first.match_ranges.len(), 1);
        assert_eq!(first.match_ranges[0].start, 4);
        assert_eq!(first.match_ranges[0].end, 7);
        let second = &lines[1];
        assert_eq!(second.line_number, 2);
        assert_eq!(second.line, "no match");
        assert!(second.match_ranges.is_empty());
    }
}
