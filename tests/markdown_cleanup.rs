use gg::crawl;

#[test]
fn removes_frontmatter_copy_and_images() {
    let input = r#"---
title: Example
meta-foo: bar
---

Copy page

![Alt](data:image/svg+xml;base64,AAAA)

<img src="https://example.com/x.png">

Hello world

Copy
"#;

    let out = crawl::sanitize_markdown_for_test(input);
    assert!(!out.contains("meta-foo"));
    assert!(!out.contains("Copy page"));
    assert!(!out.contains("Copy\n"));
    assert!(!out.contains("data:image"));
    assert!(!out.contains("<img"));
    assert!(out.contains("Hello world"));
}

#[test]
fn preserves_fenced_code_blocks_and_blank_lines() {
    let input = r#"---
title: Example
---

```python
print("hi")
```

Copy

Paragraph


Next paragraph
"#;

    let out = crawl::sanitize_markdown_for_test(input);
    assert!(out.contains("```python"));
    assert!(out.contains("print(\"hi\")"));
    assert!(out.contains("```\n"));
    assert!(!out.contains("Copy\n"));
    assert!(out.contains("Paragraph"));
    assert!(out.contains("Next paragraph"));
}

#[test]
fn drops_footer_section_links() {
    let input = r#"## Footer
[Docs](https://example.com/docs)
[API](https://example.com/api)

## Next
Keep this
"#;

    let out = crawl::sanitize_markdown_for_test(input);
    assert!(!out.contains("example.com/docs"));
    assert!(!out.contains("example.com/api"));
    assert!(out.contains("## Next"));
    assert!(out.contains("Keep this"));
}

#[test]
fn removes_svg_image_marker() {
    let input = "## hydrate [SVG Image](#hydrate)\n\nText\n";
    let out = crawl::sanitize_markdown_for_test(input);
    assert!(!out.contains("[SVG Image]"));
    assert!(out.contains("## hydrate"));
}

#[test]
fn drops_leading_nav_link_list() {
    let input = r#"[Home](/)
[Guide](/docs/guide)
[API](/docs/api)

# Title
"#;

    let out = crawl::sanitize_markdown_for_test(input);
    assert!(!out.contains("[Home](/)"));
    assert!(!out.contains("[Guide](/docs/guide)"));
    assert!(!out.contains("[API](/docs/api)"));
    assert!(out.contains("# Title"));
}

#[test]
fn drops_external_link_only_lines() {
    let input = r#"Text

[Docs](https://example.com/docs) [API](https://example.com/api)

More text
"#;

    let out = crawl::sanitize_markdown_for_test(input);
    assert!(!out.contains("example.com/docs"));
    assert!(!out.contains("example.com/api"));
    assert!(out.contains("More text"));
}

#[test]
fn drops_trailing_external_link_block() {
    let input = r#"Section text

[Docs](https://example.com/docs) [API](https://example.com/api)
[Blog](https://example.com/blog)

"#;

    let out = crawl::sanitize_markdown_for_test(input);
    assert!(!out.contains("example.com/docs"));
    assert!(!out.contains("example.com/api"));
    assert!(!out.contains("example.com/blog"));
    assert!(out.contains("Section text"));
}

#[test]
fn drops_copyright_and_junk_lines() {
    let input = " © Modal 2026\n\n[ [ [\n\n# Title\n";
    let out = crawl::sanitize_markdown_for_test(input);
    assert!(!out.contains("Modal 2026"));
    assert!(!out.contains("[ [ ["));
    assert!(out.contains("# Title"));
}

#[test]
fn preserves_language_from_code_class() {
    let input = r#"<pre><code class="language-python">print("hi")</code></pre>"#;
    let out = crawl::convert_with_code_visitor_for_test(
        input,
        Some(html_to_markdown_rs::options::ConversionOptions {
            code_block_style: html_to_markdown_rs::options::CodeBlockStyle::Backticks,
            ..Default::default()
        }),
    )
    .unwrap();
    assert!(out.contains("```python"));
}

#[test]
fn uses_fenced_code_blocks_even_if_indented_style() {
    let input = r#"<pre><code class="language-python">print("hi")</code></pre>"#;
    let out = crawl::convert_with_code_visitor_for_test(
        input,
        Some(html_to_markdown_rs::options::ConversionOptions {
            code_block_style: html_to_markdown_rs::options::CodeBlockStyle::Indented,
            ..Default::default()
        }),
    )
    .unwrap();
    assert!(out.contains("```python"));
}

#[test]
fn preserves_language_from_pre_class() {
    let input = r#"<pre class="language-typescript"><code>const x = 1;</code></pre>"#;
    let out = crawl::convert_with_code_visitor_for_test(
        input,
        Some(html_to_markdown_rs::options::ConversionOptions {
            code_block_style: html_to_markdown_rs::options::CodeBlockStyle::Backticks,
            ..Default::default()
        }),
    )
    .unwrap();
    assert!(out.contains("```typescript"));
}

#[test]
fn preserves_language_from_lang_class() {
    let input = r#"<pre><code class="lang-python">print("hi")</code></pre>"#;
    let out = crawl::convert_with_code_visitor_for_test(
        input,
        Some(html_to_markdown_rs::options::ConversionOptions {
            code_block_style: html_to_markdown_rs::options::CodeBlockStyle::Backticks,
            ..Default::default()
        }),
    )
    .unwrap();
    assert!(out.contains("```python"));
}
