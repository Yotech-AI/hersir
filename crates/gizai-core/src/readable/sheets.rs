//! Spreadsheets (.xlsx, .xlsm, .xlsb, .xls, .ods), read by calamine: each sheet under `## <sheet name>` as a Markdown
//! table (its first row the table's head; rows and columns without a value left out), each value as calamine gives it
//! (dates as dates, not serial numbers), at most `MAX_SHEET_ROWS` rows a sheet, then `(… N more rows left out)`.
//!
//! An .xlsx or .xlsb sheet is read cell by cell (calamine's cell readers), never as a grid: a sheet with one value in A1
//! and one in XFD1048576 is two cells, not 17 billion. An .xls or .ods sheet comes as a grid (calamine reads those
//! whole when it opens them); an .ods file is counted first (`ods_cells`) and refused when its grids would be too large.
use std::path::Path;

use calamine::{Data, DataRef, ExcelDateTime, Ods, Reader, SheetType, SheetVisible, Xls, Xlsb, Xlsx, open_workbook};
use quick_xml::events::Event;

use super::{MAX_SHEET_ROWS, MAX_TEXT_BYTES, Md, Zip, attr, table_row, table_rule, xml_reader};

/// The most cells the grids of an .xls or .ods file's sheets may hold together (calamine builds them whole when it
/// opens the file: about 32 bytes a cell).
const MAX_GRID_CELLS: u64 = 10_000_000;

/// Why a file whose grids would be larger isn't read.
const TOO_LARGE: &str = "its sheets are too large for Hersir to read (more than 10 million cells)";

pub(super) fn convert(path: &Path, ext: &str, what: &str, md: &mut Md) -> Result<(), String> {
    let open = |e: &dyn std::fmt::Display| format!("it can't be opened as {what} ({e})");
    let zipped = std::fs::File::open(path).and_then(|mut f| {
        let mut head = [0u8; 4];
        std::io::Read::read_exact(&mut f, &mut head).map(|_| head == *b"PK\x03\x04")
    }).unwrap_or(false);
    match ext {
        // an .xls that is really an .xlsx (renamed) is read as one
        "xlsx" | "xlsm" | "xls" if ext != "xls" || zipped => {
            Zip::open(path, what)?.check_sizes()?;
            let mut wb: Xlsx<_> = open_workbook(path).map_err(|e| open(&e))?;
            for sheet in wb.sheets_metadata().to_vec() {
                if md.full() {
                    break;
                }
                let mut t = Table::default();
                match wb.worksheet_cells_reader(&sheet.name) {
                    Ok(mut cells) => loop {
                        match cells.next_cell() {
                            Ok(Some(c)) => t.add(c.get_position(), text_ref(c.get_value())),
                            Ok(None) => break,
                            Err(e) => {
                                t.error = Some(e.to_string());
                                break;
                            }
                        }
                    },
                    Err(e) => t.error = Some(e.to_string()),
                }
                t.write(md, &sheet);
            }
        }
        "xlsb" => {
            Zip::open(path, what)?.check_sizes()?;
            let mut wb: Xlsb<_> = open_workbook(path).map_err(|e| open(&e))?;
            for sheet in wb.sheets_metadata().to_vec() {
                if md.full() {
                    break;
                }
                let mut t = Table::default();
                match wb.worksheet_cells_reader(&sheet.name) {
                    Ok(mut cells) => loop {
                        match cells.next_cell() {
                            Ok(Some(c)) => t.add(c.get_position(), text_ref(c.get_value())),
                            Ok(None) => break,
                            Err(e) => {
                                t.error = Some(e.to_string());
                                break;
                            }
                        }
                    },
                    Err(e) => t.error = Some(e.to_string()),
                }
                t.write(md, &sheet);
            }
        }
        "xls" => {
            if xls_cells(path).map_err(|e| open(&e))? > MAX_GRID_CELLS {
                return Err(TOO_LARGE.into());
            }
            let mut wb: Xls<_> = open_workbook(path).map_err(|e| open(&e))?;
            for sheet in wb.sheets_metadata().to_vec() {
                if md.full() {
                    break;
                }
                let mut t = Table::default();
                match wb.worksheet_range(&sheet.name) {
                    Ok(range) => t.add_range(&range),
                    Err(e) => t.error = Some(e.to_string()),
                }
                t.write(md, &sheet);
            }
        }
        _ => {
            let mut zip = Zip::open(path, what)?;
            zip.check_sizes()?;
            if let Some(content) = zip.xml("content.xml")?
                && ods_cells(&content) > MAX_GRID_CELLS
            {
                return Err(TOO_LARGE.into());
            }
            let mut wb: Ods<_> = open_workbook(path).map_err(|e| open(&e))?;
            for sheet in wb.sheets_metadata().to_vec() {
                if md.full() {
                    break;
                }
                let mut t = Table::default();
                match wb.worksheet_range(&sheet.name) {
                    Ok(range) => t.add_range(&range),
                    Err(e) => t.error = Some(e.to_string()),
                }
                t.write(md, &sheet);
            }
        }
    }
    Ok(())
}

