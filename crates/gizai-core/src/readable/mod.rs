//! Readable copies (GA-310): the text of a Word, Excel, PowerPoint, OpenDocument or RTF file as Markdown, for the Chief,
//! whose Read (Claude Code's) opens text, images and PDFs but not those. The copy goes beside the Chief's copy of the
//! file, `<dir>/<file id>/<name>.md`, made once and made again only when it is missing (`ensure`).
//!
//! Safety: a document's parts are read from its zip into memory by name, never unpacked to disk, and at most 200 MB of
//! any part (`Zip::part`); a spreadsheet's parts are checked against the same limit before calamine reads them, and
//! calamine reads its cells as they come, not as a grid. A readable copy is at most 20 MB (`Md`). The XML parser is
//! quick-xml, which fetches nothing: it resolves only XML's own five entities and character references (`entity`),
//! never one that a DTD defines or an external one.

mod odf;
mod rtf;
mod sheets;
mod slides;
mod word;

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use quick_xml::Reader;
use quick_xml::events::{BytesRef, BytesStart};

/// At most this much is read from any part of a document (unpacked), and from an RTF file.
pub const MAX_PART_BYTES: u64 = 200 << 20;
/// A readable copy is at most this long: a longer text is cut, with a last line that says so (`CUT_LINE`).
pub const MAX_TEXT_BYTES: usize = 20 << 20;
/// A sheet shows at most this many rows, then a line that says how many more there are.
pub const MAX_SHEET_ROWS: usize = 5_000;

/// The last line of a readable copy that was cut at `MAX_TEXT_BYTES`.
pub const CUT_LINE: &str = "(… cut here: a readable copy is at most 20 MB, so the rest is left out)";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Word,
    Sheets,
    Slides,
    OdText,
    OdSlides,
    Rtf,
}

/// The kind of a file by its extension (case ignored), and the extension in lower case.
fn kind(name: &str) -> Option<(Kind, String)> {
    let ext = name.rsplit_once('.')?.1.to_ascii_lowercase();
    let kind = match ext.as_str() {
        "docx" | "docm" | "dotx" => Kind::Word,
        "xlsx" | "xlsm" | "xlsb" | "xls" | "ods" => Kind::Sheets,
        "pptx" => Kind::Slides,
        "odt" => Kind::OdText,
        "odp" => Kind::OdSlides,
        "rtf" => Kind::Rtf,
        _ => return None,
    };
    Some((kind, ext))
}

/// What the file is, for the reasons a copy can't be made: "it can't be opened as a Word document".
fn what(kind: Kind, ext: &str) -> &'static str {
    match kind {
        Kind::Word => "a Word document",
        Kind::Sheets if ext == "ods" => "an OpenDocument spreadsheet",
        Kind::Sheets => "an Excel workbook",
        Kind::Slides => "a PowerPoint presentation",
        Kind::OdText => "an OpenDocument text",
        Kind::OdSlides => "an OpenDocument presentation",
        Kind::Rtf => "an RTF document",
    }
}

/// Whether a file called `name` gets a readable copy: a Word, Excel, PowerPoint, OpenDocument or RTF file, by its
/// extension (case ignored).
pub fn has_readable(name: &str) -> bool {
    kind(name).is_some()
}

/// Where the readable copy of the file copied at `copy` goes: beside it, with `.md` added to its name.
pub fn readable_path(copy: &Path) -> PathBuf {
    let mut p = copy.as_os_str().to_owned();
    p.push(".md");
    PathBuf::from(p)
}

/// The readable copy of the file copied at `copy` (`readable_path`), made when it isn't there yet: its path, or why it
/// couldn't be made, in plain words ("it can't be opened as a Word document (…)"). A copy that is there is kept as it
/// is. Never half a copy: it is written beside it first and then renamed.
pub fn ensure(copy: &Path) -> Result<PathBuf, String> {
    let out = readable_path(copy);
    if out.is_file() {
        return Ok(out);
    }
    let name = copy.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let text = markdown(&name, copy)?;
    let tmp = out.with_file_name(format!(".readable-{}.part", crate::ids::new_id()));
    if let Err(e) = std::fs::write(&tmp, text) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("it couldn't be written: {e}"));
    }
    match std::fs::rename(&tmp, &out) {
        Ok(()) => Ok(out),
        // made at the same time by another call (on Windows a rename doesn't replace a file that is open)
        Err(_) if out.is_file() => {
            let _ = std::fs::remove_file(&tmp);
            Ok(out)
        }
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(format!("it couldn't be written: {e}"))
        }
    }
}

