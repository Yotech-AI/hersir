//! RTF: its plain text, paragraphs kept (a table's cells apart by tabs, each row a paragraph). Left out: fonts, colours,
//! styles, document information, pictures and objects, headers and footers, field codes, hidden text and tracked
//! deletions. Read as it comes, at most `MAX_PART_BYTES` of the file.
use std::fs::File;
use std::io::Read;
use std::path::Path;

use encoding_rs::Encoding;

use super::{MAX_PART_BYTES, Md};

/// The parts of an RTF file that aren't its text, skipped with everything in them.
const SKIPPED: &[&str] = &[
    "fonttbl", "colortbl", "stylesheet", "info", "pict", "objdata", "objclass", "themedata", "colorschememapping",
    "datastore", "latentstyles", "listtable", "listoverridetable", "rsidtbl", "generator", "xmlnstbl", "mmathPr",
    "pgdsctbl", "fldinst", "header", "headerl", "headerr", "headerf", "footer", "footerl", "footerr", "footerf",
    "private", "revtbl", "filetbl", "bkmkstart", "bkmkend", "template", "nonshppict", "shp", "shpinst", "sp", "sn", "sv",
    "comment", "userprops", "docvar", "wgrffmtfilter", "atnid", "atnauthor", "atndate", "annotation", "xe", "tc", "tcn",
    "falt", "panose", "fname", "passwordhash", "listpicture", "blipuid", "operator", "author", "title", "subject",
    "keywords", "doccomm", "company", "category", "manager", "hlinkbase",
];

#[derive(Debug, Clone, Copy)]
struct State {
    /// This group isn't text (a font table, a picture, a destination marked \*).
    skip: bool,
    /// How many fallback characters follow a \u character.
    uc: usize,
    /// Tracked deletion or hidden text: left out.
    gone: bool,
}

impl Default for State {
    fn default() -> State {
        State { skip: false, uc: 1, gone: false }
    }
}

/// The file's bytes, read in chunks, with a byte of look-ahead.
struct Input<R: Read> {
    r: R,
    buf: Vec<u8>,
    pos: usize,
    len: usize,
}

impl<R: Read> Input<R> {
    fn next(&mut self) -> std::io::Result<Option<u8>> {
        let b = self.peek()?;
        if b.is_some() {
            self.pos += 1;
        }
        Ok(b)
    }

    fn peek(&mut self) -> std::io::Result<Option<u8>> {
        if self.pos == self.len {
            self.len = self.r.read(&mut self.buf)?;
            self.pos = 0;
            if self.len == 0 {
                return Ok(None);
            }
        }
        Ok(Some(self.buf[self.pos]))
    }
}

pub(super) fn convert(path: &Path, md: &mut Md) -> Result<(), String> {
    let f = File::open(path).map_err(|e| format!("it can't be read: {e}"))?;
    let size = f.metadata().map(|m| m.len()).unwrap_or(0);
    let mut input = Input { r: f.take(MAX_PART_BYTES), buf: vec![0; 1 << 16], pos: 0, len: 0 };
    let io = |e: std::io::Error| format!("it can't be read: {e}");
    // a byte order mark or white space may come first
    while let Some(b) = input.peek().map_err(io)? {
        if b == b'{' {
            break;
        }
        if !(b.is_ascii_whitespace() || [0xEF, 0xBB, 0xBF].contains(&b)) {
            break;
        }
        input.next().map_err(io)?;
    }
    let mut head = [0u8; 5];
    for h in head.iter_mut() {
        *h = input.next().map_err(io)?.unwrap_or(0);
    }
    if &head != b"{\\rtf" {
        return Err("it can't be opened as an RTF document: it doesn't start as one".into());
    }
    let mut p = Rtf { md, stack: vec![State::default()], cur: State::default(), para: String::new(), pending: Vec::new(),
                      enc: encoding_rs::WINDOWS_1252, fallback: 0, high: None, first_in_group: false };
    // the \rtf1 of the start
    p.control_word(&mut input).map_err(io)?;
    let ended = p.run(&mut input).map_err(io)?;
    p.end_para();
    if !ended && size > MAX_PART_BYTES {
        p.md.stop = Some("(… cut here: Hersir reads at most 200 MB of a file, so the rest is left out)".into());
    }
    Ok(())
}

struct Rtf<'m> {
    md: &'m mut Md,
    stack: Vec<State>,
    cur: State,
    para: String,
    /// Text bytes in the document's code page, not decoded yet.
    pending: Vec<u8>,
    enc: &'static Encoding,
    /// Fallback characters of a \u character still to skip.
    fallback: usize,
    /// The first half of a character that \u gives in two (a surrogate pair).
    high: Option<u16>,
    /// The next control word is the first thing in its group (a destination when it names one).
    first_in_group: bool,
}

