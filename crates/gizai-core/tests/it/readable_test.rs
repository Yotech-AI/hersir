// GA-310 QA: readable copies (gizai_core::readable) of Word, Excel, PowerPoint, OpenDocument and RTF files: the text of
// each as Markdown beside the Chief's copy, `<dir>/<file id>/<name>.md`. The documents are built here: a zip of their XML
// parts, and an .xls (a compound file) and an .xlsb built record by record.
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};

use gizai_core::readable;

const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const W: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture""#;
const SS: &str = r#"xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships""#;
const P: &str = r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships""#;
const OD: &str = r#"xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" xmlns:presentation="urn:oasis:names:tc:opendocument:xmlns:presentation:1.0" xmlns:dc="http://purl.org/dc/elements/1.1/" office:version="1.2""#;

/// A zip of `parts` (name, text), deflated as Office does; a `mimetype` part stored, as OpenDocument has it.
fn zip_of(parts: &[(&str, String)]) -> Vec<u8> {
    let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, body) in parts {
        let method = if *name == "mimetype" { zip::CompressionMethod::Stored } else { zip::CompressionMethod::Deflated };
        z.start_file(*name, zip::write::SimpleFileOptions::default().compression_method(method)).unwrap();
        z.write_all(body.as_bytes()).unwrap();
    }
    z.finish().unwrap().into_inner()
}

