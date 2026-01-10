use criterion::{black_box, criterion_group, criterion_main, Criterion};
use url::Url;

fn bench_url_pattern(c: &mut Criterion) {
    let pat = gg::urlspec::UrlPattern::new("https://example.com/docs/**/*").unwrap();
    c.bench_function("url_pattern_matches", |b| {
        b.iter(|| {
            let u = "https://example.com/docs/getting-started";
            black_box(pat.matches_url_string(black_box(u)))
        })
    });
}

fn bench_cache_path(c: &mut Criterion) {
    let cache = gg::cache::Cache::new(Some(std::path::PathBuf::from("/tmp/gg-bench"))).unwrap();
    let u = Url::parse("https://example.com/docs/getting-started").unwrap();
    c.bench_function("cache_page_path", |b| {
        b.iter(|| {
            black_box(cache.page_path(black_box(&u)).unwrap())
        })
    });
}

fn bench_markdown_conversion(c: &mut Criterion) {
    // A representative HTML snippet with headings, lists, code, and a table.
    let html = r#"<!doctype html>
<html>
  <body>
    <h1>Title</h1>
    <p>Hello <strong>world</strong>. <a href=\"/docs/intro\">Intro</a></p>
    <ul><li>One</li><li>Two</li></ul>
    <pre><code class=\"language-rust\">fn main() { println!(\"hi\"); }</code></pre>
    <table>
      <tr><th>Col A</th><th>Col B</th></tr>
      <tr><td>A1</td><td>B1</td></tr>
    </table>
  </body>
</html>"#;

    c.bench_function("html_to_markdown", |b| {
        b.iter(|| {
            let md = html_to_markdown_rs::convert(black_box(html), None).unwrap();
            black_box(md)
        })
    });
}

criterion_group!(benches, bench_url_pattern, bench_cache_path, bench_markdown_conversion);
criterion_main!(benches);
