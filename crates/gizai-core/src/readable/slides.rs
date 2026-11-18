//! PowerPoint (.pptx): each slide in slide order under `## Slide N`: its title first, then its other text (lists,
//! tables, [picture] for a picture), then `Notes:` and its speaker notes.
use std::path::Path;

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use super::{Block, Md, Zip, attr, counted, entity, inline, local, local_end, rel_id, xml_error, xml_reader};

pub(super) fn convert(path: &Path, what: &str, md: &mut Md) -> Result<(), String> {
    let mut zip = Zip::open(path, what)?;
    let main = zip.rels("").into_iter().find(|r| r.kind == "officeDocument").map(|r| r.target)
        .unwrap_or_else(|| "ppt/presentation.xml".into());
    let pres = zip.xml(&main)?.ok_or_else(|| format!("it can't be opened as {what}: it has no {main}"))?;
    let rels = zip.rels(&main);
    let slides: Vec<String> = slide_ids(&pres).map_err(|e| xml_error(&main, e))?.into_iter()
        .filter_map(|id| rels.iter().find(|r| r.id == id && r.kind == "slide").map(|r| r.target.clone()))
        .collect();
    for (i, part) in slides.iter().enumerate() {
        if md.full() {
            break;
        }
        let slide = match zip.xml(part) {
            Ok(Some(xml)) => read_slide(&xml, false).map_err(|e| xml_error(part, e)),
            Ok(None) => Err(format!("its part {part} is missing")),
            Err(e) => Err(e),
        };
        let slide = match slide {
            Ok(s) => s,
            Err(why) => {
                md.heading(&format!("Slide {}", i + 1));
                md.block(&format!("(This slide can't be read: {why}.)"));
                continue;
            }
        };
        md.heading(&format!("Slide {}{}", i + 1, if slide.hidden { " (hidden)" } else { "" }));
        md.blocks(&slide.title);
        md.blocks(&slide.body);
        let notes = zip.rels(part).into_iter().find(|r| r.kind == "notesSlide" && !r.external)
            .and_then(|r| zip.xml(&r.target).ok().flatten())
            .and_then(|xml| read_slide(&xml, true).ok())
            .map(|n| inline(&n.body))
            .unwrap_or_default();
        if !notes.trim().is_empty() {
            md.block(&format!("Notes: {notes}"));
        }
    }
    Ok(())
}