/// A relationships part: (id, the end of its type, target).
fn rels(items: &[(&str, &str, &str)]) -> String {
    let mut s = String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#);
    for (id, kind, target) in items {
        s.push_str(&format!(r#"<Relationship Id="{id}" Type="{REL}/{kind}" Target="{target}"/>"#));
    }
    s + "</Relationships>"
}

/// The file as the Chief's copy has it, `<dir>/<file id>/<name>`.
fn copy(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let d = dir.join(format!("id-{}", name.replace('.', "-")));
    std::fs::create_dir_all(&d).unwrap();
    let p = d.join(name);
    std::fs::write(&p, bytes).unwrap();
    p
}

/// The readable copy of `name` made from `bytes`: it is written beside the copy, and its text is returned.
fn readable_of(dir: &Path, name: &str, bytes: &[u8]) -> String {
    let p = copy(dir, name, bytes);
    let out = readable::ensure(&p).unwrap_or_else(|e| panic!("{name}: no readable copy: {e}"));
    assert_eq!(out, p.with_file_name(format!("{name}.md")), "beside the copy, with .md added to its name");
    std::fs::read_to_string(&out).unwrap()
}

/// Checks that `parts` come in `text` in this order.
fn in_order(text: &str, parts: &[&str]) {
    let mut at = 0;
    for part in parts {
        let found = text[at..].find(part).unwrap_or_else(|| panic!("{part:?} not found after byte {at} in:\n{text}"));
        at += found + part.len();
    }
}

fn first_line(name: &str) -> String {
    format!("Text of {name}, made by Hersir for reading: layout and pictures are left out.")
}

// ---- Word ----

fn wp(style: &str, text: &str) -> String {
    let ppr = if style.is_empty() { String::new() } else { format!(r#"<w:pPr><w:pStyle w:val="{style}"/></w:pPr>"#) };
    format!(r#"<w:p>{ppr}<w:r><w:t xml:space="preserve">{text}</w:t></w:r></w:p>"#)
}

fn wli(num: u32, lvl: u32, text: &str) -> String {
    format!(r#"<w:p><w:pPr><w:pStyle w:val="ListParagraph"/><w:numPr><w:ilvl w:val="{lvl}"/><w:numId w:val="{num}"/></w:numPr></w:pPr><w:r><w:t>{text}</w:t></w:r></w:p>"#)
}

fn wcell(inner: &str) -> String {
    format!(r#"<w:tc><w:tcPr><w:tcW w:w="2000" w:type="dxa"/></w:tcPr>{inner}</w:tc>"#)
}

const STYLES: &str = r#"<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style><w:style w:type="paragraph" w:styleId="Title"><w:name w:val="Title"/><w:basedOn w:val="Normal"/></w:style><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/></w:style><w:style w:type="paragraph" w:styleId="Kop2"><w:name w:val="heading 2"/><w:basedOn w:val="Normal"/></w:style><w:style w:type="paragraph" w:styleId="Heading6"><w:name w:val="heading 6"/></w:style><w:style w:type="paragraph" w:styleId="ListParagraph"><w:name w:val="List Paragraph"/></w:style><w:style w:type="character" w:styleId="Strong"><w:name w:val="Strong"/></w:style>"#;

const NUMBERING: &str = r#"<w:abstractNum w:abstractNumId="0"><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="bullet"/><w:lvlText w:val=""/></w:lvl><w:lvl w:ilvl="1"><w:start w:val="1"/><w:numFmt w:val="bullet"/><w:lvlText w:val="o"/></w:lvl></w:abstractNum><w:abstractNum w:abstractNumId="1"><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1."/></w:lvl></w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num><w:num w:numId="2"><w:abstractNumId w:val="1"/></w:num>"#;

/// A Word document with `body` (the inside of w:body) and the extra parts and relationships of word/document.xml.
fn docx(body: &str, extra: &[(&str, String)], extra_rels: &[(&str, &str, &str)]) -> Vec<u8> {
    let mut doc_rels = vec![("rId1", "styles", "styles.xml"), ("rId2", "numbering", "numbering.xml")];
    doc_rels.extend_from_slice(extra_rels);
    let mut parts = vec![
        ("[Content_Types].xml", r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#.to_string()),
        ("_rels/.rels", rels(&[("rId1", "officeDocument", "word/document.xml")])),
        ("word/document.xml", format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document {W}><w:body>{body}<w:sectPr><w:pgSz w:w="11906" w:h="16838"/></w:sectPr></w:body></w:document>"#)),
        ("word/_rels/document.xml.rels", rels(&doc_rels)),
        ("word/styles.xml", format!(r#"<w:styles {W}>{STYLES}</w:styles>"#)),
        ("word/numbering.xml", format!(r#"<w:numbering {W}>{NUMBERING}</w:numbering>"#)),
    ];
    parts.extend(extra.iter().cloned());
    zip_of(&parts)
}

/// The body of the report: title and headings (one through a localized style id), a paragraph with a tab and a line
/// break, a bulleted list with a nested item, a numbered list, a table, and a picture.
fn report_body() -> String {
    let picture = r#"<w:r><w:drawing><wp:inline><wp:extent cx="100" cy="100"/><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic><pic:blipFill><a:blip r:embed="rId9"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>"#;
    let table = format!(
        r#"<w:tbl><w:tblPr><w:tblW w:w="0" w:type="auto"/></w:tblPr><w:tblGrid><w:gridCol w:w="2000"/><w:gridCol w:w="2000"/></w:tblGrid><w:tr>{}{}</w:tr><w:tr>{}{}</w:tr><w:tr>{}{}</w:tr><w:tr>{}{}</w:tr></w:tbl>"#,
        wcell(&wp("", "Name")), wcell(&wp("", "Note")),
        wcell(&wp("", "Ann")), wcell(&wp("", "a | b")),
        wcell(&wp("", "Bob")), wcell(r#"<w:p><w:r><w:t>line one</w:t><w:br/><w:t>line two</w:t></w:r></w:p>"#),
        wcell(&wp("", "Cas")), wcell(&format!("{}{}", wp("", "first para"), wp("", "second para"))),
    );
    [
        wp("Title", "Quarterly report"),
        wp("Heading1", "Summary"),
        r#"<w:p><w:r><w:t>Region</w:t><w:tab/><w:t>North</w:t><w:br/><w:t>Second line</w:t></w:r></w:p>"#.to_string(),
        wp("Kop2", "Details"),
        wli(1, 0, "Apples"), wli(1, 1, "Green apples"), wli(1, 0, "Pears"),
        wp("", "Between the lists."),
        wli(2, 0, "First step"), wli(2, 0, "Second step"),
        wp("Heading6", "Deep"),
        table,
        format!(r#"<w:p><w:r><w:t xml:space="preserve">After the table. </w:t></w:r>{picture}</w:p>"#),
    ].concat()
}

#[test]
fn a_docx_gets_its_headings_paragraphs_lists_and_tables_in_order_under_the_first_line() {
    let dir = tempfile::tempdir().unwrap();
    let md = readable_of(dir.path(), "Report Q3.docx", &docx(&report_body(), &[], &[]));
    assert!(md.starts_with(&format!("{}\n\n", first_line("Report Q3.docx"))), "{md}");
    assert_eq!(md.lines().next().unwrap(), "Text of Report Q3.docx, made by Hersir for reading: layout and pictures are left out.");
    in_order(&md, &[
        "\n# Quarterly report\n",
        "\n# Summary\n",
        "\nRegion\tNorth\nSecond line\n",
        "\n## Details\n",
        "\n- Apples\n  - Green apples\n- Pears\n",
        "\nBetween the lists.\n",
        "\n1. First step\n2. Second step\n",
        "\n###### Deep\n",
        "\n| Name | Note |\n| --- | --- |\n| Ann | a \\| b |\n",
        "\nAfter the table. [picture]",
    ]);
    assert!(md.ends_with('\n'), "{md:?}");
}

#[test]
fn a_line_break_in_a_table_cell_becomes_a_space_and_the_table_stays_whole() {
    // the card: "In every table cell, | is written as \| and a line break becomes a space, so the table stays whole."
    let dir = tempfile::tempdir().unwrap();
    let md = readable_of(dir.path(), "table.docx", &docx(&report_body(), &[], &[]));
    let table: Vec<&str> = md.lines().skip_while(|l| !l.starts_with("| Name |")).take_while(|l| l.starts_with('|')).collect();
    assert_eq!(table.len(), 5, "the head, its rule and three rows, each on one line:\n{md}");
    assert!(table.iter().all(|l| l.ends_with('|') && l.matches(" | ").count() + l.matches("\\|").count() >= 1), "{table:?}");
    assert_eq!(table[2], "| Ann | a \\| b |", "{md}");
    assert_eq!(table[3], "| Bob | line one line two |", "a line break in a cell becomes a space:\n{md}");
    assert_eq!(table[4], "| Cas | first para second para |", "two paragraphs in a cell: a space between:\n{md}");
}

/// A Word document with a header and a footer, a footnote, an endnote, a comment and tracked changes.
fn notes_docx() -> Vec<u8> {
    let body = [
        r#"<w:p><w:r><w:t xml:space="preserve">Kept text </w:t></w:r><w:ins w:id="1" w:author="Ann" w:date="2026-11-01T10:00:00Z"><w:r><w:t>inserted words</w:t></w:r></w:ins><w:del w:id="2" w:author="Ann" w:date="2026-11-01T10:00:00Z"><w:r><w:delText>deleted words</w:delText></w:r></w:del><w:r><w:footnoteReference w:id="1"/></w:r><w:r><w:t xml:space="preserve"> and </w:t></w:r><w:r><w:endnoteReference w:id="1"/></w:r></w:p>"#,
        r#"<w:p><w:commentRangeStart w:id="0"/><w:r><w:t>Revenue grew.</w:t></w:r><w:commentRangeEnd w:id="0"/><w:r><w:commentReference w:id="0"/></w:r></w:p>"#,
        r#"<w:p><w:del w:id="3" w:author="Ann" w:date="2026-11-01T10:00:00Z"><w:r><w:delText>A whole deleted sentence.</w:delText></w:r></w:del></w:p>"#,
    ].concat();
    let sep = r#"<w:footnote w:type="separator" w:id="-1"><w:p><w:r><w:separator/></w:r></w:p></w:footnote><w:footnote w:type="continuationSeparator" w:id="0"><w:p><w:r><w:continuationSeparator/></w:r></w:p></w:footnote>"#;
    let esep = sep.replace("footnote", "endnote");
    docx(&body, &[
        ("word/header1.xml", format!(r#"<w:hdr {W}><w:p><w:r><w:t>Company confidential</w:t></w:r></w:p></w:hdr>"#)),
        ("word/footer1.xml", format!(r#"<w:ftr {W}><w:p><w:r><w:t>Page footer text</w:t></w:r></w:p></w:ftr>"#)),
        ("word/footnotes.xml", format!(r#"<w:footnotes {W}>{sep}<w:footnote w:id="1"><w:p><w:r><w:footnoteRef/></w:r><w:r><w:t xml:space="preserve"> Source: annual figures</w:t></w:r></w:p></w:footnote></w:footnotes>"#)),
        ("word/endnotes.xml", format!(r#"<w:endnotes {W}>{esep}<w:endnote w:id="1"><w:p><w:r><w:endnoteRef/></w:r><w:r><w:t xml:space="preserve"> See the appendix</w:t></w:r></w:p></w:endnote></w:endnotes>"#)),
        ("word/comments.xml", format!(r#"<w:comments {W}><w:comment w:id="0" w:author="Jef Sevinga" w:date="2026-11-02T09:00:00Z" w:initials="JS"><w:p><w:r><w:annotationRef/></w:r><w:r><w:t>Check this number</w:t></w:r></w:p></w:comment></w:comments>"#)),
    ], &[
        ("rId3", "header", "header1.xml"), ("rId4", "footer", "footer1.xml"), ("rId5", "footnotes", "footnotes.xml"),
        ("rId6", "endnotes", "endnotes.xml"), ("rId7", "comments", "comments.xml"),
    ])
}

#[test]
fn a_docx_with_headers_notes_comments_and_tracked_changes_puts_each_under_its_heading_with_insertions_and_without_deletions() {
    let dir = tempfile::tempdir().unwrap();
    let md = readable_of(dir.path(), "notes.docx", &notes_docx());
    assert!(md.starts_with(&first_line("notes.docx")), "{md}");
    in_order(&md, &[
        "Kept text inserted words",
        "Revenue grew.",
        "\n## Headers and footers\n",
        "Company confidential",
        "Page footer text",
        "\n## Footnotes\n",
        "Source: annual figures",
        "\n## Endnotes\n",
        "See the appendix",
        "\n## Comments\n",
        "\nJef Sevinga: Check this number",
    ]);
    assert!(!md.contains("deleted words") && !md.contains("A whole deleted sentence"), "tracked deletions are left out:\n{md}");
    // the separators of footnotes and endnotes aren't notes
    assert_eq!(md.matches("\n[^").count(), 2, "one footnote and one endnote:\n{md}");
}

#[test]
fn a_docx_without_notes_has_none_of_those_headings() {
    let dir = tempfile::tempdir().unwrap();
    let md = readable_of(dir.path(), "plain.docx", &docx(&wp("", "Only this."), &[], &[]));
    assert_eq!(md, format!("{}\n\nOnly this.\n", first_line("plain.docx")));
}

#[test]
fn a_docm_and_a_dotx_are_read_as_word_and_the_extension_case_is_ignored() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["macro.docm", "template.dotx", "SHOUT.DOCX"] {
        let md = readable_of(dir.path(), name, &docx(&wp("Heading1", "Hello"), &[], &[]));
        assert_eq!(md, format!("{}\n\n# Hello\n", first_line(name)), "{name}");
    }
}

#[test]
fn the_xml_reader_fetches_nothing_a_document_points_at() {
    let dir = tempfile::tempdir().unwrap();
    let secret = dir.path().join("secret.txt");
    std::fs::write(&secret, "SECRET-CONTENT").unwrap();
    let url = format!("file://{}", secret.display().to_string().replace('\\', "/"));
    let doc = format!(r#"<?xml version="1.0"?><!DOCTYPE w:document [<!ENTITY secret SYSTEM "{url}"><!ENTITY inner "INNER-ENTITY">]><w:document {W}><w:body><w:p><w:r><w:t>Before &secret; &inner; after &amp; done &#233;</w:t></w:r></w:p></w:body></w:document>"#);
    let bytes = zip_of(&[
        ("_rels/.rels", rels(&[("rId1", "officeDocument", "word/document.xml")])),
        ("word/document.xml", doc),
    ]);
    let md = readable_of(dir.path(), "xxe.docx", &bytes);
    assert!(md.contains("Before") && md.contains("after & done é"), "{md}");
    assert!(!md.contains("SECRET-CONTENT") && !md.contains("INNER-ENTITY"), "no entity is looked up:\n{md}");
}

// ---- spreadsheets ----

fn istr(cell: &str, text: &str) -> String {
    format!(r#"<c r="{cell}" t="inlineStr"><is><t>{text}</t></is></c>"#)
}

/// An Excel workbook with the sheets (name, the inside of sheetData), a date style (s="1") and one shared string.
fn xlsx(sheets: &[(&str, String)]) -> Vec<u8> {
    let mut wb_sheets = String::new();
    let mut wb_rels: Vec<(String, String, String)> = vec![];
    let mut parts: Vec<(String, String)> = vec![];
    for (i, (name, data)) in sheets.iter().enumerate() {
        wb_sheets.push_str(&format!(r#"<sheet name="{name}" sheetId="{}" r:id="rId{}"/>"#, i + 1, i + 1));
        wb_rels.push((format!("rId{}", i + 1), "worksheet".into(), format!("worksheets/sheet{}.xml", i + 1)));
        parts.push((format!("xl/worksheets/sheet{}.xml", i + 1), format!(r#"<?xml version="1.0"?><worksheet {SS}><sheetData>{data}</sheetData></worksheet>"#)));
    }
    wb_rels.push(("rId90".into(), "styles".into(), "styles.xml".into()));
    wb_rels.push(("rId91".into(), "sharedStrings".into(), "sharedStrings.xml".into()));
    let wb_rels: Vec<(&str, &str, &str)> = wb_rels.iter().map(|(a, b, c)| (a.as_str(), b.as_str(), c.as_str())).collect();
    let mut all: Vec<(&str, String)> = vec![
        ("[Content_Types].xml", r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/></Types>"#.to_string()),
        ("_rels/.rels", rels(&[("rId1", "officeDocument", "xl/workbook.xml")])),
        ("xl/workbook.xml", format!(r#"<?xml version="1.0"?><workbook {SS}><sheets>{wb_sheets}</sheets></workbook>"#)),
        ("xl/_rels/workbook.xml.rels", rels(&wb_rels)),
        ("xl/styles.xml", format!(r#"<?xml version="1.0"?><styleSheet {SS}><cellXfs count="2"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/><xf numFmtId="14" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"/></cellXfs></styleSheet>"#)),
        ("xl/sharedStrings.xml", format!(r#"<?xml version="1.0"?><sst {SS} count="1" uniqueCount="1"><si><t>Lead</t></si></sst>"#)),
    ];
    all.extend(parts.iter().map(|(n, b)| (n.as_str(), b.clone())));
    zip_of(&all)
}

fn sales_and_team() -> Vec<u8> {
    let sales = format!(
        r#"<row r="1">{}{}{}{}</row><row r="2">{}<c r="B2" s="1"><v>45000</v></c><c r="C2"><v>3</v></c><c r="D2"><f>C2*2.5</f><v>7.5</v></c></row><row r="3">{}<c r="B3" s="1"><v>45001</v></c><c r="C3"><v>4</v></c><c r="D3"><f>SUM(C2:C3)*2</f><v>14</v></c></row>"#,
        istr("A1", "Item"), istr("B1", "Date"), istr("C1", "Qty"), istr("D1", "Total"), istr("A2", "Apples"), istr("A3", "a|b"));
    let team = format!(r#"<row r="1">{}{}</row><row r="2">{}<c r="B2" t="s"><v>0</v></c></row>"#, istr("A1", "Name"), istr("B1", "Role"), istr("A2", "Ann"));
    xlsx(&[("Sales", sales), ("Team", team)])
}

#[test]
fn an_xlsx_with_two_sheets_shows_each_as_a_table_under_its_name_formulas_as_values_and_dates_as_dates() {
    let dir = tempfile::tempdir().unwrap();
    let md = readable_of(dir.path(), "Figures.xlsx", &sales_and_team());
    assert_eq!(md, format!("{}\n\n## Sales\n\n\
        | Item | Date | Qty | Total |\n| --- | --- | --- | --- |\n| Apples | 2023-03-15 | 3 | 7.5 |\n| a\\|b | 2023-03-16 | 4 | 14 |\n\n\
        ## Team\n\n| Name | Role |\n| --- | --- |\n| Ann | Lead |\n", first_line("Figures.xlsx")));
    assert!(!md.contains("45000") && !md.contains("SUM("), "no serial numbers and no formulas:\n{md}");
}

#[test]
fn an_xlsm_is_read_as_excel_too() {
    let dir = tempfile::tempdir().unwrap();
    let md = readable_of(dir.path(), "macros.XLSM", &sales_and_team());
    assert!(md.contains("\n## Sales\n") && md.contains("| Apples | 2023-03-15 | 3 | 7.5 |"), "{md}");
}

#[test]
fn a_sheet_with_more_than_5000_rows_stops_at_5000_and_says_how_many_more_there_are() {
    let dir = tempfile::tempdir().unwrap();
    let rows: String = (1..=5_003).map(|n| format!(r#"<row r="{n}"><c r="A{n}"><v>{n}</v></c><c r="B{n}"><v>{}</v></c></row>"#, n * 10)).collect();
    let small = format!(r#"<row r="1">{}</row>"#, istr("A1", "after"));
    let md = readable_of(dir.path(), "big.xlsx", &xlsx(&[("Rows", rows), ("Next", small)]));
    assert_eq!(readable::MAX_SHEET_ROWS, 5_000);
    let table: Vec<&str> = md.lines().filter(|l| l.starts_with("| ") && !l.starts_with("| ---")).collect();
    assert_eq!(table.len(), 5_000 + 1, "5,000 rows of the first sheet and one of the second:\n{}", &md[md.len() - 300..]);
    assert!(md.contains("\n| 5000 | 50000 |\n") && !md.contains("| 5001 |"), "{}", &md[md.len() - 300..]);
    in_order(&md, &["| 5000 | 50000 |\n\n(… 3 more rows left out)\n", "\n## Next\n", "| after |"]);
}

#[test]
fn an_ods_gets_its_sheets_as_tables_with_dates_as_dates() {
    let dir = tempfile::tempdir().unwrap();
    let content = format!(r#"<?xml version="1.0" encoding="UTF-8"?><office:document-content {OD}><office:body><office:spreadsheet><table:table table:name="Budget"><table:table-column table:number-columns-repeated="3"/><table:table-row><table:table-cell office:value-type="string"><text:p>Item</text:p></table:table-cell><table:table-cell office:value-type="string"><text:p>Cost</text:p></table:table-cell><table:table-cell office:value-type="string"><text:p>Due</text:p></table:table-cell></table:table-row><table:table-row><table:table-cell office:value-type="string"><text:p>Rent</text:p></table:table-cell><table:table-cell office:value-type="float" office:value="1200"><text:p>1200</text:p></table:table-cell><table:table-cell office:value-type="date" office:date-value="2023-03-15"><text:p>15-03-23</text:p></table:table-cell></table:table-row></table:table></office:spreadsheet></office:body></office:document-content>"#);
    let bytes = zip_of(&[
        ("mimetype", "application/vnd.oasis.opendocument.spreadsheet".into()),
        ("content.xml", content),
        ("META-INF/manifest.xml", r#"<?xml version="1.0"?><manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0"><manifest:file-entry manifest:full-path="/" manifest:media-type="application/vnd.oasis.opendocument.spreadsheet"/></manifest:manifest>"#.into()),
    ]);
    let md = readable_of(dir.path(), "budget.ods", &bytes);
    assert_eq!(md, format!("{}\n\n## Budget\n\n| Item | Cost | Due |\n| --- | --- | --- |\n| Rent | 1200 | 2023-03-15 |\n", first_line("budget.ods")));
}

/// One BIFF8 record.
fn biff(out: &mut Vec<u8>, typ: u16, data: &[u8]) {
    out.extend_from_slice(&typ.to_le_bytes());
    out.extend_from_slice(&(data.len() as u16).to_le_bytes());
    out.extend_from_slice(data);
}

/// An old Excel (.xls, BIFF8) workbook with the sheets (name, rows of cells: text, or a number when it parses as one).
fn xls(path: &Path, sheets: &[(&str, &[&[&str]])]) {
    let bof = |dt: u16| [&0x0600u16.to_le_bytes()[..], &dt.to_le_bytes(), &[0u8; 12]].concat();
    let mut globals = Vec::new();
    biff(&mut globals, 0x0809, &bof(0x0005));
    biff(&mut globals, 0x0042, &1200u16.to_le_bytes());
    let mut at_pos = Vec::new();
    for (name, _) in sheets {
        let mut d = vec![0u8; 4];
        d.extend_from_slice(&[0, 0, name.len() as u8, 0]);
        d.extend_from_slice(name.as_bytes());
        at_pos.push(globals.len() + 4);
        biff(&mut globals, 0x0085, &d);
    }
    biff(&mut globals, 0x000A, &[]);
    let mut stream = globals;
    for (i, (_, rows)) in sheets.iter().enumerate() {
        let pos = (stream.len() as u32).to_le_bytes();
        stream[at_pos[i]..at_pos[i] + 4].copy_from_slice(&pos);
        biff(&mut stream, 0x0809, &bof(0x0010));
        for (r, row) in rows.iter().enumerate() {
            for (c, v) in row.iter().enumerate() {
                let head = [(r as u16).to_le_bytes(), (c as u16).to_le_bytes(), 0u16.to_le_bytes()].concat();
                match v.parse::<f64>() {
                    Ok(n) => biff(&mut stream, 0x0203, &[head, n.to_le_bytes().to_vec()].concat()),
                    Err(_) => biff(&mut stream, 0x0204, &[head, (v.len() as u16).to_le_bytes().to_vec(), vec![0], v.as_bytes().to_vec()].concat()),
                }
            }
        }
        biff(&mut stream, 0x000A, &[]);
    }
    let mut comp = cfb::create(path).unwrap();
    let mut s = comp.create_stream("/Workbook").unwrap();
    s.write_all(&stream).unwrap();
    drop(s);
    comp.flush().unwrap();
}

#[test]
fn an_old_xls_gets_a_text_copy_too() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path().join("id-xls");
    std::fs::create_dir_all(&d).unwrap();
    let p = d.join("Old figures.xls");
    xls(&p, &[("Sheet1", &[&["Item", "Qty"], &["Pears", "12"]]), ("Other", &[&["Note"], &["kept"]])]);
    let out = readable::ensure(&p).unwrap_or_else(|e| panic!("no readable copy: {e}"));
    let md = std::fs::read_to_string(out).unwrap();
    assert_eq!(md, format!("{}\n\n## Sheet1\n\n| Item | Qty |\n| --- | --- |\n| Pears | 12 |\n\n## Other\n\n| Note |\n| --- |\n| kept |\n",
                           first_line("Old figures.xls")));
}

/// One XLSB record: its type and length as 7-bit numbers, then its data.
fn brt(out: &mut Vec<u8>, typ: u16, data: &[u8]) {
    if typ < 0x80 {
        out.push(typ as u8);
    } else {
        out.push((typ & 0x7F) as u8 | 0x80);
        out.push((typ >> 7) as u8);
    }
    let mut len = data.len();
    loop {
        let b = (len & 0x7F) as u8;
        len >>= 7;
        if len == 0 {
            out.push(b);
            break;
        }
        out.push(b | 0x80);
    }
    out.extend_from_slice(data);
}

fn wide(s: &str) -> Vec<u8> {
    let u: Vec<u16> = s.encode_utf16().collect();
    let mut v = (u.len() as u32).to_le_bytes().to_vec();
    for c in u {
        v.extend_from_slice(&c.to_le_bytes());
    }
    v
}

/// An Excel binary workbook (.xlsb) with one sheet: a text cell and a number cell under a head row.
fn xlsb() -> Vec<u8> {
    let mut wb = Vec::new();
    brt(&mut wb, 0x0083, &[]);
    brt(&mut wb, 0x009C, &[0u32.to_le_bytes().to_vec(), 1u32.to_le_bytes().to_vec(), wide("rId1"), wide("Binary")].concat());
    brt(&mut wb, 0x0090, &[]);
    brt(&mut wb, 0x009D, &[0u8; 8]);
    brt(&mut wb, 0x0084, &[]);
    let mut sh = Vec::new();
    brt(&mut sh, 0x0081, &[]);
    brt(&mut sh, 0x0094, &[0u32.to_le_bytes(), 1u32.to_le_bytes(), 0u32.to_le_bytes(), 1u32.to_le_bytes()].concat());
    brt(&mut sh, 0x0091, &[]);
    let cell = |col: u32| [col.to_le_bytes(), 0u32.to_le_bytes()].concat();
    brt(&mut sh, 0x0000, &[0u32.to_le_bytes().to_vec(), vec![0; 13]].concat());
    brt(&mut sh, 0x0006, &[cell(0), wide("Product")].concat());
    brt(&mut sh, 0x0006, &[cell(1), wide("Price")].concat());
    brt(&mut sh, 0x0000, &[1u32.to_le_bytes().to_vec(), vec![0; 13]].concat());
    brt(&mut sh, 0x0006, &[cell(0), wide("Tea")].concat());
    brt(&mut sh, 0x0005, &[cell(1), 2.25f64.to_le_bytes().to_vec()].concat());
    brt(&mut sh, 0x0092, &[]);
    brt(&mut sh, 0x0082, &[]);
    // zip_of takes text: the binary parts go through a zip of their own
    let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opts = || zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (name, body) in [
        ("[Content_Types].xml", r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"/>"#.as_bytes().to_vec()),
        ("_rels/.rels", rels(&[("rId1", "officeDocument", "xl/workbook.bin")]).into_bytes()),
        ("xl/workbook.bin", wb),
        ("xl/_rels/workbook.bin.rels", rels(&[("rId1", "worksheet", "worksheets/sheet1.bin")]).into_bytes()),
        ("xl/worksheets/sheet1.bin", sh),
    ] {
        z.start_file(name, opts()).unwrap();
        z.write_all(&body).unwrap();
    }
    z.finish().unwrap().into_inner()
}

#[test]
fn an_xlsb_gets_a_text_copy_too() {
    let dir = tempfile::tempdir().unwrap();
    let md = readable_of(dir.path(), "binary.xlsb", &xlsb());
    assert_eq!(md, format!("{}\n\n## Binary\n\n| Product | Price |\n| --- | --- |\n| Tea | 2.25 |\n", first_line("binary.xlsb")));
}

// ---- PowerPoint ----

fn shape(ph: &str, paras: &[&str]) -> String {
    let ps: String = paras.iter().map(|t| format!(r#"<a:p><a:r><a:rPr lang="en-US"/><a:t>{t}</a:t></a:r></a:p>"#)).collect();
    format!(r#"<p:sp><p:nvSpPr><p:cNvPr id="2" name="S"/><p:cNvSpPr/><p:nvPr><p:ph type="{ph}"/></p:nvPr></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/>{ps}</p:txBody></p:sp>"#)
}

fn slide(title: &str, body: &[&str]) -> String {
    // the body before the title in the XML: the title still comes first
    format!(r#"<?xml version="1.0"?><p:sld {P}><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>{}{}</p:spTree></p:cSld></p:sld>"#,
            shape("body", body), shape("title", &[title]))
}

fn notes(text: &str) -> String {
    format!(r#"<?xml version="1.0"?><p:notes {P}><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/><p:sp><p:nvSpPr><p:cNvPr id="2" name="Slide Image"/><p:cNvSpPr/><p:nvPr><p:ph type="sldImg"/></p:nvPr></p:nvSpPr><p:spPr/></p:sp>{}<p:sp><p:nvSpPr><p:cNvPr id="4" name="Number"/><p:cNvSpPr/><p:nvPr><p:ph type="sldNum"/></p:nvPr></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:p><a:r><a:t>99</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:notes>"#,
            shape("body", &[text]))
}

fn pptx() -> Vec<u8> {
    // slide order: slide3.xml, slide1.xml, slide2.xml (the order of p:sldIdLst, not of the part names)
    zip_of(&[
        ("_rels/.rels", rels(&[("rId1", "officeDocument", "ppt/presentation.xml")])),
        ("ppt/presentation.xml", format!(r#"<?xml version="1.0"?><p:presentation {P}><p:sldIdLst><p:sldId id="256" r:id="rId12"/><p:sldId id="257" r:id="rId10"/><p:sldId id="258" r:id="rId11"/></p:sldIdLst></p:presentation>"#)),
        ("ppt/_rels/presentation.xml.rels", rels(&[("rId10", "slide", "slides/slide1.xml"), ("rId11", "slide", "slides/slide2.xml"), ("rId12", "slide", "slides/slide3.xml")])),
        ("ppt/slides/slide3.xml", slide("Welcome", &["Who we are", "What we do"])),
        ("ppt/slides/slide1.xml", slide("The plan", &["Build it"])),
        ("ppt/slides/slide2.xml", slide("Questions", &["Ask away"])),
        ("ppt/slides/_rels/slide3.xml.rels", rels(&[("rId1", "notesSlide", "../notesSlides/notesSlide1.xml")])),
        ("ppt/slides/_rels/slide1.xml.rels", rels(&[("rId1", "notesSlide", "../notesSlides/notesSlide2.xml")])),
        ("ppt/slides/_rels/slide2.xml.rels", rels(&[("rId1", "notesSlide", "../notesSlides/notesSlide3.xml")])),
        ("ppt/notesSlides/notesSlide1.xml", notes("Say hello first")),
        ("ppt/notesSlides/notesSlide2.xml", notes("Mention the dates")),
        ("ppt/notesSlides/notesSlide3.xml", notes("Leave ten minutes")),
    ])
}

#[test]
fn a_pptx_with_three_slides_and_notes_has_slide_1_to_3_in_slide_order_each_title_first_with_its_notes() {
    let dir = tempfile::tempdir().unwrap();
    let md = readable_of(dir.path(), "Pitch.pptx", &pptx());
    assert!(md.starts_with(&first_line("Pitch.pptx")), "{md}");
    in_order(&md, &[
        "\n## Slide 1\n", "Welcome", "Who we are", "What we do", "Notes: Say hello first",
        "\n## Slide 2\n", "The plan", "Build it", "Notes: Mention the dates",
        "\n## Slide 3\n", "Questions", "Ask away", "Notes: Leave ten minutes",
    ]);
    assert_eq!(md.matches("\n## Slide ").count(), 3, "{md}");
    assert!(!md.contains("99"), "a notes page's slide number isn't a note:\n{md}");
}

// ---- OpenDocument text and presentations, RTF ----

#[test]
fn an_odt_is_read_like_word() {
    let dir = tempfile::tempdir().unwrap();
    let content = format!(r#"<?xml version="1.0" encoding="UTF-8"?><office:document-content {OD}><office:automatic-styles><text:list-style style:name="L2"><text:list-level-style-number text:level="1" style:num-format="1"/></text:list-style></office:automatic-styles><office:body><office:text><text:h text:outline-level="1">Meeting notes</text:h><text:h text:outline-level="2">Monday</text:h><text:p>We met<text:tab/>at ten.<text:note text:id="ftn1" text:note-class="footnote"><text:note-citation>1</text:note-citation><text:note-body><text:p>In the office.</text:p></text:note-body></text:note></text:p><text:list><text:list-item><text:p>Budget</text:p></text:list-item><text:list-item><text:p>Hiring</text:p></text:list-item></text:list><text:list text:style-name="L2"><text:list-item><text:p>Open the meeting</text:p></text:list-item><text:list-item><text:p>Close it</text:p></text:list-item></text:list><table:table table:name="T1"><table:table-column table:number-columns-repeated="2"/><table:table-row><table:table-cell><text:p>Who</text:p></table:table-cell><table:table-cell><text:p>What</text:p></table:table-cell></table:table-row><table:table-row><table:table-cell><text:p>Ann</text:p></table:table-cell><table:table-cell><text:p>x | y</text:p></table:table-cell></table:table-row></table:table><text:p>Signed.<office:annotation><dc:creator>Ann</dc:creator><text:p>Who signed?</text:p></office:annotation></text:p></office:text></office:body></office:document-content>"#);
    let bytes = zip_of(&[("mimetype", "application/vnd.oasis.opendocument.text".into()), ("content.xml", content)]);
    let md = readable_of(dir.path(), "notes.odt", &bytes);
    assert!(md.starts_with(&first_line("notes.odt")), "{md}");
    in_order(&md, &[
        "\n# Meeting notes\n", "\n## Monday\n", "\nWe met\tat ten.[^1]\n", "\n- Budget\n- Hiring\n",
        "1. Open the meeting\n2. Close it\n", "\n| Who | What |\n| --- | --- |\n| Ann | x \\| y |\n", "Signed.",
        "\n## Footnotes\n", "In the office.", "\n## Comments\n", "Ann: Who signed?",
    ]);
}

#[test]
fn an_odp_is_read_like_powerpoint() {
    let dir = tempfile::tempdir().unwrap();
    let page = |title: &str, point: &str, note: &str| format!(r#"<draw:page draw:name="p"><draw:frame presentation:class="outline"><draw:text-box><text:list><text:list-item><text:p>{point}</text:p></text:list-item></text:list></draw:text-box></draw:frame><draw:frame presentation:class="title"><draw:text-box><text:p>{title}</text:p></draw:text-box></draw:frame><presentation:notes><draw:page-thumbnail/><draw:frame presentation:class="notes"><draw:text-box><text:p>{note}</text:p></draw:text-box></draw:frame></presentation:notes></draw:page>"#);
    let content = format!(r#"<?xml version="1.0" encoding="UTF-8"?><office:document-content {OD}><office:body><office:presentation>{}{}</office:presentation></office:body></office:document-content>"#,
                          page("Kick-off", "Goals", "Smile"), page("Next steps", "Plan the sprint", "Keep it short"));
    let bytes = zip_of(&[("mimetype", "application/vnd.oasis.opendocument.presentation".into()), ("content.xml", content)]);
    let md = readable_of(dir.path(), "deck.odp", &bytes);
    assert!(md.starts_with(&first_line("deck.odp")), "{md}");
    in_order(&md, &["\n## Slide 1\n", "Kick-off", "Goals", "Notes: Smile", "\n## Slide 2\n", "Next steps", "Plan the sprint", "Notes: Keep it short"]);
}

#[test]
fn an_rtf_gets_its_plain_text_with_its_paragraphs() {
    let dir = tempfile::tempdir().unwrap();
    let rtf = r"{\rtf1\ansi\ansicpg1252\deff0{\fonttbl{\f0\fswiss Arial;}}{\colortbl;\red0\green0\blue0;}{\info{\title Not shown}}{\*\generator Some writer;}\pard\f0 First paragraph with caf\'e9.\par Second \b bold\b0  paragraph.\par\pard Third \u8364? euro.\par}";
    let md = readable_of(dir.path(), "letter.rtf", rtf.as_bytes());
    assert_eq!(md, format!("{}\n\nFirst paragraph with café.\n\nSecond bold paragraph.\n\nThird € euro.\n", first_line("letter.rtf")));
}

#[test]
fn a_readable_copy_is_at_most_20_mb_and_a_longer_text_is_cut_at_a_line_end_with_a_last_line_that_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let line = "Paragraph with some words to fill the line up to the end.";
    let mut rtf = String::from(r"{\rtf1\ansi ");
    let n = readable::MAX_TEXT_BYTES / (line.len() + 2) + 10_000;
    for _ in 0..n {
        rtf.push_str(line);
        rtf.push_str("\\par\n");
    }
    rtf.push('}');
    let md = readable_of(dir.path(), "long.rtf", rtf.as_bytes());
    assert!(md.len() <= readable::MAX_TEXT_BYTES, "{} bytes", md.len());
    assert!(md.len() > readable::MAX_TEXT_BYTES - 10_000, "{} bytes", md.len());
    assert!(md.ends_with(&format!("\n\n{}\n", readable::CUT_LINE)), "{:?}", &md[md.len() - 200..]);
    let before = md[..md.len() - readable::CUT_LINE.len() - 1].trim_end();
    assert!(before.ends_with(&format!("\n\n{line}")), "cut at the end of a line: {:?}", &before[before.len() - 200..]);
}

// ---- table cells in every kind of document (the fix round: a line break is a space, `|` is `\|`) ----

/// The lines of the first Markdown table in `md` that starts with `head`.
fn table_lines<'a>(md: &'a str, head: &str) -> Vec<&'a str> {
    md.lines().skip_while(|l| !l.starts_with(head)).take_while(|l| l.starts_with('|')).collect()
}

#[test]
fn a_line_break_in_a_table_cell_is_a_space_in_odt_pptx_and_xlsx_tables_too() {
    let dir = tempfile::tempdir().unwrap();
    let cut = |md: &str| assert!(!md.contains("<br"), "no <br> anywhere:\n{md}");

    // OpenDocument text: a text:line-break and two paragraphs in one cell
    let cell = |inner: &str| format!("<table:table-cell>{inner}</table:table-cell>");
    let odt_table = format!(
        r#"<table:table table:name="T"><table:table-column table:number-columns-repeated="2"/><table:table-row>{}{}</table:table-row><table:table-row>{}{}</table:table-row><table:table-row>{}{}</table:table-row></table:table>"#,
        cell("<text:p>Who</text:p>"), cell("<text:p>Note</text:p>"),
        cell("<text:p>Ann</text:p>"), cell("<text:p>line one<text:line-break/>line two | three</text:p>"),
        cell("<text:p>Bob</text:p>"), cell("<text:p>first para</text:p><text:p>second para</text:p>"),
    );
    let content = format!(r#"<?xml version="1.0" encoding="UTF-8"?><office:document-content {OD}><office:body><office:text><text:p>Before.</text:p>{odt_table}<text:p>After.</text:p></office:text></office:body></office:document-content>"#);
    let md = readable_of(dir.path(), "cells.odt", &zip_of(&[("mimetype", "application/vnd.oasis.opendocument.text".into()), ("content.xml", content)]));
    cut(&md);
    assert_eq!(table_lines(&md, "| Who |"), ["| Who | Note |", "| --- | --- |", "| Ann | line one line two \\| three |", "| Bob | first para second para |"], "{md}");
    in_order(&md, &["Before.", "| Who | Note |", "After."]);

    // PowerPoint: a table on a slide, an a:br and two paragraphs in one cell
    let tc = |paras: &[&str]| {
        let ps: String = paras.iter().map(|p| format!("<a:p>{p}</a:p>")).collect();
        format!("<a:tc><a:txBody><a:bodyPr/>{ps}</a:txBody><a:tcPr/></a:tc>")
    };
    let r = |t: &str| format!("<a:r><a:t>{t}</a:t></a:r>");
    let table = format!(
        r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="5" name="Table"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr><p:xfrm/><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/table"><a:tbl><a:tblGrid><a:gridCol w="100"/><a:gridCol w="100"/></a:tblGrid><a:tr h="10">{}{}</a:tr><a:tr h="10">{}{}</a:tr><a:tr h="10">{}{}</a:tr></a:tbl></a:graphicData></a:graphic></p:graphicFrame>"#,
        tc(&[&r("Who")]), tc(&[&r("Note")]),
        tc(&[&r("Ann")]), tc(&[&format!("{}<a:br/>{}", r("line one"), r("line two | three"))]),
        tc(&[&r("Bob")]), tc(&[&r("first para"), &r("second para")]),
    );
    let slide1 = format!(r#"<?xml version="1.0"?><p:sld {P}><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>{}{table}</p:spTree></p:cSld></p:sld>"#, shape("title", &["Team"]));
    let bytes = zip_of(&[
        ("_rels/.rels", rels(&[("rId1", "officeDocument", "ppt/presentation.xml")])),
        ("ppt/presentation.xml", format!(r#"<?xml version="1.0"?><p:presentation {P}><p:sldIdLst><p:sldId id="256" r:id="rId10"/></p:sldIdLst></p:presentation>"#)),
        ("ppt/_rels/presentation.xml.rels", rels(&[("rId10", "slide", "slides/slide1.xml")])),
        ("ppt/slides/slide1.xml", slide1),
    ]);
    let md = readable_of(dir.path(), "cells.pptx", &bytes);
    cut(&md);
    assert_eq!(table_lines(&md, "| Who |"), ["| Who | Note |", "| --- | --- |", "| Ann | line one line two \\| three |", "| Bob | first para second para |"], "{md}");
    in_order(&md, &["## Slide 1", "Team", "| Who | Note |"]);

    // Excel: a cell with a line break (Alt+Enter), a CR LF and a `|`
    let sheet = format!(r#"<row r="1">{}{}</row><row r="2">{}{}</row><row r="3">{}{}</row>"#,
                        istr("A1", "Who"), istr("B1", "Note"), istr("A2", "Ann"), istr("B2", "line one\nline two | three"),
                        istr("A3", "Bob"), istr("B3", "first&#13;\nsecond"));
    let md = readable_of(dir.path(), "cells.xlsx", &xlsx(&[("Notes", sheet)]));
    cut(&md);
    assert_eq!(table_lines(&md, "| Who |"), ["| Who | Note |", "| --- | --- |", "| Ann | line one line two \\| three |", "| Bob | first second |"], "{md}");
}

// ---- files it can't read, and when a copy is made ----

#[test]
fn a_damaged_a_password_protected_and_a_renamed_file_get_no_readable_copy_and_a_short_reason() {
    let dir = tempfile::tempdir().unwrap();
    let whole = sales_and_team();
    let mut locked = vec![0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
    locked.resize(4096, 0);
    for (name, bytes, says) in [
        ("damaged.xlsx", whole[..whole.len() / 2].to_vec(), "it can't be opened as an Excel workbook"),
        ("locked.docx", locked, "protected with a password"),
        ("renamed.docx", b"just some text, called .docx".to_vec(), "it can't be opened as a Word document"),
        ("empty.pptx", vec![], "the file is empty"),
        ("text.rtf", b"not rtf at all".to_vec(), "it can't be opened as an RTF document"),
    ] {
        let p = copy(dir.path(), name, &bytes);
        let why = readable::ensure(&p).expect_err(name);
        assert!(why.contains(says), "{name}: {why}");
        assert!(!why.contains('\n') && why.len() < 300, "{name}: short and on one line: {why:?}");
        assert!(!readable::readable_path(&p).exists(), "{name}: no readable copy");
        let left: Vec<String> = std::fs::read_dir(p.parent().unwrap()).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(left, [name.to_string()], "{name}: nothing else beside the copy");
        assert_eq!(std::fs::read(&p).unwrap(), bytes, "{name}: the copy is untouched");
    }
}

#[test]
fn a_readable_copy_is_made_once_kept_after_that_and_made_again_only_when_it_is_missing() {
    let dir = tempfile::tempdir().unwrap();
    // a file copied before readable copies existed: no .md beside it yet
    let p = copy(dir.path(), "plan.docx", &docx(&wp("", "Version one."), &[], &[]));
    let out = readable::readable_path(&p);
    assert_eq!(out, p.parent().unwrap().join("plan.docx.md"));
    assert!(!out.exists());
    assert_eq!(readable::ensure(&p).unwrap(), out);
    assert!(std::fs::read_to_string(&out).unwrap().contains("Version one."));

    // kept as it is: not made again
    std::fs::write(&out, "kept").unwrap();
    assert_eq!(readable::ensure(&p).unwrap(), out);
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "kept");

    // made again when it is gone
    std::fs::remove_file(&out).unwrap();
    assert_eq!(readable::ensure(&p).unwrap(), out);
    assert!(std::fs::read_to_string(&out).unwrap().contains("Version one."));
    let names: Vec<String> = std::fs::read_dir(p.parent().unwrap()).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(names.len(), 2, "the copy and its readable copy, no temp file left: {names:?}");
}

#[test]
fn only_word_excel_powerpoint_opendocument_and_rtf_files_get_a_readable_copy_by_their_extension_case_ignored() {
    for name in ["a.docx", "a.docm", "a.dotx", "a.xlsx", "a.xlsm", "a.xlsb", "a.xls", "a.ods", "a.pptx", "a.odt", "a.odp", "a.rtf",
                 "Report.DOCX", "x.Xlsx", "deck.PPTX", "letter.RTF", "my file.with.dots.odt"] {
        assert!(readable::has_readable(name), "{name}");
    }
    for name in ["a.png", "a.pdf", "a.txt", "a.md", "a.csv", "a.json", "a.doc", "a.ppt", "a.pages", "a.key", "a.numbers",
                 "a.docx.md", "docx", "a.zip", "a.html", "noext"] {
        assert!(!readable::has_readable(name), "{name}");
    }
}
