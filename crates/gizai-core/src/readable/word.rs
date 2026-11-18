//! Word (.docx, .docm, .dotx): the body in order (headings, paragraphs, lists, tables, line breaks and tabs; tracked
//! insertions as text, tracked deletions left out; pictures as [picture]), then, when there are any, its headers and
//! footers, footnotes, endnotes and comments.
use std::collections::HashMap;
use std::path::Path;

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use super::{Block, Md, Zip, attr, counted, entity, inline, local, local_end, rel_id, xml_error, xml_reader};

pub(super) fn convert(path: &Path, what: &str, md: &mut Md) -> Result<(), String> {
    let mut zip = Zip::open(path, what)?;
    let main = zip.rels("").into_iter().find(|r| r.kind == "officeDocument").map(|r| r.target)
        .unwrap_or_else(|| "word/document.xml".into());
    let doc = zip.xml(&main)?.ok_or_else(|| format!("it can't be opened as {what}: it has no {main}"))?;
    let rels = zip.rels(&main);
    let part = |zip: &mut Zip, kind: &str| rels.iter().find(|r| r.kind == kind && !r.external).and_then(|r| zip.xml(&r.target).ok().flatten());
    let styles = part(&mut zip, "styles").map(|x| Styles::parse(&x)).unwrap_or_default();
    let numbering = part(&mut zip, "numbering").map(|x| Numbering::parse(&x)).unwrap_or_default();

    let mut body = Walker::new(&styles, &numbering, links(&mut zip, &main));
    md.blocks(&body.walk(&doc).map_err(|e| xml_error(&main, e))?);

    // Headers and footers, headers first, each in the order of its part's name (header1, header2, header10); the same
    // text once.
    let mut hf: Vec<(&str, String)> = rels.iter().filter(|r| !r.external && (r.kind == "header" || r.kind == "footer"))
        .map(|r| (if r.kind == "header" { "Header" } else { "Footer" }, r.target.clone())).collect();
    hf.sort_by(|a, b| (a.0 != "Header", a.1.len(), &a.1).cmp(&(b.0 != "Header", b.1.len(), &b.1)));
    hf.dedup_by(|a, b| a.1 == b.1);
    let mut seen: Vec<String> = Vec::new();
    for (label, target) in hf {
        let Ok(Some(xml)) = zip.xml(&target) else { continue };
        let mut w = Walker::new(&styles, &numbering, links(&mut zip, &target));
        let Ok(blocks) = w.walk(&xml) else { continue };
        let text = inline(&blocks);
        if text.is_empty() || seen.contains(&format!("{label}{text}")) {
            continue;
        }
        if seen.is_empty() {
            md.heading("Headers and footers");
        }
        seen.push(format!("{label}{text}"));
        md.block(&format!("{label}: {text}"));
    }

    for (kind, title) in [("footnotes", "Footnotes"), ("endnotes", "Endnotes"), ("comments", "Comments")] {
        let Some(target) = rels.iter().find(|r| r.kind == kind && !r.external).map(|r| r.target.clone()) else { continue };
        let Ok(Some(xml)) = zip.xml(&target) else { continue };
        let mut w = Walker::new(&styles, &numbering, links(&mut zip, &target));
        if w.walk(&xml).is_err() {
            continue;
        }
        let notes: Vec<String> = w.notes.iter().filter_map(|n| {
            let text = inline(&n.blocks).replace('\n', " ");
            if text.trim().is_empty() {
                return None;
            }
            Some(match n.kind {
                NoteKind::Footnote => format!("[^{}]: {text}", n.id),
                NoteKind::Endnote => format!("[^e{}]: {text}", n.id),
                NoteKind::Comment => format!("{}: {text}", n.author.as_deref().filter(|a| !a.trim().is_empty()).unwrap_or("Unknown")),
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

/// The addresses of a part's external links, by relationship id.
fn links(zip: &mut Zip, part: &str) -> HashMap<String, String> {
    zip.rels(part).into_iter().filter(|r| r.external && r.kind == "hyperlink").map(|r| (r.id, r.target)).collect()
}

/// A link's address as Markdown takes it: no spaces or round brackets.
pub(super) fn link_target(url: &str) -> String {
    url.trim().replace(' ', "%20").replace('(', "%28").replace(')', "%29")
}

/// The text of a link: `[text](address)`, or the text alone when it is empty or the address isn't one.
pub(super) fn linked(text: &str, url: &str) -> String {
    let url = link_target(url);
    if text.trim().is_empty() || url.is_empty() {
        text.to_string()
    } else {
        format!("[{text}]({url})")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NoteKind {
    Footnote,
    Endnote,
    Comment,
}

/// A footnote, endnote or comment: its id, its author (a comment's) and its text.
struct Note {
    kind: NoteKind,
    id: String,
    author: Option<String>,
    blocks: Vec<Block>,
}

/// What holds blocks: the part itself, a table cell (how many grid columns it spans), or a note.
enum Holder {
    Root,
    Cell(usize),
    Note(NoteKind, String, Option<String>),
}

/// A paragraph being read.
#[derive(Default)]
struct Para {
    style: Option<String>,
    num_id: Option<String>,
    ilvl: Option<usize>,
    outline: Option<usize>,
    text: String,
    /// The links open in it: where each starts in `text`, and its address (None: a link inside the document).
    links: Vec<(usize, Option<String>)>,
    /// Blocks that come after it: the paragraphs of a text box in it.
    after: Vec<Block>,
}

enum Frame {
    Blocks(Vec<Block>, Holder),
    Para(Para),
    Table(Vec<Vec<String>>),
    Row(Vec<String>),
    /// A drawing, picture or object: whether it shows a picture.
    Drawing(bool),
}

/// Reads one part of a Word document (the body, a header or footer, the footnotes, endnotes or comments) into blocks.
struct Walker<'a> {
    styles: &'a Styles,
    numbering: &'a Numbering,
    links: HashMap<String, String>,
    /// Each list's count per level, by its numbering id.
    counters: HashMap<String, [Option<i64>; 9]>,
    stack: Vec<Frame>,
    /// Inside a text element (w:t, or m:t in an equation).
    in_text: bool,
    notes: Vec<Note>,
}

impl<'a> Walker<'a> {
    fn new(styles: &'a Styles, numbering: &'a Numbering, links: HashMap<String, String>) -> Walker<'a> {
        Walker { styles, numbering, links, counters: HashMap::new(), stack: vec![], in_text: false, notes: vec![] }
    }

    /// The blocks of the part `xml`, in order; footnotes, endnotes and comments go to `notes`.
    fn walk(&mut self, xml: &str) -> Result<Vec<Block>, quick_xml::Error> {
        self.stack = vec![Frame::Blocks(vec![], Holder::Root)];
        self.in_text = false;
        let mut r = xml_reader(xml);
        loop {
            match r.read_event()? {
                Event::Start(e) => self.start(&mut r, &e)?,
                Event::Empty(e) => self.empty(&e),
                Event::End(e) => self.end(local_end(&e)),
                Event::Text(t) if self.in_text => self.text(&t.xml10_content()),
                Event::CData(t) if self.in_text => self.text(&t.xml10_content()),
                Event::GeneralRef(g) if self.in_text => {
                    if let Some(s) = entity(&g) {
                        self.text(&s);
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
        match local(e) {
            // deleted and moved-away text, field codes, the properties a tracked change replaced, the other version of
            // a choice, tab stops, ruby text and properties that hold no text
            "del" | "moveFrom" | "delText" | "delInstrText" | "instrText" | "Fallback" | "pPrChange" | "rPrChange"
            | "tblPrChange" | "tcPrChange" | "trPrChange" | "sectPrChange" | "numberingChange" | "tabs" | "sectPr"
            | "sdtPr" | "rPr" | "fldData" | "background" | "tblPr" | "tblGrid" | "trPr" | "rt" => {
                r.read_to_end(e.name())?;
            }
            "p" => self.stack.push(Frame::Para(Para::default())),
            "tbl" => self.stack.push(Frame::Table(vec![])),
            "tr" => self.stack.push(Frame::Row(vec![])),
            "tc" => self.stack.push(Frame::Blocks(vec![], Holder::Cell(1))),
            "t" => self.in_text = true,
            "drawing" | "pict" | "object" => self.stack.push(Frame::Drawing(false)),
            "hyperlink" => {
                let url = rel_id(e).and_then(|id| self.links.get(&id).cloned());
                if let Some(p) = self.para() {
                    p.links.push((p.text.len(), url));
                }
            }
            n @ ("footnote" | "endnote" | "comment") => {
                // the lines that separate notes from the text aren't notes
                if attr(e, "type").is_some_and(|t| t != "normal") {
                    r.read_to_end(e.name())?;
                } else {
                    let kind = match n {
                        "footnote" => NoteKind::Footnote,
                        "endnote" => NoteKind::Endnote,
                        _ => NoteKind::Comment,
                    };
                    self.stack.push(Frame::Blocks(vec![], Holder::Note(kind, attr(e, "id").unwrap_or_default(), attr(e, "author"))));
                }
            }
            _ => self.prop(e),
        }
        Ok(())
    }

    fn empty(&mut self, e: &BytesStart) {
        match local(e) {
            "tab" | "ptab" => self.text("\t"),
            "br" | "cr" => self.text("\n"),
            "noBreakHyphen" => self.text("-"),
            "footnoteReference" => {
                if let Some(id) = attr(e, "id") {
                    self.text(&format!("[^{id}]"));
                }
            }
            "endnoteReference" => {
                if let Some(id) = attr(e, "id") {
                    self.text(&format!("[^e{id}]"));
                }
            }
            _ => self.prop(e),
        }
    }

    /// A property that matters for the text: a paragraph's style, list and outline level, a cell's span, a picture.
    fn prop(&mut self, e: &BytesStart) {
        let val = || attr(e, "val");
        match local(e) {
            "pStyle" => {
                if let Some(p) = self.para() {
                    p.style = val();
                }
            }
            "numId" => {
                if let Some(p) = self.para() {
                    p.num_id = val();
                }
            }
            "ilvl" => {
                if let Some(p) = self.para() {
                    p.ilvl = val().and_then(|v| v.parse().ok());
                }
            }
            "outlineLvl" => {
                if let Some(p) = self.para() {
                    p.outline = val().and_then(|v| v.parse().ok());
                }
            }
            "gridSpan" => {
                if let Some(Frame::Blocks(_, Holder::Cell(span))) = self.stack.last_mut() {
                    *span = val().and_then(|v| v.parse().ok()).unwrap_or(1).clamp(1, 64);
                }
            }
            "blip" | "imagedata" => {
                if let Some(Frame::Drawing(pic)) = self.stack.iter_mut().rev().find(|f| matches!(f, Frame::Drawing(_))) {
                    *pic = true;
                }
            }
            _ => {}
        }
    }

    fn end(&mut self, name: &str) {
        match name {
            "t" => self.in_text = false,
            "p" => {
                if let Some(Frame::Para(p)) = self.pop(|f| matches!(f, Frame::Para(_))) {
                    self.close_para(p);
                }
            }
            "tc" => {
                if let Some(Frame::Blocks(b, Holder::Cell(span))) = self.pop(|f| matches!(f, Frame::Blocks(_, Holder::Cell(_))))
                    && let Some(Frame::Row(cells)) = self.stack.last_mut()
                {
                    cells.push(inline(&b));
                    cells.extend((1..span).map(|_| String::new()));
                }
            }
            "tr" => {
                // a row with no text (a deleted one) is left out
                if let Some(Frame::Row(cells)) = self.pop(|f| matches!(f, Frame::Row(_)))
                    && let Some(Frame::Table(rows)) = self.stack.last_mut()
                    && cells.iter().any(|c| !c.trim().is_empty())
                {
                    rows.push(cells);
                }
            }
            "tbl" => {
                if let Some(Frame::Table(rows)) = self.pop(|f| matches!(f, Frame::Table(_))) && !rows.is_empty() {
                    self.emit(Block::Table(rows));
                }
            }
            "drawing" | "pict" | "object" => {
                if let Some(Frame::Drawing(true)) = self.pop(|f| matches!(f, Frame::Drawing(_))) {
                    self.picture();
                }
            }
            "hyperlink" => {
                if let Some(p) = self.para()
                    && let Some((start, Some(url))) = p.links.pop()
                    && start <= p.text.len()
                    && p.text.is_char_boundary(start)
                {
                    let inner = p.text.split_off(start);
                    p.text.push_str(&linked(&inner, &url));
                }
            }
            "footnote" | "endnote" | "comment" => {
                if let Some(Frame::Blocks(blocks, Holder::Note(kind, id, author))) = self.pop(|f| matches!(f, Frame::Blocks(_, Holder::Note(..)))) {
                    self.notes.push(Note { kind, id, author, blocks });
                }
            }
            _ => {}
        }
    }

    /// Takes the top frame off when `is` says it is the one that ends.
    fn pop(&mut self, is: impl Fn(&Frame) -> bool) -> Option<Frame> {
        if self.stack.len() > 1 && self.stack.last().is_some_and(is) { self.stack.pop() } else { None }
    }

    /// The paragraph being read, when it is the innermost thing (not inside a drawing).
    fn para(&mut self) -> Option<&mut Para> {
        match self.stack.last_mut() {
            Some(Frame::Para(p)) => Some(p),
            _ => None,
        }
    }

    fn text(&mut self, s: &str) {
        if let Some(p) = self.para() {
            p.text.push_str(s);
        }
    }

    /// `[picture]` in the paragraph it is in, or as a paragraph of its own.
    fn picture(&mut self) {
        match self.para() {
            Some(p) => {
                if !p.text.is_empty() && !p.text.ends_with(char::is_whitespace) {
                    p.text.push(' ');
                }
                p.text.push_str("[picture]");
            }
            None => self.emit(Block::Text("[picture]".into())),
        }
    }

    /// Adds a block to what holds it: a cell, a note, the part, or the paragraph a text box is in (after it).
    fn emit(&mut self, b: Block) {
        for f in self.stack.iter_mut().rev() {
            match f {
                Frame::Blocks(blocks, _) => return blocks.push(b),
                Frame::Para(p) => return p.after.push(b),
                _ => {}
            }
        }
    }

    fn close_para(&mut self, p: Para) {
        let text = p.text.trim().to_string();
        if !text.is_empty() {
            let b = self.para_block(&p, text);
            self.emit(b);
        }
        for b in p.after {
            self.emit(b);
        }
    }

    /// A paragraph as a heading (its style or outline level), a list item (its numbering) or plain text.
    fn para_block(&mut self, p: &Para, text: String) -> Block {
        let style = p.style.as_deref();
        let level = match p.outline {
            Some(o) => (o < 9).then(|| (o + 1).min(6)),
            None => style.and_then(|s| self.styles.heading(s)),
        };
        if let Some(level) = level {
            return Block::Text(format!("{} {}", "#".repeat(level), text.replace('\n', " ")));
        }
        let (style_num, style_ilvl) = style.map(|s| self.styles.list(s)).unwrap_or_default();
        let Some(num_id) = p.num_id.clone().or(style_num).filter(|n| n != "0") else { return Block::Text(text) };
        let ilvl = p.ilvl.or(style_ilvl).unwrap_or(0).min(8);
        let lvl = self.numbering.level(&num_id, ilvl);
        if lvl.none {
            return Block::Text(text);
        }
        let marker = if lvl.bullet {
            "-".to_string()
        } else {
            let c = self.counters.entry(num_id).or_insert([None; 9]);
            let n = counted(lvl.start, &mut c[ilvl]);
            c[ilvl + 1..].iter_mut().for_each(|d| *d = None);
            format!("{n}.")
        };
        let indent = "  ".repeat(ilvl);
        Block::Item(format!("{indent}{marker} {}", text.replace('\n', &format!("\n{indent}  "))))
    }
}

/// A paragraph style: its name (lower case), the style it is based on, its outline level and its list.
#[derive(Default, Debug, Clone)]
struct Style {
    name: String,
    based_on: Option<String>,
    outline: Option<usize>,
    num_id: Option<String>,
    ilvl: Option<usize>,
}

/// The paragraph styles of styles.xml, by id.
#[derive(Default)]
struct Styles {
    by_id: HashMap<String, Style>,
}

impl Styles {
    fn parse(xml: &str) -> Styles {
        let mut out = Styles::default();
        let mut cur: Option<(String, Style)> = None;
        let mut r = xml_reader(xml);
        loop {
            let (e, start) = match r.read_event() {
                Ok(Event::Start(e)) => (e, true),
                Ok(Event::Empty(e)) => (e, false),
                Ok(Event::End(e)) => {
                    if local_end(&e) == "style" && let Some((id, s)) = cur.take() {
                        out.by_id.insert(id, s);
                    }
                    continue;
                }
                Ok(Event::Eof) | Err(_) => break,
                Ok(_) => continue,
            };
            let val = || attr(&e, "val");
            match local(&e) {
                "style" => {
                    cur = None;
                    if attr(&e, "type").is_none_or(|t| t == "paragraph") {
                        cur = Some((attr(&e, "styleId").unwrap_or_default(), Style::default()));
                    } else if start && r.read_to_end(e.name()).is_err() {
                        break;
                    }
                }
                "rPr" | "pPrChange" | "rPrChange" | "tblPr" | "tcPr" | "trPr" | "tblStylePr" | "latentStyles" | "docDefaults" if start => {
                    if r.read_to_end(e.name()).is_err() {
                        break;
                    }
                }
                n => {
                    if let Some((_, s)) = cur.as_mut() {
                        match n {
                            "name" => s.name = val().unwrap_or_default().to_lowercase(),
                            "basedOn" => s.based_on = val(),
                            "outlineLvl" => s.outline = val().and_then(|v| v.parse().ok()),
                            "numId" => s.num_id = val(),
                            "ilvl" => s.ilvl = val().and_then(|v| v.parse().ok()),
                            _ => {}
                        }
                    }
                }
            }
        }
        out
    }

    /// The heading level of the style `id`: Title 1, Heading N N (at most 6), else its outline level, else the level of
    /// the style it is based on. None for a style that isn't a heading.
    fn heading(&self, id: &str) -> Option<usize> {
        let mut id = id.to_string();
        for _ in 0..16 {
            let Some(s) = self.by_id.get(&id) else { return name_level(&id) };
            if let Some(l) = name_level(&s.name) {
                return Some(l);
            }
            if let Some(o) = s.outline {
                return (o < 9).then(|| (o + 1).min(6));
            }
            id = s.based_on.clone()?;
        }
        None
    }

    /// The list of the style `id` (its numbering id and level), from it or a style it is based on.
    fn list(&self, id: &str) -> (Option<String>, Option<usize>) {
        let (mut num, mut ilvl) = (None, None);
        let mut id = id.to_string();
        for _ in 0..16 {
            let Some(s) = self.by_id.get(&id) else { break };
            num = num.or_else(|| s.num_id.clone());
            ilvl = ilvl.or(s.ilvl);
            match &s.based_on {
                Some(b) => id = b.clone(),
                None => break,
            }
        }
        (num, ilvl)
    }
}

/// The heading level a style's name (or id) gives: Title 1, "Heading N" or "Heading N" N (at most 6).
fn name_level(name: &str) -> Option<usize> {
    let n = name.trim().to_lowercase();
    if n == "title" {
        return Some(1);
    }
    let level: usize = n.strip_prefix("heading")?.trim().parse().ok()?;
    (1..=9).contains(&level).then(|| level.min(6))
}

/// One level of a list: bullets, no marker at all, or numbers from `start`.
#[derive(Debug, Clone, Copy)]
struct Level {
    bullet: bool,
    none: bool,
    start: i64,
}

impl Default for Level {
    fn default() -> Level {
        Level { bullet: false, none: false, start: 1 }
    }
}

/// The lists of numbering.xml: each numbering id's levels.
#[derive(Default)]
struct Numbering {
    abstracts: HashMap<String, [Level; 9]>,
    /// A numbering id's abstract list and the start values it overrides, by level.
    nums: HashMap<String, (String, HashMap<usize, i64>)>,
}

impl Numbering {
    fn parse(xml: &str) -> Numbering {
        let mut out = Numbering::default();
        let mut abs: Option<(String, [Level; 9])> = None;
        let mut lvl: Option<usize> = None;
        let mut num: Option<(String, String, HashMap<usize, i64>)> = None;
        let mut over: Option<usize> = None;
        let mut r = xml_reader(xml);
        loop {
            let e = match r.read_event() {
                Ok(Event::Start(e) | Event::Empty(e)) => e,
                Ok(Event::End(e)) => {
                    match local_end(&e) {
                        "abstractNum" => {
                            if let Some((id, levels)) = abs.take() {
                                out.abstracts.insert(id, levels);
                            }
                        }
                        "num" => {
                            if let Some((id, a, o)) = num.take() {
                                out.nums.insert(id, (a, o));
                            }
                        }
                        "lvl" => lvl = None,
                        "lvlOverride" => over = None,
                        _ => {}
                    }
                    continue;
                }
                Ok(Event::Eof) | Err(_) => break,
                Ok(_) => continue,
            };
            let val = || attr(&e, "val");
            match local(&e) {
                "abstractNum" => abs = Some((attr(&e, "abstractNumId").unwrap_or_default(), [Level::default(); 9])),
                "lvl" if num.is_none() => lvl = attr(&e, "ilvl").and_then(|v| v.parse().ok()).filter(|&l: &usize| l < 9),
                "start" => {
                    if let (Some((_, levels)), Some(l)) = (abs.as_mut(), lvl) {
                        levels[l].start = val().and_then(|v| v.parse().ok()).unwrap_or(1);
                    }
                }
                "numFmt" => {
                    if let (Some((_, levels)), Some(l)) = (abs.as_mut(), lvl) {
                        let f = val().unwrap_or_default();
                        levels[l].bullet = f == "bullet";
                        levels[l].none = f == "none";
                    }
                }
                "num" => num = Some((attr(&e, "numId").unwrap_or_default(), String::new(), HashMap::new())),
                "abstractNumId" => {
                    if let Some((_, a, _)) = num.as_mut() {
                        *a = val().unwrap_or_default();
                    }
                }
                "lvlOverride" => over = attr(&e, "ilvl").and_then(|v| v.parse().ok()),
                "startOverride" => {
                    if let (Some((_, _, o)), Some(l)) = (num.as_mut(), over) {
                        o.insert(l, val().and_then(|v| v.parse().ok()).unwrap_or(1));
                    }
                }
                _ => {}
            }
        }
        out
    }

    /// The level `ilvl` of the list `num_id`; bullets for a list that isn't defined.
    fn level(&self, num_id: &str, ilvl: usize) -> Level {
        let Some((abs, over)) = self.nums.get(num_id) else { return Level { bullet: true, ..Level::default() } };
        let mut l = self.abstracts.get(abs).map(|a| a[ilvl.min(8)]).unwrap_or(Level { bullet: true, ..Level::default() });
        if let Some(s) = over.get(&ilvl) {
            l.start = *s;
        }
        l
    }
}
