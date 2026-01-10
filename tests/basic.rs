use url::Url;

#[test]
fn url_pattern_subtree_detection() {
    let pat = gg::urlspec::UrlPattern::new("https://example.com/docs/**/*").unwrap();
    assert!(pat.is_subtree_pattern());
}

#[test]
fn url_pattern_matching() {
    let pat = gg::urlspec::UrlPattern::new("https://example.com/docs/*").unwrap();
    assert!(pat.matches_url_string("https://example.com/docs/a"));
    assert!(!pat.matches_url_string("https://example.com/docs/a/b"));
}

#[test]
fn cache_path_mapping() {
    let cache = gg::cache::Cache::new(Some(std::path::PathBuf::from("/tmp/gg-test"))).unwrap();
    let u = Url::parse("https://example.com/docs/getting-started").unwrap();
    let p = cache.page_path(&u).unwrap();
    assert!(p.to_string_lossy().ends_with("sites/https/example.com/docs/getting-started.md"));
}
