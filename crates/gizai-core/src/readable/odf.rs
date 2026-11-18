//! OpenDocument: a text (.odt) like a Word document (headings, paragraphs, lists, tables, then headers and footers,
//! footnotes, endnotes and comments; tracked deletions left out), a presentation (.odp) like a PowerPoint one (each slide
//! under `## Slide N`, title first, then `Notes:`).
use std::collections::HashMap;
use std::path::Path;

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use super::word::linked;
use super::{Block, Md, Zip, attr, counted, entity, inline, xml_error, xml_reader};

/// An OpenDocument text.
pub(super) fn text(path: &Path, what: &str, md: &mut Md) -> Result<(), String> {
    let mut zip = Zip::open(path, what)?;
    let content = zip.xml("content.xml")?.ok_or_else(|| format!("it can't be opened as {what}: it has no content.xml"))?;
    let styles_xml = zip.xml("styles.xml").ok().flatten();
    let mut styles = Styles::default();
    if let Some(s) = &styles_xml {
        styles.parse(s);
    }
    styles.parse(&content);

    let mut w = Walker::new(&styles);
    md.blocks(&w.walk(&content).map_err(|e| xml_error("content.xml", e))?);

    // headers and footers: in styles.xml's master pages; the same text once
    if let Some(s) = &styles_xml {
        let mut hw = Walker::new(&styles);
        if hw.walk(s).is_ok() {
            let mut seen: Vec<String> = Vec::new();
            for n in hw.notes.iter().filter(|n| matches!(n.kind, Kind::Header | Kind::Footer)) {
                let label = if n.kind == Kind::Header { "Header" } else { "Footer" };
                let text = inline(&n.blocks);
                if text.is_empty() || seen.contains(&format!("{label}{text}")) {
                    continue;
                }
                if seen.is_empty() {
                    md.heading("Headers and footers");
                }
                seen.push(format!("{label}{text}"));
                md.block(&format!("{label}: {text}"));
            }
        }
    }
    for (kind, title) in [(Kind::Footnote, "Footnotes"), (Kind::Endnote, "Endnotes"), (Kind::Comment, "Comments")] {
        let notes: Vec<String> = w.notes.iter().filter(|n| n.kind == kind).filter_map(|n| {
            let text = inline(&n.blocks).replace('\n', " ");
            if text.trim().is_empty() {
                return None;
            }
            Some(match kind {
                Kind::Comment => format!("{}: {text}", Some(n.label.trim()).filter(|a| !a.is_empty()).unwrap_or("Unknown")),
                _ => format!("[^{}]: {text}", n.label.trim()),
            })
        }).collect();
        if notes.is_empty() {
            continue;
        }
        md.heading(title);
        for n in &notes {
            md.block(n);
        }
    }
    Ok(())
}

/// An OpenDocument presentation.
pub(super) fn slides(path: &Path, what: &str, md: &mut Md) -> Result<(), String> {
    let mut zip = Zip::open(path, what)?;
    let content = zip.xml("content.xml")?.ok_or_else(|| format!("it can't be opened as {what}: it has no content.xml"))?;
    let mut styles = Styles::default();
    if let Ok(Some(s)) = zip.xml("styles.xml") {
        styles.parse(&s);
    }
    styles.parse(&content);
    let mut w = Walker::new(&styles);
    w.walk(&content).map_err(|e| xml_error("content.xml", e))?;
    for (i, s) in w.slides.iter().enumerate() {
        if md.full() {
            break;
        }
        md.heading(&format!("Slide {}", i + 1));
        md.blocks(&s.title);
        md.blocks(&s.body);
        let notes = inline(&s.notes);
        if !notes.trim().is_empty() {
            md.block(&format!("Notes: {notes}"));
        }
    }
    Ok(())
}

/// A list's marker at one level.
#[derive(Debug, Clone, Copy)]
enum Marker {
    Bullet,
    Number(i64),
    None,
}