/// A sheet's rows with a value, as they come: the first `MAX_SHEET_ROWS` kept (each with its cells' columns and text),
/// the rest counted.
#[derive(Default)]
struct Table {
    rows: Vec<Vec<(u32, String)>>,
    /// Rows with a value, kept or not.
    total: usize,
    last_row: Option<u32>,
    /// The row being read is kept.
    keeping: bool,
    /// The text kept so far: past what a readable copy holds, no more is kept.
    bytes: usize,
    /// Why the sheet couldn't be read (to its end).
    error: Option<String>,
}

impl Table {
    /// A cell at (row, column) with its text; an empty one is left out.
    fn add(&mut self, (row, col): (u32, u32), text: String) {
        if text.trim().is_empty() {
            return;
        }
        if self.last_row != Some(row) {
            self.last_row = Some(row);
            self.total += 1;
            self.keeping = self.rows.len() < MAX_SHEET_ROWS;
            if self.keeping {
                self.rows.push(Vec::new());
            }
        }
        if self.keeping && self.bytes <= MAX_TEXT_BYTES {
            self.bytes += text.len() + 3;
            if let Some(cells) = self.rows.last_mut() {
                cells.push((col, text));
            }
        }
    }

    /// The cells of a sheet that calamine read as a grid.
    fn add_range(&mut self, range: &calamine::Range<Data>) {
        let (row0, col0) = range.start().unwrap_or((0, 0));
        for (r, row) in range.rows().enumerate() {
            for (c, d) in row.iter().enumerate() {
                if !matches!(d, Data::Empty) {
                    self.add((row0 + r as u32, col0 + c as u32), text(d));
                }
            }
        }
    }

    /// The sheet under `## <name>` (and "(hidden)" for a hidden one): a table of its kept rows, with only the columns
    /// that have a value, and the line on the rows left out.
    fn write(&self, md: &mut Md, sheet: &calamine::Sheet) {
        let hidden = if sheet.visible == SheetVisible::Visible { "" } else { " (hidden)" };
        md.heading(&format!("{}{hidden}", sheet.name.replace(['\r', '\n'], " ")));
        if sheet.typ == SheetType::ChartSheet {
            return md.block("(This sheet is a chart.)");
        }
        if self.rows.is_empty() {
            return md.block(&match &self.error {
                Some(e) => format!("(This sheet can't be read: {e}.)"),
                None => "(This sheet is empty.)".to_string(),
            });
        }
        let mut cols: Vec<u32> = self.rows.iter().flat_map(|r| r.iter().map(|c| c.0)).collect();
        cols.sort_unstable();
        cols.dedup();
        let width = cols.len();
        for (i, cells) in self.rows.iter().enumerate() {
            if md.full() {
                return;
            }
            let mut row = vec![""; width];
            for (col, text) in cells {
                if let Ok(at) = cols.binary_search(col) {
                    row[at] = text;
                }
            }
            let line = table_row(row.into_iter(), width);
            if i == 0 {
                md.block(&line);
                md.line(&table_rule(width));
            } else {
                md.line(&line);
            }
        }
        if let Some(e) = &self.error {
            md.block(&format!("(The rest of this sheet can't be read: {e}.)"));
        }
        let left_out = self.total.saturating_sub(self.rows.len());
        if left_out > 0 {
            md.block(&format!("(… {left_out} more rows left out)"));
        }
    }
}

/// A cell's text as calamine gives it.
fn text(d: &Data) -> String {
    match d {
        Data::Int(i) => i.to_string(),
        Data::Float(f) => number(*f),
        Data::String(s) => s.clone(),
        Data::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_string(),
        Data::DateTime(dt) => date(dt),
        Data::DateTimeIso(s) => iso_date(s),
        Data::DurationIso(s) => iso_duration(s),
        Data::Error(e) => e.to_string(),
        Data::Empty => String::new(),
    }
}

/// `text` for a cell from a cell reader.
fn text_ref(d: &DataRef) -> String {
    match d {
        DataRef::Int(i) => i.to_string(),
        DataRef::Float(f) => number(*f),
        DataRef::String(s) => s.clone(),
        DataRef::SharedString(s) => s.to_string(),
        DataRef::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_string(),
        DataRef::DateTime(dt) => date(dt),
        DataRef::DateTimeIso(s) => iso_date(s),
        DataRef::DurationIso(s) => iso_duration(s),
        DataRef::Error(e) => e.to_string(),
        DataRef::Empty => String::new(),
    }
}

