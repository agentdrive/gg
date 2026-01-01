mod client;
mod error;
mod models;
mod query;
mod snippet;

pub use client::GrepAppClient;
pub use error::GrepAppError;
pub use models::{LineMatch, SearchHit, SearchPage, SearchResult};
pub use query::{SearchOptions, SearchQuery};