/// A paragraph style: its parent and its outline level.
#[derive(Debug, Clone, Default)]
struct ParaStyle {
    parent: Option<String>,
    outline: Option<usize>,
}

/// The paragraph and list styles of styles.xml and content.xml.
#[derive(Default)]
struct Styles {
    paras: HashMap<String, ParaStyle>,
    lists: HashMap<String, [Marker; 10]>,
}

impl Styles {
    fn parse(&mut self, xml: &str) {
        let mut r = xml_reader(xml);
        let mut list: Option<(String, [Marker; 10])> = None;
        loop {
            let e = match r.read_event() {
                Ok(Event::Start(e) | Event::Empty(e)) => e,
                Ok(Event::End(e)) => {
                    if e.name().0 == "text:list-style" && let Some((name, levels)) = list.take() {
                        self.lists.insert(name, levels);
                    }
                    continue;
                }
                Ok(Event::Eof) | Err(_) => break,
                Ok(_) => continue,
            };
            match e.name().0 {
                "style:style" if attr(&e, "family").as_deref() == Some("paragraph") => {
                    let outline = attr(&e, "default-outline-level").and_then(|v| v.parse().ok());
                    self.paras.insert(attr(&e, "name").unwrap_or_default(), ParaStyle { parent: attr(&e, "parent-style-name"), outline });
                }
                "text:list-style" => list = Some((attr(&e, "name").unwrap_or_default(), [Marker::Bullet; 10])),
                n @ ("text:list-level-style-bullet" | "text:list-level-style-image" | "text:list-level-style-number") => {
                    let level: usize = attr(&e, "level").and_then(|v| v.parse().ok()).unwrap_or(1);
                    if let Some((_, levels)) = list.as_mut() && (1..=10).contains(&level) {
                        levels[level - 1] = if n != "text:list-level-style-number" {
                            Marker::Bullet
                        } else if attr(&e, "num-format").is_some_and(|f| f.is_empty()) {
                            Marker::None
                        } else {
                            Marker::Number(attr(&e, "start-value").and_then(|v| v.parse().ok()).unwrap_or(1))
                        };
                    }
                }
                _ => {}
            }
        }
    }

    /// The heading level a paragraph style gives: Title 1, Heading N N (at most 6), or its outline level; from it or its
    /// parents.
    fn heading(&self, style: &str) -> Option<usize> {
        let mut name = style.to_string();
        for _ in 0..16 {
            let shown = name.replace("_20_", " ");
            if shown.eq_ignore_ascii_case("title") {
                return Some(1);
            }
            if let Some(n) = shown.strip_prefix("Heading ").and_then(|n| n.trim().parse::<usize>().ok()) {
                return Some(n.clamp(1, 6));
            }
            let s = self.paras.get(&name)?;
            if let Some(o) = s.outline.filter(|&o| o > 0) {
                return Some(o.min(6));
            }
            name = s.parent.clone()?;
        }
        None
    }