impl Rtf<'_> {
    /// Reads to the document's last brace (true) or to the end of what is read (false).
    fn run<R: Read>(&mut self, input: &mut Input<R>) -> std::io::Result<bool> {
        while let Some(b) = input.next()? {
            if self.md.full() {
                return Ok(true);
            }
            match b {
                b'{' => {
                    self.flush();
                    self.stack.push(self.cur);
                    self.first_in_group = true;
                    continue;
                }
                b'}' => {
                    self.flush();
                    self.fallback = 0;
                    match self.stack.pop() {
                        Some(s) if !self.stack.is_empty() => self.cur = s,
                        _ => return Ok(true),
                    }
                }
                b'\\' => {
                    let first = self.first_in_group;
                    self.first_in_group = false;
                    self.control(input, first)?;
                    continue;
                }
                b'\r' | b'\n' => {}
                b => self.byte(b),
            }
            self.first_in_group = false;
        }
        Ok(false)
    }

    /// What follows a backslash: a control word, a character in hex, an escaped character or a control symbol.
    fn control<R: Read>(&mut self, input: &mut Input<R>, first: bool) -> std::io::Result<()> {
        let Some(c) = input.peek()? else { return Ok(()) };
        if c.is_ascii_alphabetic() {
            let (word, param) = self.read_word(input)?;
            return self.word(input, &word, param, first);
        }
        input.next()?;
        match c {
            b'\'' => {
                let hex = [input.next()?.unwrap_or(b'0'), input.next()?.unwrap_or(b'0')];
                let v = std::str::from_utf8(&hex).ok().and_then(|h| u8::from_str_radix(h, 16).ok()).unwrap_or(b'?');
                self.byte(v);
            }
            b'\\' | b'{' | b'}' => self.byte(c),
            b'~' => self.char_text("\u{a0}"),
            b'_' => self.char_text("-"),
            b'*' => self.cur.skip = true,
            b'\r' | b'\n' => self.end_para(),
            _ => {}
        }
        Ok(())
    }

    /// The control word's letters and its number, after the backslash; the space after it is part of it.
    fn read_word<R: Read>(&mut self, input: &mut Input<R>) -> std::io::Result<(String, Option<i32>)> {
        let mut word = String::new();
        while let Some(c) = input.peek()? {
            if !c.is_ascii_alphabetic() || word.len() >= 32 {
                break;
            }
            word.push(c as char);
            input.next()?;
        }
        let mut num = String::new();
        if input.peek()? == Some(b'-') {
            num.push('-');
            input.next()?;
        }
        while let Some(c) = input.peek()? {
            if !c.is_ascii_digit() || num.len() >= 11 {
                break;
            }
            num.push(c as char);
            input.next()?;
        }
        if input.peek()? == Some(b' ') {
            input.next()?;
        }
        Ok((word, num.parse().ok()))
    }

    /// The \rtf at the start (read as a word).
    fn control_word<R: Read>(&mut self, input: &mut Input<R>) -> std::io::Result<()> {
        self.read_word(input).map(|_| ())
    }

    fn word<R: Read>(&mut self, input: &mut Input<R>, word: &str, param: Option<i32>, first: bool) -> std::io::Result<()> {
        if self.fallback > 0 && word != "u" {
            self.fallback -= 1;
            return Ok(());
        }
        if first && SKIPPED.contains(&word) {
            self.cur.skip = true;
            return Ok(());
        }
        match word {
            "par" | "sect" | "page" | "row" => self.end_para(),
            "line" => self.char_text("\n"),
            "tab" | "cell" | "nestcell" => self.char_text("\t"),
            "emdash" => self.char_text("—"),
            "endash" => self.char_text("–"),
            "bullet" => self.char_text("•"),
            "lquote" => self.char_text("‘"),
            "rquote" => self.char_text("’"),
            "ldblquote" => self.char_text("“"),
            "rdblquote" => self.char_text("”"),
            "emspace" | "enspace" | "qmspace" => self.char_text(" "),
            "u" => {
                if let Some(n) = param {
                    self.unicode(n);
                    self.fallback = self.cur.uc;
                }
            }
            "uc" => self.cur.uc = param.unwrap_or(1).clamp(0, 16) as usize,
            "ansicpg" => {
                if let Some(enc) = param.and_then(|n| u16::try_from(n).ok()).and_then(codepage::to_encoding) {
                    self.flush();
                    self.enc = enc;
                }
            }
            "mac" => self.enc = encoding_rs::MACINTOSH,
            "pc" | "pca" => self.enc = codepage::to_encoding(if word == "pc" { 437 } else { 850 }).unwrap_or(encoding_rs::WINDOWS_1252),
            "deleted" | "v" => {
                self.flush();
                self.cur.gone = param != Some(0);
            }
            "plain" => {
                self.flush();
                self.cur.gone = false;
            }
            "bin" => {
                // binary data: its bytes are skipped
                for _ in 0..param.unwrap_or(0).max(0) {
                    if input.next()?.is_none() {
                        break;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// A character from \u: a signed 16-bit number, two of them for a character outside the first plane.
    fn unicode(&mut self, n: i32) {
        let u = if n < 0 { (n + 65_536) as u16 } else { n as u16 };
        let c = match (self.high.take(), u) {
            (_, 0xD800..=0xDBFF) => {
                self.high = Some(u);
                return;
            }
            (Some(h), 0xDC00..=0xDFFF) => char::from_u32(0x10000 + ((u32::from(h) - 0xD800) << 10) + (u32::from(u) - 0xDC00)),
            (_, u) => char::from_u32(u32::from(u)),
        };
        self.char_text(&c.unwrap_or('\u{fffd}').to_string());
    }

    /// A byte of text in the document's code page (a fallback character after \u is skipped).
    fn byte(&mut self, b: u8) {
        if self.fallback > 0 {
            self.fallback -= 1;
            return;
        }
        if !self.cur.skip && !self.cur.gone {
            self.pending.push(b);
        }
    }

    /// Text that is a character already.
    fn char_text(&mut self, s: &str) {
        if self.cur.skip || self.cur.gone {
            return;
        }
        self.flush();
        self.para.push_str(s);
    }

    /// Decodes the bytes waiting into the paragraph.
    fn flush(&mut self) {
        if !self.pending.is_empty() {
            let (text, _) = self.enc.decode_without_bom_handling(&self.pending);
            self.para.push_str(&text);
            self.pending.clear();
        }
    }

    fn end_para(&mut self) {
        self.flush();
        if self.cur.skip {
            return;
        }
        let text = self.para.trim_end().trim_start_matches(['\n', '\r']).to_string();
        if !text.trim().is_empty() {
            self.md.block(&text);
        }
        self.para.clear();
    }
}
