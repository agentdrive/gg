use std::time::Duration;

#[derive(Debug, Clone)]
pub struct SearchQuery {
    pub pattern: String,
    pub regex: bool,
    pub whole_words: bool,
    pub case_sensitive: bool,
    pub repo_filter: Option<String>,
    pub path_filter: Option<String>,
    pub languages: Vec<String>,
}

impl SearchQuery {
    pub fn new(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            regex: false,
            whole_words: false,
            case_sensitive: false,
            repo_filter: None,
            path_filter: None,
            languages: Vec::new(),
        }
    }

    pub fn regex(mut self, regex: bool) -> Self {
        self.regex = regex;
        self
    }

    pub fn whole_words(mut self, whole_words: bool) -> Self {
        self.whole_words = whole_words;
        self
    }

    pub fn case_sensitive(mut self, case_sensitive: bool) -> Self {
        self.case_sensitive = case_sensitive;
        self
    }

    pub fn repo_filter(mut self, repo_filter: impl Into<String>) -> Self {
        self.repo_filter = Some(repo_filter.into());
        self
    }

    pub fn path_filter(mut self, path_filter: impl Into<String>) -> Self {
        self.path_filter = Some(path_filter.into());
        self
    }

    pub fn add_language(mut self, language: impl Into<String>) -> Self {
        self.languages.push(language.into());
        self
    }

    pub fn languages<I, S>(mut self, languages: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.languages.extend(languages.into_iter().map(Into::into));
        self
    }

    pub(crate) fn to_query_pairs(&self) -> Vec<(String, String)> {
        let mut pairs = Vec::new();
        pairs.push(("q".to_string(), self.pattern.clone()));
        if self.regex {
            pairs.push(("regexp".to_string(), "true".to_string()));
        } else if self.whole_words {
            pairs.push(("words".to_string(), "true".to_string()));
        }
        if self.case_sensitive {
            pairs.push(("case".to_string(), "true".to_string()));
        }
        if let Some(repo) = &self.repo_filter {
            pairs.push(("f.repo.pattern".to_string(), repo.clone()));
        }
        if let Some(path) = &self.path_filter {
            pairs.push(("f.path.pattern".to_string(), path.clone()));
        }
        for lang in &self.languages {
            pairs.push(("f.lang".to_string(), lang.clone()));
        }
        pairs
    }
}

#[derive(Debug, Clone)]
pub struct SearchOptions {
    pub max_pages: u32,
    pub concurrency: usize,
    pub timeout: Option<Duration>,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            max_pages: 10,
            concurrency: 8,
            timeout: None,
        }
    }
}

impl SearchOptions {
    pub fn max_pages(mut self, max_pages: u32) -> Self {
        self.max_pages = max_pages;
        self
    }

    pub fn concurrency(mut self, concurrency: usize) -> Self {
        self.concurrency = concurrency.max(1);
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::SearchQuery;

    #[test]
    fn builds_query_pairs_with_filters() {
        let query = SearchQuery::new("todo")
            .regex(true)
            .case_sensitive(true)
            .repo_filter("rust-lang/.*")
            .path_filter("src/.*")
            .languages(["Rust", "Go"]);

        let pairs = query.to_query_pairs();
        assert!(pairs.contains(&("q".to_string(), "todo".to_string())));
        assert!(pairs.contains(&("regexp".to_string(), "true".to_string())));
        assert!(pairs.contains(&("case".to_string(), "true".to_string())));
        assert!(pairs.contains(&("f.repo.pattern".to_string(), "rust-lang/.*".to_string())));
        assert!(pairs.contains(&("f.path.pattern".to_string(), "src/.*".to_string())));
        assert!(pairs.contains(&("f.lang".to_string(), "Rust".to_string())));
        assert!(pairs.contains(&("f.lang".to_string(), "Go".to_string())));
    }
}