/// The readable text of the file at `path`, called `name` (which says its kind): its first line says what it is, then
/// the text as Markdown, at most `MAX_TEXT_BYTES`. Err says why there is none.
pub fn markdown(name: &str, path: &Path) -> Result<String, String> {
    let Some((kind, ext)) = kind(name) else {
        return Err("Hersir makes readable copies only of Word, Excel, PowerPoint, OpenDocument and RTF files".into());
    };
    let size = std::fs::metadata(path).map_err(|e| format!("it can't be read: {e}"))?.len();
    if size == 0 {
        return Err("the file is empty".into());
    }
    let what = what(kind, &ext);
    let mut md = Md::new(name);
    // A reader that trips over a strange file must not take the rest of Hersir with it.
    let done = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match kind {
        Kind::Word => word::convert(path, what, &mut md),
        Kind::Slides => slides::convert(path, what, &mut md),
        Kind::OdText => odf::text(path, what, &mut md),
        Kind::OdSlides => odf::slides(path, what, &mut md),
        Kind::Sheets => sheets::convert(path, &ext, what, &mut md),
        Kind::Rtf => rtf::convert(path, &mut md),
    }));
    match done {
        Ok(Ok(())) => Ok(md.finish()),
        // on one line: it goes after the file's name in a chat message
        Ok(Err(why)) => Err(why.split_whitespace().collect::<Vec<_>>().join(" ")),
        Err(_) => Err(format!("it couldn't be read as {what}: Hersir's reader stopped on it")),
    }
}

/// A block of a document's text, before it is written: a paragraph or heading (Markdown already), a list item (with its
/// marker and indent) or a table (rows of cells, the first row its head).
#[derive(Debug, Clone)]
enum Block {
    Text(String),
    Item(String),
    Table(Vec<Vec<String>>),
}

/// A readable copy being written: its first line, then blocks, at most `MAX_TEXT_BYTES` in all; `finish` cuts a longer
/// one at a line and ends it with `CUT_LINE`.
struct Md {
    s: String,
    /// More text came than fits: the rest is left out, and the converters may stop.
    full: bool,
    /// The last block was a list item: the next item follows on the next line, not after a blank line.
    in_list: bool,
    /// Why the text stops before the file's end for another reason (an RTF file over 200 MB): its last line.
    stop: Option<String>,
}

impl Md {
    fn new(name: &str) -> Md {
        let first = format!("Text of {name}, made by Hersir for reading: layout and pictures are left out.");
        Md { s: first, full: false, in_list: false, stop: None }
    }

    fn full(&self) -> bool {
        self.full
    }

    /// Adds `sep` and `text`, or what fits of them (then the copy is full). One byte stays free for the last newline.
    /// Control characters other than line breaks and tabs (a NUL from a broken file) are left out.
    fn push(&mut self, sep: &str, text: &str) {
        if self.full {
            return;
        }
        let shown = |c: char| !c.is_control() || matches!(c, '\n' | '\t');
        let clean: String;
        let text = if text.chars().all(shown) {
            text
        } else {
            clean = text.chars().filter(|&c| shown(c)).collect();
            &clean
        };
        let room = (MAX_TEXT_BYTES - 1).saturating_sub(self.s.len());
        if sep.len() + text.len() <= room {
            self.s.push_str(sep);
            self.s.push_str(text);
            return;
        }
        self.full = true;
        let joined = format!("{sep}{text}");
        self.s.push_str(&joined[..floor_char(&joined, room)]);
    }

    /// A paragraph, heading or table, after a blank line. Nothing for an empty one.
    fn block(&mut self, text: &str) {
        if text.trim().is_empty() {
            return;
        }
        self.push("\n\n", text.trim_end());
        self.in_list = false;
    }

