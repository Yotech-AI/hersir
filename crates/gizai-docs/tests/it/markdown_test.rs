//! Markdown for the docs page (guides, decisions, changelog), as yodocs rendered it: an id on each heading from its
//! text, collected for "On this page"; raw HTML shown as text; a link that would run a script or open data or a file
//! going nowhere.
use gizai_docs::markdown::{escape, render, slugify, Heading};

#[test]
fn each_heading_gets_an_id_from_its_text_and_is_collected() {
    let r = render("# Build and look at it\n\n## 1. Install\n\nText.\n\n## 1. Install\n\n### `npx yodocs` check\n\n## !!!\n");
    let h = |level, text: &str, slug: &str| Heading { level, text: text.into(), slug: slug.into() };
    assert_eq!(r.headings, vec![h(1, "Build and look at it", "build-and-look-at-it"), h(2, "1. Install", "1-install"),
        h(2, "1. Install", "1-install-2"), h(3, "npx yodocs check", "npx-yodocs-check"), h(2, "!!!", "section")]);
    assert!(r.html.contains(r#"<h2 id="1-install-2">1. Install</h2>"#), "{}", r.html);
    assert!(r.html.contains(r#"<h3 id="npx-yodocs-check"><code>npx yodocs</code> check</h3>"#), "{}", r.html);
}

#[test]
fn slugs_keep_ascii_letters_digits_and_dashes() {
    assert_eq!(slugify("Opening the files directly"), "opening-the-files-directly");
    assert_eq!(slugify("  Café & co -- Ünïcode_1 "), "caf-co-ncode_1");
}

#[test]
fn html_in_the_markdown_is_shown_not_run() {
    let r = render("Before.\n\n<script>alert(1)</script>\n\nAn <img src=x onerror=alert(1)> inline.\n");
    assert!(!r.html.contains("<script>") && !r.html.contains("<img"), "{}", r.html);
    assert!(r.html.contains("<p>&lt;script&gt;alert(1)&lt;/script&gt;"), "{}", r.html);
    assert!(r.html.contains("An &lt;img src=x onerror=alert(1)&gt; inline."), "{}", r.html);
}

#[test]
fn a_link_that_would_run_a_script_or_open_data_or_a_file_goes_nowhere() {
    let r = render("[a](javascript:alert(1)) [b](<JavaScript :alert(1)>) [c](data:text/html,x) [d](file:///etc/passwd) \
        [e](https://example.com/x) [f](#install) ![g](data:image/png;base64,AA)");
    for bad in ["javascript", "JavaScript", "data:", "file:"] {
        assert!(!r.html.contains(&format!("href=\"{bad}")) && !r.html.contains(&format!("src=\"{bad}")), "{bad}: {}", r.html);
    }
    assert!(r.html.contains(r#"<a href="https://example.com/x">e</a>"#), "{}", r.html);
    assert!(r.html.contains(r##"<a href="#install">f</a>"##), "{}", r.html);
    assert_eq!(r.html.matches(r##"="#""##).count(), 5, "{}", r.html);
}

#[test]
fn tables_and_code_blocks_come_out_as_the_page_styles_them() {
    let r = render("| a | b |\n|---|---|\n| 1 | 2 |\n\n```php\necho 1;\n```\n");
    assert!(r.html.contains("<table>") && r.html.contains("<td>2</td>"), "{}", r.html);
    assert!(r.html.contains(r#"<pre><code class="language-php">echo 1;"#), "{}", r.html);
}

#[test]
fn text_escapes_as_html() {
    assert_eq!(escape(r#"<b class="x">Tom & Jerry's</b>"#), "&lt;b class=&quot;x&quot;&gt;Tom &amp; Jerry&#39;s&lt;/b&gt;");
}