    /// The marker of the list style `style` at `level` (1 the outermost); bullets when it isn't known.
    fn marker(&self, style: Option<&str>, level: usize) -> Marker {
        style.and_then(|s| self.lists.get(s)).map(|l| l[level.clamp(1, 10) - 1]).unwrap_or(Marker::Bullet)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Footnote,
    Endnote,
    Comment,
    Header,
    Footer,
}

/// A footnote or endnote (its citation), a comment (its author), a header or a footer, with its text.
struct Note {
    kind: Kind,
    label: String,
    blocks: Vec<Block>,
}

/// A slide: its title, its other text and its notes.
#[derive(Default)]
struct Slide {
    title: Vec<Block>,
    body: Vec<Block>,
    notes: Vec<Block>,
}

enum Holder {
    Root,
    /// A table cell, repeated this many times.
    Cell(usize),
    Note(Kind, String),
    /// A presentation's title frame.
    Title,
    /// A slide's notes.
    Notes,
}

#[derive(Default)]
struct Para {
    heading: Option<usize>,
    text: String,
    links: Vec<(usize, Option<String>)>,
    after: Vec<Block>,
}

/// A list: its style (its own or its parent's), its level (1 the outermost) and count, and whether an item is open and
/// already has its marker.
struct List {
    style: Option<String>,
    level: usize,
    count: Option<i64>,
    marked: bool,
}

enum Frame {
    Blocks(Vec<Block>, Holder),
    Para(Para),
    List(List),
    Table(Vec<Vec<String>>),
    /// A table row, repeated this many times.
    Row(Vec<String>, usize),
    /// A drawing frame: whether it shows a picture.
    Draw(bool),
    Page(Slide),
}

/// At most this many repeats of a row or cell that has text (an OpenDocument file may repeat one a million times).
const MAX_REPEAT: usize = 256;

struct Walker<'a> {
    styles: &'a Styles,
    stack: Vec<Frame>,
    notes: Vec<Note>,
    slides: Vec<Slide>,
    /// Inside a note's citation (its number) or a comment's author.
    in_label: bool,
    /// Where a list that continues the numbering of the one before it starts, by level.
    last_counts: HashMap<usize, Option<i64>>,
}