    /// A list item: on the line after the item before it, else after a blank line.
    fn item(&mut self, text: &str) {
        if text.trim().is_empty() {
            return;
        }
        let sep = if self.in_list { "\n" } else { "\n\n" };
        self.push(sep, text.trim_end());
        self.in_list = true;
    }

    /// A line right under the one before it, in the same block (a table's next row).
    fn line(&mut self, text: &str) {
        self.push("\n", text.trim_end());
        self.in_list = false;
    }

    /// A heading of the copy's own, like `## Footnotes`.
    fn heading(&mut self, text: &str) {
        self.block(&format!("## {text}"));
    }

    fn blocks(&mut self, blocks: &[Block]) {
        for b in blocks {
            if self.full {
                return;
            }
            match b {
                Block::Text(t) => self.block(t),
                Block::Item(t) => self.item(t),
                Block::Table(rows) => self.block(&table_md(rows)),
            }
        }
    }

    /// The copy's text: cut to `MAX_TEXT_BYTES` with `CUT_LINE` when it was full, or ended with the line that says why it
    /// stops early; always ending in a newline.
    fn finish(mut self) -> String {
        let last = if self.full { Some(CUT_LINE.to_string()) } else { self.stop.take() };
        if let Some(last) = last {
            let keep = MAX_TEXT_BYTES - last.len() - 3;
            if self.s.len() > keep {
                let mut at = floor_char(&self.s, keep);
                // at the end of a line, when there is one near
                if let Some(nl) = self.s[..at].rfind('\n').filter(|&nl| at - nl < 4096) {
                    at = nl;
                }
                self.s.truncate(at);
            }
            self.s.truncate(self.s.trim_end().len());
            self.s.push_str("\n\n");
            self.s.push_str(&last);
        }
        self.s.push('\n');
        self.s
    }
}

/// The largest index at most `at` that is on a character boundary of `s`.
fn floor_char(s: &str, at: usize) -> usize {
    let mut at = at.min(s.len());
    while !s.is_char_boundary(at) {
        at -= 1;
    }
    at
}

/// A table as Markdown, its first row as the head; every row as wide as the widest. Empty without rows.
fn table_md(rows: &[Vec<String>]) -> String {
    let width = rows.iter().map(Vec::len).max().unwrap_or(0);
    if width == 0 {
        return String::new();
    }
    let mut out = String::new();
    for (i, row) in rows.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&table_row(row.iter().map(String::as_str), width));
        if i == 0 {
            out.push('\n');
            out.push_str(&table_rule(width));
        }
    }
    out
}

/// One row of a Markdown table, `width` cells wide (missing cells are empty).
fn table_row<'a>(cells: impl Iterator<Item = &'a str>, width: usize) -> String {
    let mut s = String::from("|");
    let mut n = 0;
    for c in cells.take(width) {
        s.push(' ');
        s.push_str(&table_cell(c));
        s.push_str(" |");
        n += 1;
    }
    for _ in n..width {
        s.push_str("  |");
    }
    s
}

/// The line under a Markdown table's head.
fn table_rule(width: usize) -> String {
    let mut s = String::from("|");
    for _ in 0..width {
        s.push_str(" --- |");
    }
    s
}

/// A cell's text on one line: `|` escaped, line breaks as `<br>`, tabs as spaces.
fn table_cell(text: &str) -> String {
    text.trim().replace('|', "\\|").replace("\r\n", "<br>").replace(['\n', '\r'], "<br>").replace('\t', " ")
}

/// Blocks inside a table cell, as lines: list items without their indent, a table inside it as its rows (cells apart
/// by "; ").
fn inline(blocks: &[Block]) -> String {
    let mut lines: Vec<String> = Vec::new();
    for b in blocks {
        match b {
            Block::Text(t) => lines.push(t.trim().to_string()),
            Block::Item(t) => lines.push(t.trim().to_string()),
            Block::Table(rows) => lines.extend(rows.iter().map(|r| {
                r.iter().map(|c| c.trim()).filter(|c| !c.is_empty()).collect::<Vec<_>>().join("; ")
            })),
        }
    }
    lines.retain(|l| !l.is_empty());
    lines.join("\n")
}

