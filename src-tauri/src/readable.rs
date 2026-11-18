//! The Chief's readable copy of a file, beside its copy in the Chief's folder (`chat::lead_file_path`): for a Word,
//! Excel, PowerPoint, OpenDocument or RTF file its text as Markdown, `<name>.md` (`gizai_core::readable`, GA-310); for
//! a Pages, Keynote, Numbers, .doc, .dot, .ppt or .pps file a PDF that LibreOffice makes, `<name>.pdf`
//! (`gizai_agents::libreoffice`, GA-311). Each is made once, and made again only when it is missing.
use std::path::{Path, PathBuf};

use gizai_agents::libreoffice::{self, LibreOffice};

/// Whether a file called `name` gets a readable copy: a text copy (a Word, Excel, PowerPoint, OpenDocument or RTF file)
/// or a PDF copy (a Pages, Keynote, Numbers, .doc, .dot, .ppt or .pps file), by its extension (case ignored).
pub fn has_readable(name: &str) -> bool {
    gizai_core::readable::has_readable(name) || libreoffice::has_pdf(name)
}

/// Where the readable copy of the file called `name`, copied at `copy`, goes: beside it, `<name>.pdf` for a file that
/// gets a PDF copy, else `<name>.md`.
pub fn readable_path(name: &str, copy: &Path) -> PathBuf {
    if libreoffice::has_pdf(name) { libreoffice::pdf_path(copy) } else { gizai_core::readable::readable_path(copy) }
}

/// The readable copy of the file called `name`, copied at `copy`, made when it isn't there yet: its path, or why there
/// is none, in plain words on one line. A PDF copy is made by LibreOffice (`lo`), found on the PATH Hersir gives the
/// coding CLIs (`runs::command_path`). It blocks, a PDF copy for up to LibreOffice's limit: call it off the async
/// threads.
pub fn ensure(lo: &LibreOffice, name: &str, copy: &Path) -> Result<PathBuf, String> {
    if libreoffice::has_pdf(name) {
        lo.ensure_pdf(name, copy, crate::runs::command_path)
    } else {
        gizai_core::readable::ensure(copy)
    }
}
