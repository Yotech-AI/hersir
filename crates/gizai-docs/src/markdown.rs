//! Markdown to HTML for the docs page's guides, decisions and changelog, as yodocs rendered it (its
//! src/content/markdown.ts: markdown-it without HTML). Raw HTML shows as text, a link to a script, data or file address
//! goes nowhere, and each heading gets an id from its text, for the page's "On this page".
use std::collections::HashMap;

use pulldown_cmark::{CowStr, Event, Options, Parser, Tag, TagEnd};
use serde::Serialize;

/// A heading of a rendered page. `slug` is the id on it, so "On this page" can link to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Heading {
    pub level: u8,
    pub text: String,
    pub slug: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    pub html: String,
    pub headings: Vec<Heading>,
}

/// yodocs' slug: lower case, without anything but ASCII letters, digits, `_`, `-` and spaces, spaces as one `-`.
pub fn slugify(text: &str) -> String {
    let kept: String = text.to_lowercase().chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-' || c.is_whitespace()).collect();
    let mut slug = String::new();
    for c in kept.split_whitespace().collect::<Vec<_>>().join("-").chars() {
        if !(c == '-' && slug.ends_with('-')) {
            slug.push(c);
        }
    }
    slug
}

// shortcut: a bare URL stays text (markdown-it linked it); add a linkify pass if guides turn out to need it.
pub fn render(source: &str) -> Rendered {
    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let events: Vec<Event> = Parser::new_ext(source, options).collect();

    // Each heading's text first, so its id is known when its start is written.
    let mut headings = vec![];
    let mut used: HashMap<String, usize> = HashMap::new();
    let mut open: Option<(u8, String)> = None;
    for event in &events {
        match event {
            Event::Start(Tag::Heading { level, .. }) => open = Some((*level as u8, String::new())),
            Event::Text(t) | Event::Code(t) => {
                if let Some((_, text)) = open.as_mut() {
                    text.push_str(t);
                }
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some((level, text)) = open.take() {
                    let base = Some(slugify(&text)).filter(|s| !s.is_empty()).unwrap_or_else(|| "section".into());
                    let seen = used.entry(base.clone()).or_insert(0);
                    *seen += 1;
                    let slug = if *seen == 1 { base } else { format!("{base}-{seen}") };
                    headings.push(Heading { level, text, slug });
                }
            }
            _ => {}
        }
    }

    let mut ids = headings.iter().map(|h| CowStr::from(h.slug.clone()));
    let events = events.into_iter().map(|event| match event {
        Event::Start(Tag::Heading { level, classes, attrs, .. }) => Event::Start(Tag::Heading { level, id: ids.next(), classes, attrs }),
        // HTML in the Markdown is shown, never run: a block of it becomes a paragraph of text.
        Event::Html(t) | Event::InlineHtml(t) => Event::Text(t),
        Event::Start(Tag::HtmlBlock) => Event::Start(Tag::Paragraph),
        Event::End(TagEnd::HtmlBlock) => Event::End(TagEnd::Paragraph),
        Event::Start(Tag::Link { link_type, dest_url, title, id }) => Event::Start(Tag::Link { link_type, dest_url: safe(dest_url), title, id }),
        Event::Start(Tag::Image { link_type, dest_url, title, id }) => Event::Start(Tag::Image { link_type, dest_url: safe(dest_url), title, id }),
        other => other,
    });
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, events);
    Rendered { html, headings }
}

/// A link's address, or `#` for one that would run a script or open data or a file. Spaces and control characters
/// are dropped before the check, as a browser drops them from an address.
fn safe(url: CowStr<'_>) -> CowStr<'_> {
    let plain: String = url.chars().filter(|c| !c.is_whitespace() && !c.is_control()).collect::<String>().to_ascii_lowercase();
    if ["javascript:", "vbscript:", "data:", "file:"].iter().any(|s| plain.starts_with(s)) { CowStr::Borrowed("#") } else { url }
}

/// Text as HTML: `&`, `<`, `>`, `"` and `'` escaped.
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}
