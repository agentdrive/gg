use serde::Deserialize;
use serde::de;
use std::ops::Range;

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum StringOrU64 {
    String(String),
    Number(u64),
}

fn de_u64_from_str<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: de::Deserializer<'de>,
{
    match StringOrU64::deserialize(deserializer)? {
        StringOrU64::String(value) => {
            let trimmed = value.trim().trim_end_matches('+').replace(',', "");
            trimmed.parse().map_err(de::Error::custom)
        }
        StringOrU64::Number(value) => Ok(value),
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct ApiResponse {
    pub hits: ApiHits,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ApiHits {
    pub total: u64,
    pub hits: Vec<ApiHit>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ApiHit {
    pub repo: String,
    pub branch: String,
    pub path: String,
    #[serde(deserialize_with = "de_u64_from_str")]
    pub total_matches: u64,
    pub content: ApiContent,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ApiContent {
    pub snippet: String,
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub total: u64,
    pub hits: Vec<SearchHit>,
}

#[derive(Debug, Clone)]
pub struct SearchPage {
    pub page: u32,
    pub total: u64,
    pub hits: Vec<SearchHit>,
}

#[derive(Debug, Clone)]
pub struct SearchHit {
    pub repo: String,
    pub path: String,
    pub branch: String,
    pub total_matches: u64,
    pub lines: Vec<LineMatch>,
}

#[derive(Debug, Clone)]
pub struct LineMatch {
    pub line_number: usize,
    pub line: String,
    pub match_ranges: Vec<Range<usize>>,
}

impl LineMatch {
    pub fn highlight(&self, start: &str, end: &str) -> String {
        if self.match_ranges.is_empty() {
            return self.line.clone();
        }
        let mut out = String::new();
        let mut cursor = 0;
        for range in &self.match_ranges {
            if range.start > cursor {
                out.push_str(&self.line[cursor..range.start]);
            }
            out.push_str(start);
            out.push_str(&self.line[range.start..range.end]);
            out.push_str(end);
            cursor = range.end;
        }
        if cursor < self.line.len() {
            out.push_str(&self.line[cursor..]);
        }
        out
    }
}