/// A number as a spreadsheet shows it in its General format: whole numbers without decimals, others to 15 significant
/// digits (0.1 + 0.2 is 0.3).
fn number(v: f64) -> String {
    if !v.is_finite() {
        return v.to_string();
    }
    if v == v.trunc() && v.abs() < 1e17 {
        return format!("{}", v as i64);
    }
    if v.abs() < 1e-9 {
        return format!("{v:e}");
    }
    let digits = if v.abs() >= 1.0 { v.abs().log10().floor() as i32 + 1 } else { 1 };
    let decimals = (15 - digits).clamp(0, 15) as usize;
    let s = format!("{v:.decimals$}");
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
    if s == "-0" { "0".into() } else { s }
}

/// A date or time cell: 2026-11-18, 2026-11-18 14:30, 14:30 (a time only) or 36:00:00 (a duration).
fn date(dt: &ExcelDateTime) -> String {
    let v = dt.as_f64();
    if dt.is_duration() {
        let secs = (v * 86_400.0).round() as i64;
        let sign = if secs < 0 { "-" } else { "" };
        let secs = secs.abs();
        return format!("{sign}{}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60);
    }
    let (y, mo, d, h, mi, s, ms) = dt.to_ymd_hms_milli();
    let time = if s > 0 || ms > 0 { format!("{h:02}:{mi:02}:{s:02}") } else { format!("{h:02}:{mi:02}") };
    if (0.0..1.0).contains(&v) {
        time
    } else if (h, mi, s, ms) == (0, 0, 0, 0) {
        format!("{y:04}-{mo:02}-{d:02}")
    } else {
        format!("{y:04}-{mo:02}-{d:02} {time}")
    }
}

/// An OpenDocument date: 2026-11-18, or 2026-11-18 14:30:00 with its time.
fn iso_date(s: &str) -> String {
    let s = s.strip_suffix("T00:00:00").unwrap_or(s);
    s.replacen('T', " ", 1)
}

/// An OpenDocument time or duration (PT14H30M00S) as 14:30:00; another one as it is.
fn iso_duration(s: &str) -> String {
    let Some(rest) = s.strip_prefix("PT") else { return s.to_string() };
    let (mut h, mut m, mut sec, mut num) = (0f64, 0f64, 0f64, String::new());
    for c in rest.chars() {
        match c {
            '0'..='9' | '.' => num.push(c),
            'H' | 'M' | 'S' => {
                let Ok(n) = num.parse::<f64>() else { return s.to_string() };
                match c {
                    'H' => h = n,
                    'M' => m = n,
                    _ => sec = n,
                }
                num.clear();
            }
            _ => return s.to_string(),
        }
    }
    let total = (h * 3600.0 + m * 60.0 + sec).round() as i64;
    format!("{:02}:{:02}:{:02}", total / 3600, total / 60 % 60, total % 60)
}

/// How many cells an .xls file's sheets take as calamine reads them: per sheet, the larger of what its dimensions record
/// sets aside (worked out as calamine does, wrapping included) and its grid from the first used row and column to the
/// last. From the records of the file's Workbook stream, at most 200 MB of it.
fn xls_cells(path: &Path) -> Result<u64, String> {
    let mut comp = cfb::open(path).map_err(|e| e.to_string())?;
    let Some(name) = ["/Workbook", "/Book"].into_iter().find(|n| comp.is_stream(n)) else { return Ok(0) };
    let stream = comp.open_stream(name).map_err(|e| e.to_string())?;
    if stream.len() > super::MAX_PART_BYTES {
        return Err("its workbook is larger than 200 MB".into());
    }
    let mut buf = Vec::new();
    std::io::Read::read_to_end(&mut std::io::Read::take(stream, super::MAX_PART_BYTES), &mut buf).map_err(|e| e.to_string())?;
    let u16at = |d: &[u8], i: usize| d.get(i..i + 2).map(|b| u32::from(u16::from_le_bytes([b[0], b[1]])));
    let u32at = |d: &[u8], i: usize| d.get(i..i + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    let (mut total, mut depth, mut at) = (0u64, 0u32, 0usize);
    let (mut dim, mut used) = (0u64, None::<(u32, u32, u32, u32)>);
    let widen = |used: &mut Option<(u32, u32, u32, u32)>, r: u32, c0: u32, c1: u32| {
        let (a, b, c, d) = used.unwrap_or((r, r, c0, c1));
        *used = Some((a.min(r), b.max(r), c.min(c0), d.max(c1)));
    };
    while at + 4 <= buf.len() {
        let (typ, len) = (u16at(&buf, at).unwrap_or(0), u16at(&buf, at + 2).unwrap_or(0) as usize);
        let data = &buf[at + 4..(at + 4 + len).min(buf.len())];
        at += 4 + len;
        match typ {
            // BOF: a sheet (or the workbook's globals) starts; one inside it (a chart) calamine skips
            0x0809 => {
                depth += 1;
                if depth == 1 {
                    (dim, used) = (0, None);
                }
            }
            // EOF
            0x000A => {
                if depth == 1 {
                    let grid = used.map_or(0, |(r0, r1, c0, c1)| u64::from(r1 - r0 + 1) * u64::from(c1 - c0 + 1));
                    total = total.saturating_add(dim.max(grid));
                }
                depth = depth.saturating_sub(1);
            }
            _ if depth != 1 => {}
            // Dimensions
            0x0200 => {
                let (rf, rl, cf, cl) = match data.len() {
                    10 => (u16at(data, 0), u16at(data, 2), u16at(data, 4), u16at(data, 6)),
                    14 | 16 => (u32at(data, 0), u32at(data, 4), u16at(data, 8), u16at(data, 10)),
                    _ => continue,
                };
                let (Some(rf), Some(rl), Some(cf), Some(cl)) = (rf, rl, cf, cl) else { continue };
                let cf = if cf > 0xFF || cl < cf { 0 } else { cf };
                dim = if rl >= 1 && cl >= 1 {
                    u64::from((rl - 1).wrapping_sub(rf).wrapping_add(1)) * u64::from(cl - cf)
                } else {
                    1
                };
            }
            // a cell: Number, Label, RString, BoolErr, Rk, LabelSst, Formula
            0x0203 | 0x0204 | 0x00D6 | 0x0205 | 0x027E | 0x00FD | 0x0006 => {
                if let (Some(r), Some(c)) = (u16at(data, 0), u16at(data, 2)) {
                    widen(&mut used, r, c, c);
                }
            }
            // MulRk: a row's cells from one column to another
            0x00BD => {
                let last = data.len().checked_sub(2).and_then(|k| u16at(data, k));
                if let (Some(r), Some(c0), Some(c1)) = (u16at(data, 0), u16at(data, 2), last) {
                    widen(&mut used, r, c0.min(c1), c0.max(c1));
                }
            }
            _ => {}
        }
    }
    Ok(total)
}

/// How many cells the grids of an .ods file's sheets hold as calamine builds them: per sheet, its rows from the first
/// with a value to the last (repeats counted), times its columns from the first with a value to the last.
fn ods_cells(content: &str) -> u64 {
    const MAX_ROWS: u64 = 1_048_576;
    const MAX_COLS: u64 = 16_384;
    let mut total = 0u64;
    let mut r = xml_reader(content);
    // per sheet: the rows so far, the first row with a value, the end of the last one, and the columns with values
    let (mut rows, mut first, mut last, mut cmin, mut cmax) = (0u64, None::<u64>, 0u64, u64::MAX, 0u64);
    // per row: its repeats, the column, and its columns with values
    let (mut rep, mut col, mut rmin, mut rmax) = (1u64, 0u64, u64::MAX, None::<u64>);
    loop {
        let (e, start) = match r.read_event() {
            Ok(Event::Start(e)) => (e, true),
            Ok(Event::Empty(e)) => (e, false),
            Ok(Event::End(e)) => {
                match e.name().0 {
                    "table:table-row" => {
                        if let Some(max) = rmax {
                            first.get_or_insert(rows);
                            last = rows + rep;
                            cmin = cmin.min(rmin);
                            cmax = cmax.max(max);
                        }
                        rows = (rows + rep).min(MAX_ROWS);
                    }
                    "table:table" => {
                        if let Some(f) = first {
                            total = total.saturating_add(last.saturating_sub(f).saturating_mul(cmax + 1 - cmin.min(cmax)));
                        }
                        (rows, first, last, cmin, cmax) = (0, None, 0, u64::MAX, 0);
                    }
                    _ => {}
                }
                continue;
            }
            Ok(Event::Eof) | Err(_) => break,
            Ok(_) => continue,
        };
        let n = |a: &str| attr(&e, a).and_then(|v| v.parse::<u64>().ok()).unwrap_or(1).max(1);
        match e.name().0 {
            "table:table-row" => (rep, col, rmin, rmax) = (n("number-rows-repeated").min(MAX_ROWS), 0, u64::MAX, None),
            "table:table-cell" | "table:covered-table-cell" => {
                let k = n("number-columns-repeated").min(MAX_COLS);
                // a cell with a value, a formula or content (calamine keeps it)
                if start || attr(&e, "value-type").is_some() || attr(&e, "formula").is_some() {
                    rmin = rmin.min(col);
                    rmax = Some(rmax.unwrap_or(0).max((col + k - 1).min(MAX_COLS - 1)));
                }
                col = (col + k).min(MAX_COLS);
                if start && r.read_to_end(e.name()).is_err() {
                    break;
                }
            }
            _ => {}
        }
    }
    total
}