/// A document opened as a zip: its parts are read into memory by name, never unpacked to disk.
struct Zip {
    z: zip::ZipArchive<BufReader<File>>,
}

/// The first bytes of an OLE file: an old .doc, .xls or .ppt, and also an Office file protected with a password.
const OLE_MAGIC: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

/// Whether the file at `path` starts as an OLE file does.
fn is_ole(path: &Path) -> bool {
    let mut head = [0u8; 8];
    File::open(path).and_then(|mut f| f.read_exact(&mut head)).is_ok() && head == OLE_MAGIC
}

impl Zip {
    /// Opens the file at `path`, which should be `what` (a zip). Err says why it can't be opened.
    fn open(path: &Path, what: &str) -> Result<Zip, String> {
        if is_ole(path) {
            return Err(format!("it can't be opened as {what}: it is protected with a password, or it is an older kind of file under a new name"));
        }
        let f = File::open(path).map_err(|e| format!("it can't be read: {e}"))?;
        zip::ZipArchive::new(BufReader::new(f)).map(|z| Zip { z }).map_err(|e| format!("it can't be opened as {what} ({e})"))
    }

    /// The index of the part called `name` (as in the zip, else with case ignored, as part names are in Office files).
    fn index(&self, name: &str) -> Option<usize> {
        let name = name.trim_start_matches('/');
        self.z.index_for_name(name).or_else(|| {
            (0..self.z.len()).find(|&i| self.z.name_for_index(i).and_then(|n| n.ok()).is_some_and(|n| n.eq_ignore_ascii_case(name)))
        })
    }

    /// The unpacked bytes of the part `name`, or None when there is no such part; at most `MAX_PART_BYTES`.
    fn part(&mut self, name: &str) -> Result<Option<Vec<u8>>, String> {
        let Some(i) = self.index(name) else { return Ok(None) };
        let f = self.z.by_index(i).map_err(|e| format!("its part {name} can't be read ({e})"))?;
        if f.size() > MAX_PART_BYTES {
            return Err(format!("its part {name} is larger than 200 MB unpacked"));
        }
        let mut buf = Vec::with_capacity(f.size() as usize);
        f.take(MAX_PART_BYTES + 1).read_to_end(&mut buf).map_err(|e| format!("its part {name} can't be read ({e})"))?;
        if buf.len() as u64 > MAX_PART_BYTES {
            return Err(format!("its part {name} is larger than 200 MB unpacked"));
        }
        Ok(Some(buf))
    }

    /// The part `name` as XML text (`xml_text`), or None when there is no such part.
    fn xml(&mut self, name: &str) -> Result<Option<String>, String> {
        Ok(self.part(name)?.map(xml_text))
    }

    /// Checks that no part is larger than `MAX_PART_BYTES` unpacked, before another reader (calamine) reads them: the
    /// zip reader stops a part at the size it declares, so none can give more.
    fn check_sizes(&self) -> Result<(), String> {
        for i in 0..self.z.len() {
            let e = self.z.by_index_data(i).map_err(|e| format!("its parts can't be read ({e})"))?;
            if e.size() > MAX_PART_BYTES {
                let name = e.name().map(|n| n.into_owned()).unwrap_or_default();
                return Err(format!("its part {name} is larger than 200 MB unpacked"));
            }
        }
        Ok(())
    }

    /// The relationships of the part `part` (from `<folder>/_rels/<name>.rels`), with their targets as part names.
    fn rels(&mut self, part: &str) -> Vec<Rel> {
        let (dir, file) = part.rsplit_once('/').unwrap_or(("", part));
        let rels = if dir.is_empty() { format!("_rels/{file}.rels") } else { format!("{dir}/_rels/{file}.rels") };
        let Ok(Some(xml)) = self.xml(&rels) else { return vec![] };
        let mut r = xml_reader(&xml);
        let mut out = Vec::new();
        while let Ok(ev) = r.read_event() {
            match ev {
                quick_xml::events::Event::Start(e) | quick_xml::events::Event::Empty(e) if local(&e) == "Relationship" => {
                    let (Some(id), Some(kind), Some(target)) = (attr(&e, "Id"), attr(&e, "Type"), attr(&e, "Target")) else { continue };
                    let external = attr(&e, "TargetMode").is_some_and(|m| m.eq_ignore_ascii_case("External"));
                    let kind = kind.rsplit('/').next().unwrap_or_default().to_string();
                    let target = if external { target } else { join_part(dir, &target) };
                    out.push(Rel { id, kind, target, external });
                }
                quick_xml::events::Event::Eof => break,
                _ => {}
            }
        }
        out
    }
}