/// The relationship ids of the slides, in slide order (presentation.xml's p:sldIdLst).
fn slide_ids(xml: &str) -> Result<Vec<String>, quick_xml::Error> {
    let mut r = xml_reader(xml);
    let mut ids = Vec::new();
    loop {
        match r.read_event()? {
            Event::Start(e) | Event::Empty(e) if local(&e) == "sldId" => ids.extend(rel_id(&e)),
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(ids)
}

/// A slide's text: its title's blocks, the others, and whether it is hidden. For a notes page, `body` is its notes.
#[derive(Default)]
struct Slide {
    hidden: bool,
    title: Vec<Block>,
    body: Vec<Block>,
}

/// What a shape is, from its placeholder type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Role {
    /// A title: it comes first.
    Title,
    /// A placeholder for the slide's points: its paragraphs are bullets unless they say otherwise.
    Points,
    /// Any other text: plain unless a paragraph has a bullet or a number of its own.
    Text,
    /// A slide number, date, footer or header, or the slide's image on a notes page: left out.
    Skip,
}

/// A paragraph being read: its level, its bullet (None: as the shape has it) and its text.
#[derive(Default)]
struct Para {
    lvl: usize,
    bullet: Option<Bullet>,
    text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Bullet {
    None,
    Char,
    Number(i64),
}

enum Frame {
    Shape { role: Role, blocks: Vec<Block>, counts: [Option<i64>; 9] },
    Para(Para),
    Table(Vec<Vec<String>>),
    Row(Vec<String>),
    Cell { blocks: Vec<Block>, span: usize },
}

/// Reads a slide (or, `notes`, a notes page: then only its notes placeholder counts, as `body`).
fn read_slide(xml: &str, notes: bool) -> Result<Slide, quick_xml::Error> {
    let mut w = Walker { notes, slide: Slide::default(), stack: vec![], in_text: false };
    let mut r = xml_reader(xml);
    loop {
        match r.read_event()? {
            Event::Start(e) => w.start(&mut r, &e)?,
            Event::Empty(e) => w.empty(&e),
            Event::End(e) => w.end(local_end(&e)),
            Event::Text(t) if w.in_text => w.text(&t.xml10_content()),
            Event::CData(t) if w.in_text => w.text(&t.xml10_content()),
            Event::GeneralRef(g) if w.in_text => {
                if let Some(s) = entity(&g) {
                    w.text(&s);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(w.slide)
}

struct Walker {
    notes: bool,
    slide: Slide,
    stack: Vec<Frame>,
    in_text: bool,
}

impl Walker {
    fn start(&mut self, r: &mut Reader<&[u8]>, e: &BytesStart) -> Result<(), quick_xml::Error> {
        match local(e) {
            // the other version of a choice, and run properties (no text)
            "Fallback" | "rPr" | "endParaRPr" | "extLst" => {
                r.read_to_end(e.name())?;
            }
            "sld" => self.slide.hidden = attr(e, "show").is_some_and(|s| s == "0" || s == "false"),
            "sp" => self.stack.push(Frame::Shape { role: if self.notes { Role::Skip } else { Role::Text }, blocks: vec![], counts: [None; 9] }),
            "p" => self.stack.push(Frame::Para(Para::default())),
            "tbl" => self.stack.push(Frame::Table(vec![])),
            "tr" => self.stack.push(Frame::Row(vec![])),
            "tc" => self.cell(e),
            "t" => self.in_text = true,
            "pic" => {
                self.emit(Block::Text("[picture]".into()));
                r.read_to_end(e.name())?;
            }
            _ => self.prop(e),
        }
        Ok(())
    }

    fn empty(&mut self, e: &BytesStart) {
        match local(e) {
            "br" => self.text("\n"),
            "tc" => {
                self.cell(e);
                self.end("tc");
            }
            _ => self.prop(e),
        }
    }

    /// A table cell: a merged one's continuation is an empty cell.
    fn cell(&mut self, e: &BytesStart) {
        let merged = ["hMerge", "vMerge"].iter().any(|a| attr(e, a).is_some_and(|v| v == "1" || v == "true"));
        let span = attr(e, "gridSpan").and_then(|v| v.parse().ok()).unwrap_or(1usize).clamp(1, 64);
        self.stack.push(Frame::Cell { blocks: vec![], span: if merged { 0 } else { span } });
    }

    fn prop(&mut self, e: &BytesStart) {
        match local(e) {
            "ph" => {
                let typ = attr(e, "type").unwrap_or_else(|| "obj".into());
                let role = match (self.notes, typ.as_str()) {
                    (true, "body") => Role::Text,
                    (true, _) => Role::Skip,
                    (false, "title" | "ctrTitle") => Role::Title,
                    (false, "body" | "obj") => Role::Points,
                    (false, "sldNum" | "dt" | "ftr" | "hdr" | "sldImg") => Role::Skip,
                    _ => Role::Text,
                };
                if let Some(Frame::Shape { role: r, .. }) = self.stack.last_mut() {
                    *r = role;
                }
            }
            "pPr" => {
                if let Some(Frame::Para(p)) = self.stack.last_mut() {
                    p.lvl = attr(e, "lvl").and_then(|v| v.parse().ok()).unwrap_or(0usize).min(8);
                }
            }
            "buNone" => self.bullet(Bullet::None),
            "buChar" | "buBlip" => self.bullet(Bullet::Char),
            "buAutoNum" => self.bullet(Bullet::Number(attr(e, "startAt").and_then(|v| v.parse().ok()).unwrap_or(1))),
            _ => {}
        }
    }

    fn bullet(&mut self, b: Bullet) {
        if let Some(Frame::Para(p)) = self.stack.last_mut() {
            p.bullet = Some(b);
        }
    }

    fn text(&mut self, s: &str) {
        if let Some(Frame::Para(p)) = self.stack.last_mut() {
            p.text.push_str(s);
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
            "sp" => {
                if let Some(Frame::Shape { role, blocks, .. }) = self.pop(|f| matches!(f, Frame::Shape { .. })) {
                    match role {
                        Role::Title => self.slide.title.extend(blocks.into_iter().map(|b| match b {
                            Block::Text(t) | Block::Item(t) => Block::Text(t.trim().replace('\n', " ")),
                            b => b,
                        })),
                        Role::Skip => {}
                        _ => self.slide.body.extend(blocks),
                    }
                }
            }
            "tc" => {
                if let Some(Frame::Cell { blocks, span }) = self.pop(|f| matches!(f, Frame::Cell { .. }))
                    && let Some(Frame::Row(cells)) = self.stack.last_mut()
                {
                    cells.push(if span == 0 { String::new() } else { inline(&blocks) });
                    cells.extend((1..span).map(|_| String::new()));
                }
            }
            "tr" => {
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
            _ => {}
        }
    }

    fn pop(&mut self, is: impl Fn(&Frame) -> bool) -> Option<Frame> {
        if self.stack.last().is_some_and(is) { self.stack.pop() } else { None }
    }

    /// Adds a block to the cell or shape it is in, else to the slide (a table or picture outside a shape).
    fn emit(&mut self, b: Block) {
        for f in self.stack.iter_mut().rev() {
            match f {
                Frame::Shape { blocks, .. } | Frame::Cell { blocks, .. } => return blocks.push(b),
                _ => {}
            }
        }
        if !self.notes {
            self.slide.body.push(b);
        }
    }

    /// A paragraph as a bullet, a numbered item or plain text, as its shape and its own bullet say.
    fn close_para(&mut self, p: Para) {
        let text = p.text.trim().to_string();
        if text.is_empty() {
            return;
        }
        // in a table cell, a paragraph is a line of the cell
        if matches!(self.stack.last(), Some(Frame::Cell { .. })) {
            return self.emit(Block::Text(text));
        }
        let indent = "  ".repeat(p.lvl);
        let block = match self.stack.iter_mut().rev().find(|f| matches!(f, Frame::Shape { .. })) {
            Some(Frame::Shape { role, counts, .. }) => {
                let bullet = match (p.bullet, *role) {
                    (_, Role::Title) => Bullet::None,
                    (Some(b), _) => b,
                    (None, Role::Points) => Bullet::Char,
                    (None, _) => Bullet::None,
                };
                match bullet {
                    Bullet::None => Block::Text(text),
                    Bullet::Char => Block::Item(format!("{indent}- {}", text.replace('\n', &format!("\n{indent}  ")))),
                    Bullet::Number(start) => {
                        let n = counted(start, &mut counts[p.lvl]);
                        counts[p.lvl + 1..].iter_mut().for_each(|c| *c = None);
                        Block::Item(format!("{indent}{n}. {}", text.replace('\n', &format!("\n{indent}  "))))
                    }
                }
            }
            _ => Block::Text(text),
        };
        self.emit(block);
    }
}
