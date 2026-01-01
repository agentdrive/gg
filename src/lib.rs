mod client;
mod error;
mod languages;
mod models;
mod query;
mod snippet;

pub use client::GrepAppClient;
pub use error::GrepAppError;
pub use languages::{is_language_supported, languages};
pub use models::{LineMatch, SearchHit, SearchPage, SearchResult};
pub use query::{SearchOptions, SearchQuery};