/// A relationship of a part: its id (rId3), its kind (the end of its type: "header", "slide", "hyperlink"), and its
/// target: a part name, or an address when it is external.
#[derive(Debug, Clone)]
struct Rel {
    id: String,
    kind: String,
    target: String,
    external: bool,
}

/// The part name of `target`, relative to the folder `dir` (or from the zip's root when it starts with a slash).
fn join_part(dir: &str, target: &str) -> String {
    let mut parts: Vec<&str> = if target.starts_with('/') { vec![] } else { dir.split('/').filter(|p| !p.is_empty()).collect() };
    for p in target.split('/') {
        match p {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            p => parts.push(p),
        }
    }
    parts.join("/")
}

/// A part's XML as text: UTF-8 (a byte order mark dropped), or UTF-16 with its byte order mark. Bytes that aren't UTF-8
/// become U+FFFD.
fn xml_text(bytes: Vec<u8>) -> String {
    if bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) {
        let enc = if bytes[0] == 0xFF { encoding_rs::UTF_16LE } else { encoding_rs::UTF_16BE };
        return enc.decode_with_bom_removal(&bytes).0.into_owned();
    }
    let body = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes);
    match std::str::from_utf8(body) {
        Ok(s) => s.to_string(),
        Err(_) => String::from_utf8_lossy(body).into_owned(),
    }
}

/// A reader over `xml` that keeps text as it is. quick-xml fetches nothing: entities are left to `entity`.
fn xml_reader(xml: &str) -> Reader<&[u8]> {
    let mut r = Reader::from_str(xml);
    r.config_mut().trim_text(false);
    r
}

/// An element's name without its prefix: "p" for w:p.
fn local<'a>(e: &'a BytesStart) -> &'a str {
    let n = e.name().0;
    n.rsplit_once(':').map_or(n, |(_, l)| l)
}

/// An end tag's name without its prefix.
fn local_end<'a>(e: &'a quick_xml::events::BytesEnd) -> &'a str {
    let n = e.name().0;
    n.rsplit_once(':').map_or(n, |(_, l)| l)
}

/// The value of an element's attribute, by its name without its prefix (`val` for w:val).
fn attr(e: &BytesStart, name: &str) -> Option<String> {
    e.attributes().flatten().find(|a| {
        let k = a.key.0;
        k.rsplit_once(':').map_or(k, |(_, l)| l) == name
    }).map(|a| a.normalized_value(quick_xml::XmlVersion::Implicit1_0).map(|v| v.into_owned()).unwrap_or_else(|_| a.value.to_string()))
}

/// An element's relationship id: its r:id (an attribute called id with a prefix other than w:, as p:sldId also has an id
/// of its own).
fn rel_id(e: &BytesStart) -> Option<String> {
    e.attributes().flatten().find(|a| a.key.0.rsplit_once(':').is_some_and(|(p, l)| l == "id" && p != "w"))
        .map(|a| a.value.to_string())
}

/// The text of a reference in text: a character reference or one of XML's five entities. Any other is left out:
/// nothing is looked up.
fn entity(r: &BytesRef) -> Option<String> {
    if let Ok(Some(c)) = r.resolve_char_ref() {
        return Some(c.to_string());
    }
    quick_xml::escape::resolve_predefined_entity(r).map(str::to_string)
}

/// Why a part's XML can't be read.
fn xml_error(part: &str, e: impl std::fmt::Display) -> String {
    format!("its part {part} isn't valid XML ({e})")
}

/// The number `n` as a list item's marker counts: 1, 2, 3.
fn counted(start: i64, n: &mut Option<i64>) -> i64 {
    let v = n.map_or(start, |v| v + 1);
    *n = Some(v);
    v
}
