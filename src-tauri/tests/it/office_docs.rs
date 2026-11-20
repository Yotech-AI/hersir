//! GA-310 QA: small Office files for the tests of readable copies (chat_files_flow_test, lead_files_test): a Word
//! document as a zip of its XML parts, stored (this test program has no zip crate), and files Hersir can't read.
#![allow(dead_code)]

const W: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships""#;
const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

/// A zip of `parts` (name, text), each stored as it is.
pub fn stored_zip(parts: &[(&str, String)]) -> Vec<u8> {
    let (mut out, mut central) = (Vec::new(), Vec::new());
    for (name, body) in parts {
        let (name, body) = (name.as_bytes(), body.as_bytes());
        let (crc, size, offset) = (crc32(body), body.len() as u32, out.len() as u32);
        // version 2.0, no flags, stored, 1980-01-01
        let common = [&20u16.to_le_bytes()[..], &[0, 0, 0, 0, 0, 0], &0x21u16.to_le_bytes(), &crc.to_le_bytes(), &size.to_le_bytes(),
                      &size.to_le_bytes(), &(name.len() as u16).to_le_bytes()].concat();
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        out.extend_from_slice(&common);
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(name);
        out.extend_from_slice(body);
        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&common);
        central.extend_from_slice(&[0u8; 12]);
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name);
    }
    let (cd_offset, cd_size, n) = (out.len() as u32, central.len() as u32, parts.len() as u16);
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0u8; 4]);
    out.extend_from_slice(&n.to_le_bytes());
    out.extend_from_slice(&n.to_le_bytes());
    out.extend_from_slice(&cd_size.to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

fn rels(items: &[(&str, &str, &str)]) -> String {
    let mut s = String::from(r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#);
    for (id, kind, target) in items {
        s.push_str(&format!(r#"<Relationship Id="{id}" Type="{REL}/{kind}" Target="{target}"/>"#));
    }
    s + "</Relationships>"
}

/// A Word document: a Heading 1, a paragraph and a table (Name, Role / Ann, Lead).
pub fn docx(heading: &str, text: &str) -> Vec<u8> {
    let cell = |t: &str| format!("<w:tc><w:p><w:r><w:t>{t}</w:t></w:r></w:p></w:tc>");
    let doc = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document {W}><w:body><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>{heading}</w:t></w:r></w:p><w:p><w:r><w:t>{text}</w:t></w:r></w:p><w:tbl><w:tr>{}{}</w:tr><w:tr>{}{}</w:tr></w:tbl><w:sectPr/></w:body></w:document>"#,
        cell("Name"), cell("Role"), cell("Ann"), cell("Lead"));
    stored_zip(&[
        ("[Content_Types].xml", r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/></Types>"#.into()),
        ("_rels/.rels", rels(&[("rId1", "officeDocument", "word/document.xml")])),
        ("word/document.xml", doc),
        ("word/_rels/document.xml.rels", rels(&[("rId1", "styles", "styles.xml")])),
        ("word/styles.xml", format!(r#"<w:styles {W}><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/></w:style></w:styles>"#)),
    ])
}

/// The readable copy of `docx(heading, text)` called `name`, as Hersir writes it.
pub fn docx_md(name: &str, heading: &str, text: &str) -> String {
    format!("Text of {name}, made by Hersir for reading: layout and pictures are left out.\n\n# {heading}\n\n{text}\n\n| Name | Role |\n| --- | --- |\n| Ann | Lead |\n")
}

/// An .xlsx cut off halfway, as a broken download is.
pub fn damaged_xlsx() -> Vec<u8> {
    let whole = docx("x", "y");
    whole[..whole.len() / 2].to_vec()
}

/// An Office file protected with a password: an OLE compound file, not a zip.
pub fn locked_docx() -> Vec<u8> {
    let mut b = vec![0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
    b.resize(4096, 0);
    b
}