impl<'a> Walker<'a> {
    fn new(styles: &'a Styles) -> Walker<'a> {
        Walker { styles, stack: vec![], notes: vec![], slides: vec![], in_label: false, last_counts: HashMap::new() }
    }

    fn walk(&mut self, xml: &str) -> Result<Vec<Block>, quick_xml::Error> {
        self.stack = vec![Frame::Blocks(vec![], Holder::Root)];
        let mut r = xml_reader(xml);
        loop {
            match r.read_event()? {
                Event::Start(e) => self.start(&mut r, &e)?,
                Event::Empty(e) => self.empty(&e),
                Event::End(e) => self.end(e.name().0),
                Event::Text(t) => self.text(&t.xml10_content(), true),
                Event::CData(t) => self.text(&t.xml10_content(), true),
                Event::GeneralRef(g) => {
                    if let Some(s) = entity(&g) {
                        self.text(&s, false);
                    }
                }
                Event::Eof => break,
                _ => {}
            }
        }
        Ok(match self.stack.drain(..).next() {
            Some(Frame::Blocks(b, _)) => b,
            _ => vec![],
        })
    }

    fn start(&mut self, r: &mut Reader<&[u8]>, e: &BytesStart) -> Result<(), quick_xml::Error> {
        let name = e.name().0;
        match name {
            // deleted text (tracked changes), declarations, forms, styles, alt text, embedded objects and pictures' data,
            // ruby text, a comment's date, and the templates of indexes
            "text:tracked-changes" | "text:sequence-decls" | "text:variable-decls" | "text:user-field-decls"
            | "text:dde-connection-decls" | "office:forms" | "office:automatic-styles" | "office:font-face-decls"
            | "office:scripts" | "office:settings" | "office:event-listeners" | "svg:title" | "svg:desc" | "draw:object"
            | "draw:object-ole" | "office:binary-data" | "text:ruby-text" | "dc:date" | "meta:date-string"
            | "meta:creator-initials" | "draw:page-thumbnail" | "presentation:settings" | "office:styles" => {
                r.read_to_end(e.name())?;
            }
            n if n.starts_with("text:") && n.ends_with("-source") => {
                r.read_to_end(e.name())?;
            }
            "text:p" => {
                let heading = attr(e, "style-name").and_then(|s| self.styles.heading(&s));
                self.stack.push(Frame::Para(Para { heading, ..Para::default() }));
            }
            "text:h" => {
                let level = attr(e, "outline-level").and_then(|v| v.parse().ok()).unwrap_or(1usize).clamp(1, 6);
                self.stack.push(Frame::Para(Para { heading: Some(level), ..Para::default() }));
            }
            "text:list" => {
                let outer = self.stack.iter().rev().find_map(|f| match f {
                    Frame::List(l) => Some((l.style.clone(), l.level)),
                    _ => None,
                });
                let level = outer.as_ref().map_or(1, |o| o.1 + 1);
                let style = attr(e, "style-name").or_else(|| outer.and_then(|o| o.0));
                let count = if attr(e, "continue-numbering").is_some_and(|v| v == "true") {
                    self.last_counts.get(&level).copied().flatten()
                } else {
                    None
                };
                self.stack.push(Frame::List(List { style, level, count, marked: true }));
            }
            "text:list-item" | "text:list-header" => self.item(e, name == "text:list-item"),
            "table:table-header-rows" | "table:table-rows" | "table:table-row-group" | "table:table-header-columns"
            | "table:table-columns" | "table:table-column-group" => {}
            "table:table" => self.stack.push(Frame::Table(vec![])),
            "table:table-row" => self.stack.push(Frame::Row(vec![], repeat(e, "number-rows-repeated"))),
            "table:table-cell" | "table:covered-table-cell" => {
                self.stack.push(Frame::Blocks(vec![], Holder::Cell(repeat(e, "number-columns-repeated"))));
            }
            "draw:frame" => match attr(e, "class").as_deref() {
                Some("title") => self.stack.push(Frame::Blocks(vec![], Holder::Title)),
                Some("footer" | "header" | "date-time" | "page-number") => {
                    r.read_to_end(e.name())?;
                }
                _ => self.stack.push(Frame::Draw(false)),
            },
            "draw:image" => {
                self.picture_seen();
                r.read_to_end(e.name())?;
            }
            "draw:page" => self.stack.push(Frame::Page(Slide::default())),
            "presentation:notes" => self.stack.push(Frame::Blocks(vec![], Holder::Notes)),
            "text:note" => {
                let kind = if attr(e, "note-class").is_some_and(|c| c == "endnote") { Kind::Endnote } else { Kind::Footnote };
                self.stack.push(Frame::Blocks(vec![], Holder::Note(kind, String::new())));
            }
            "office:annotation" => self.stack.push(Frame::Blocks(vec![], Holder::Note(Kind::Comment, String::new()))),
            "text:note-citation" | "dc:creator" => self.in_label = true,
            "text:a" | "draw:a" => {
                let url = attr(e, "href").filter(|h| !h.starts_with('#'));
                if let Some(Frame::Para(p)) = self.stack.last_mut() {
                    p.links.push((p.text.len(), url));
                }
            }
            "style:header" | "style:header-left" | "style:header-first" | "style:footer" | "style:footer-left"
            | "style:footer-first" => {
                if attr(e, "display").is_some_and(|d| d == "false") {
                    r.read_to_end(e.name())?;
                } else {
                    let kind = if name.starts_with("style:header") { Kind::Header } else { Kind::Footer };
                    self.stack.push(Frame::Blocks(vec![], Holder::Note(kind, String::new())));
                }
            }
            _ => self.empty(e),
        }
        Ok(())
    }

    fn empty(&mut self, e: &BytesStart) {
        match e.name().0 {
            "text:s" => {
                let n = attr(e, "c").and_then(|v| v.parse().ok()).unwrap_or(1usize).clamp(1, 1000);
                self.push_text(&" ".repeat(n));
            }
            "text:tab" => self.push_text("\t"),
            "text:line-break" => self.push_text("\n"),
            "draw:image" => self.picture_seen(),
            "table:table-cell" | "table:covered-table-cell" => {
                let n = repeat(e, "number-columns-repeated").min(MAX_REPEAT);
                if let Some(Frame::Row(cells, _)) = self.stack.last_mut() {
                    cells.extend((0..n).map(|_| String::new()));
                }
            }
            _ => {}
        }
    }

    /// A list item, whose first paragraph gets the marker, or a list's header (`with_marker` false), which has none.
    fn item(&mut self, e: &BytesStart, with_marker: bool) {
        if let Some(Frame::List(l)) = self.stack.last_mut() {
            l.marked = !with_marker;
            if let Some(start) = attr(e, "start-value").and_then(|v| v.parse::<i64>().ok()) {
                l.count = Some(start - 1);
            }
        }
    }

    fn end(&mut self, name: &str) {
        match name {
            "text:p" | "text:h" => {
                if let Some(Frame::Para(p)) = self.pop(|f| matches!(f, Frame::Para(_))) {
                    self.close_para(p);
                }
            }
            "text:list" => {
                if let Some(Frame::List(l)) = self.pop(|f| matches!(f, Frame::List(_))) {
                    self.last_counts.insert(l.level, l.count);
                }
            }
            "text:list-item" | "text:list-header" => {
                if let Some(Frame::List(l)) = self.stack.last_mut() {
                    l.marked = true;
                }
            }
            "table:table-cell" | "table:covered-table-cell" => {
                if let Some(Frame::Blocks(b, Holder::Cell(n))) = self.pop(|f| matches!(f, Frame::Blocks(_, Holder::Cell(_)))) {
                    let text = inline(&b);
                    if let Some(Frame::Row(cells, _)) = self.stack.last_mut() {
                        cells.extend((0..n.min(MAX_REPEAT)).map(|_| text.clone()));
                    }
                }
            }
            "table:table-row" => {
                if let Some(Frame::Row(mut cells, n)) = self.pop(|f| matches!(f, Frame::Row(..))) {
                    while cells.last().is_some_and(|c| c.trim().is_empty()) {
                        cells.pop();
                    }
                    if let Some(Frame::Table(rows)) = self.stack.last_mut() && !cells.is_empty() {
                        for _ in 0..n.min(MAX_REPEAT) {
                            rows.push(cells.clone());
                        }
                    }
                }
            }
            "table:table" => {
                if let Some(Frame::Table(rows)) = self.pop(|f| matches!(f, Frame::Table(_))) && !rows.is_empty() {
                    self.emit(Block::Table(rows));
                }
            }
            "draw:frame" => match self.pop(|f| matches!(f, Frame::Draw(_) | Frame::Blocks(_, Holder::Title))) {
                Some(Frame::Draw(true)) => self.picture(),
                Some(Frame::Blocks(blocks, Holder::Title)) => {
                    let title = blocks.into_iter().map(|b| match b {
                        Block::Text(t) | Block::Item(t) => Block::Text(t.trim().replace('\n', " ")),
                        b => b,
                    });
                    match self.stack.iter_mut().rev().find(|f| matches!(f, Frame::Page(_))) {
                        Some(Frame::Page(s)) => s.title.extend(title),
                        _ => title.for_each(|b| self.emit(b)),
                    }
                }
                _ => {}
            },
            "draw:page" => {
                if let Some(Frame::Page(s)) = self.pop(|f| matches!(f, Frame::Page(_))) {
                    self.slides.push(s);
                }
            }
            "presentation:notes" => {
                if let Some(Frame::Blocks(blocks, Holder::Notes)) = self.pop(|f| matches!(f, Frame::Blocks(_, Holder::Notes)))
                    && let Some(Frame::Page(s)) = self.stack.iter_mut().rev().find(|f| matches!(f, Frame::Page(_)))
                {
                    s.notes.extend(blocks);
                }
            }
            "text:note-citation" | "dc:creator" => self.in_label = false,
            "text:note" | "office:annotation" | "style:header" | "style:header-left" | "style:header-first" | "style:footer"
            | "style:footer-left" | "style:footer-first" => {
                if let Some(Frame::Blocks(blocks, Holder::Note(kind, label))) = self.pop(|f| matches!(f, Frame::Blocks(_, Holder::Note(..)))) {
                    if matches!(kind, Kind::Footnote | Kind::Endnote) {
                        self.push_text(&format!("[^{}]", label.trim()));
                    }
                    self.notes.push(Note { kind, label, blocks });
                }
            }
            "text:a" | "draw:a" => {
                if let Some(Frame::Para(p)) = self.stack.last_mut()
                    && let Some((start, Some(url))) = p.links.pop()
                    && start <= p.text.len()
                    && p.text.is_char_boundary(start)
                {
                    let inner = p.text.split_off(start);
                    p.text.push_str(&linked(&inner, &url));
                }
            }
            _ => {}
        }
    }

    fn pop(&mut self, is: impl Fn(&Frame) -> bool) -> Option<Frame> {
        if self.stack.len() > 1 && self.stack.last().is_some_and(is) { self.stack.pop() } else { None }
    }

    /// Text in the document: in a paragraph, with runs of white space as one space (as OpenDocument shows them); in a
    /// note's citation or a comment's author, that label.
    fn text(&mut self, s: &str, collapse: bool) {
        if self.in_label {
            if let Some(Frame::Blocks(_, Holder::Note(_, label))) = self.stack.last_mut() {
                label.push_str(s);
            }
            return;
        }
        let Some(Frame::Para(p)) = self.stack.last_mut() else { return };
        if !collapse {
            return p.text.push_str(s);
        }
        for c in s.chars() {
            if c.is_whitespace() {
                if !p.text.is_empty() && !p.text.ends_with([' ', '\n', '\t']) {
                    p.text.push(' ');
                }
            } else {
                p.text.push(c);
            }
        }
    }

    /// Text as it is (spaces, tabs, line breaks and markers), in the paragraph being read.
    fn push_text(&mut self, s: &str) {
        if let Some(Frame::Para(p)) = self.stack.last_mut() {
            p.text.push_str(s);
        }
    }

    fn picture_seen(&mut self) {
        if let Some(Frame::Draw(pic)) = self.stack.iter_mut().rev().find(|f| matches!(f, Frame::Draw(_))) {
            *pic = true;
        }
    }

    /// `[picture]` in the paragraph it is in, or as a paragraph of its own.
    fn picture(&mut self) {
        match self.stack.last_mut() {
            Some(Frame::Para(p)) => {
                if !p.text.is_empty() && !p.text.ends_with(char::is_whitespace) {
                    p.text.push(' ');
                }
                p.text.push_str("[picture]");
            }
            _ => self.emit(Block::Text("[picture]".into())),
        }
    }

    /// Adds a block to what holds it: a cell, note, title or the document, a slide, or the paragraph a text box is in.
    fn emit(&mut self, b: Block) {
        for f in self.stack.iter_mut().rev() {
            match f {
                Frame::Blocks(blocks, _) => return blocks.push(b),
                Frame::Para(p) => return p.after.push(b),
                Frame::Page(s) => return s.body.push(b),
                _ => {}
            }
        }
    }

    fn close_para(&mut self, p: Para) {
        let text = p.text.trim().to_string();
        if !text.is_empty() {
            let styles = self.styles;
            let block = match (p.heading, self.stack.last_mut()) {
                (Some(level), _) => Block::Text(format!("{} {}", "#".repeat(level), text.replace('\n', " "))),
                (None, Some(Frame::List(l))) => {
                    let indent = "  ".repeat(l.level - 1);
                    let body = text.replace('\n', &format!("\n{indent}  "));
                    if l.marked {
                        // a paragraph after the first in an item: it goes on with the item
                        Block::Item(format!("{indent}  {body}"))
                    } else {
                        l.marked = true;
                        match styles.marker(l.style.as_deref(), l.level) {
                            Marker::Bullet => Block::Item(format!("{indent}- {body}")),
                            Marker::Number(start) => Block::Item(format!("{indent}{}. {body}", counted(start, &mut l.count))),
                            Marker::None => Block::Item(format!("{indent}{body}")),
                        }
                    }
                }
                _ => Block::Text(text),
            };
            self.emit(block);
        }
        for b in p.after {
            self.emit(b);
        }
    }
}

/// How many times a row or cell repeats (its attribute `name`), at least once.
fn repeat(e: &BytesStart, name: &str) -> usize {
    attr(e, name).and_then(|v| v.parse().ok()).unwrap_or(1usize).max(1)
}
