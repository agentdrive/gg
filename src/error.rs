use thiserror::Error;

#[derive(Debug, Error)]
pub enum GrepAppError {
    #[error("http request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("unexpected status {status} from {url}: {body}")]
    HttpStatus {
        status: reqwest::StatusCode,
        url: String,
        body: String,
    },
    #[error("failed to parse JSON response: {0}")]
    Json(#[from] serde_json::Error),
    #[error("snippet parse error: {0}")]
    Snippet(String),
}
