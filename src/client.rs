use crate::error::GrepAppError;
use crate::models::{ApiResponse, SearchHit, SearchPage, SearchResult};
use crate::query::{SearchOptions, SearchQuery};
use crate::snippet::parse_snippet;
use futures::{StreamExt, stream};
use reqwest::Url;
use std::time::Duration;

const DEFAULT_TIMEOUT_SECS: u64 = 20;
const PAGE_SIZE: u64 = 10;
const MAX_API_PAGES: u32 = 100;

#[derive(Clone)]
pub struct GrepAppClient {
    http: reqwest::Client,
    base_url: Url,
    timeout: Duration,
}

impl GrepAppClient {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .user_agent(format!(
                "gg/{} (+https://grep.app)",
                env!("CARGO_PKG_VERSION")
            ))
            .build()
            .expect("reqwest client");
        let base_url = Url::parse("https://grep.app").expect("valid base url");
        Self {
            http,
            base_url,
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
        }
    }

    pub fn with_base_url(base_url: Url) -> Self {
        let mut client = Self::new();
        client.base_url = base_url;
        client
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub async fn search(
        &self,
        query: &SearchQuery,
        options: &SearchOptions,
    ) -> Result<SearchResult, GrepAppError> {
        let timeout = options.timeout.unwrap_or(self.timeout);
        let first_page = self.search_page_with_timeout(query, 1, timeout).await?;
        let total = first_page.total;
        let mut hits = first_page.hits;

        let total_pages = if total == 0 {
            0
        } else {
            total.div_ceil(PAGE_SIZE) as u32
        };
        let mut max_pages = options.max_pages.clamp(1, MAX_API_PAGES);
        if total_pages > 0 {
            max_pages = max_pages.min(total_pages);
        }

        if max_pages > 1 {
            let concurrency = options.concurrency.max(1);
            let pages = 2..=max_pages;
            let mut stream = stream::iter(pages)
                .map(|page| {
                    let client = self.clone();
                    let query = query.clone();
                    async move { client.search_page_with_timeout(&query, page, timeout).await }
                })
                .buffer_unordered(concurrency);

            while let Some(page_result) = stream.next().await {
                let page = page_result?;
                hits.extend(page.hits);
            }
        }

        Ok(SearchResult { total, hits })
    }

    pub async fn search_page(
        &self,
        query: &SearchQuery,
        page: u32,
    ) -> Result<SearchPage, GrepAppError> {
        self.search_page_with_timeout(query, page, self.timeout)
            .await
    }

    async fn search_page_with_timeout(
        &self,
        query: &SearchQuery,
        page: u32,
        timeout: Duration,
    ) -> Result<SearchPage, GrepAppError> {
        let mut url = self.base_url.clone();
        url.set_path("/api/search");
        {
            let mut pairs = url.query_pairs_mut();
            for (k, v) in query.to_query_pairs() {
                pairs.append_pair(&k, &v);
            }
            pairs.append_pair("page", &page.to_string());
        }

        let response = self.http.get(url.clone()).timeout(timeout).send().await?;
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            return Err(GrepAppError::HttpStatus {
                status,
                url: url.to_string(),
                body,
            });
        }

        let api: ApiResponse = serde_json::from_str(&body)?;
        let mut hits = Vec::with_capacity(api.hits.hits.len());
        for hit in api.hits.hits {
            let lines = parse_snippet(&hit.content.snippet)?;
            hits.push(SearchHit {
                repo: hit.repo,
                path: hit.path,
                branch: hit.branch,
                total_matches: hit.total_matches,
                lines,
            });
        }

        Ok(SearchPage {
            page,
            total: api.hits.total,
            hits,
        })
    }
}

impl Default for GrepAppClient {
    fn default() -> Self {
        Self::new()
    }
}
