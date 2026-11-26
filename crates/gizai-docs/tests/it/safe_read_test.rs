//! GA-362 QA: Docs reads a project's files the safe way, and a slow file doesn't stall the page.
//! - A tracked link to `/dev/zero`, to a FIFO or to a file outside the repository, as a route file, a declaration file, a
//!   guide, the changelog, a decision record, `.mcp.json` or `openapi.json`, and a FIFO in the folder itself: every
//!   reader is done at once, reads nothing through the link and leaks nothing from outside the repository.
//! - A plain file still reads, a file bigger than its limit is cut short (guides) or left out (Reference, Source).
//! - 156 KB of nested `[`: the file is cut off at its limit (`FILE_TIME`) and named as a problem (gone with the Reference
//!   switched off); the next view takes it from what the last one kept (`Kept`), and a file whose text changed is read
//!   again.
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use gizai_docs::index::{build, source, Input};
use gizai_docs::{reference, reference_kept, Kept, Reference, FILE_TIME, MAX_BYTES};

fn write(dir: &Path, path: &str, text: &str) -> String {
    let p = dir.join(path);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
    path.to_string()
}

/// `f` on a thread of its own, failing the test when it takes longer than `limit`: a read that waits on a FIFO, or never
/// ends at /dev/zero, would otherwise hang the test for good.
fn within<T: Send + 'static>(limit: Duration, what: &str, f: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(limit).unwrap_or_else(|_| panic!("{what} took longer than {limit:?}: it waited on a file or read without end"))
}

/// A FIFO at `path`, which nothing ever writes to: opening it for reading waits for a writer, for good.
#[cfg(unix)]
fn fifo(path: &Path) {
    use std::os::unix::ffi::OsStrExt;
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let c = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
    // SAFETY: a NUL-terminated path this test owns
    assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o644) }, 0, "mkfifo {}", path.display());
}

#[cfg(unix)]
fn link(target: &Path, at: &Path) {
    std::fs::create_dir_all(at.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(target, at).unwrap();
}

/// What came out of every reader the Docs page uses, as one text to search for leaks.
#[cfg(unix)]
fn everything(repo: &Path, files: &[String]) -> String {
    let routes = gizai_docs::api::api(repo, files);
    let reference = reference(repo, files);
    let content = gizai_docs::content::read(repo, files);
    let sources: Vec<_> = files.iter().filter_map(|f| source(repo, files, f)).collect();
    format!("{routes:?}\n{reference:?}\n{:?}\n{:?}\n{:?}\n{:?}\n{:?}\n{:?}\n{sources:?}", content.guides, content.changelog,
            content.decisions, content.mcp, content.api, content.problems)
}

#[cfg(unix)]
#[test]
fn links_to_dev_zero_to_a_fifo_or_outside_the_repository_read_nothing_and_never_wait() {
    let outside = tempfile::tempdir().unwrap();
    let o = outside.path();
    // what lies outside the repository: route lines, a declaration, a guide, a changelog, a decision, MCP servers, an API
    let secret_routes = write(o, "web.php", "<?php\nRoute::get('/outside-secret', fn () => 'x');\n");
    let secret_ts = write(o, "secret.ts", "export class OutsideSecret {}\nexport async function GET() {}\n");
    let secret_md = write(o, "guide.md", "# Outside secret guide\n\nOutside secret text.\n");
    let secret_log = write(o, "CHANGELOG.md", "# Changelog\n\n## [1.0.0] - 2026-01-01\n\n### Added\n- Outside secret change\n");
    let secret_adr = write(o, "0001-outside.md", "# Outside secret decision\n\nStatus: accepted\n");
    let secret_mcp = write(o, "mcp.json", r#"{"mcpServers":{"outside-secret":{"command":"x"}}}"#);
    let secret_api = write(o, "openapi.json", r#"{"openapi":"3.0.0","info":{"title":"Outside secret API","version":"1"},"paths":{"/outside-secret":{"get":{}}}}"#);
    fifo(&o.join("pipe"));

    let repo = tempfile::tempdir().unwrap();
    let d = repo.path();
    let mut files = vec![write(d, "src/fine.ts", "export function fine() {}\n")];
    let mut add = |path: &str, target: &Path| {
        link(target, &d.join(path));
        files.push(path.to_string());
    };
    add("routes/web.php", Path::new("/dev/zero"));
    add("routes/api.php", &o.join("pipe"));
    add("routes/channels.php", &o.join(&secret_routes));
    add("src/app/api/leak/route.ts", &o.join(&secret_ts));
    add("src/secret.ts", &o.join(&secret_ts));
    add("src/zero.ts", Path::new("/dev/zero"));
    add("src/pipe.ts", &o.join("pipe"));
    add("README.md", &o.join(&secret_md));
    add("docs/zero.md", Path::new("/dev/zero"));
    add("docs/pipe.md", &o.join("pipe"));
    add("CHANGELOG.md", &o.join(&secret_log));
    add("docs/decisions/0001-linked.md", &o.join(&secret_adr));
    add("docs/decisions/0002-zero.md", Path::new("/dev/zero"));
    add(".mcp.json", &o.join(&secret_mcp));
    add("openapi.json", &o.join(&secret_api));
    // a link to a link: still a link
    add("src/twice.ts", &d.join("src/secret.ts"));

    let (repo_path, all) = (d.to_path_buf(), files.clone());
    let started = Instant::now();
    let out = within(Duration::from_secs(20), "reading a repository of links", move || everything(&repo_path, &all));
    assert!(started.elapsed() < Duration::from_secs(10), "at once: {:?}", started.elapsed());
    assert!(!out.to_lowercase().contains("outside"), "nothing from outside the repository: {out}");
    assert!(!out.contains('\0') && !out.contains("\\0"), "nothing from /dev/zero (Debug writes a NUL as \\0)");
    // the repository's own plain file still reads
    let r = reference(d, &files);
    assert_eq!((r.files, r.read.as_slice()), (1, ["src/fine.ts".to_string()].as_slice()));
    assert!(r.cut_off.is_empty() && r.partial.is_empty(), "{r:?}");
}

#[cfg(unix)]
#[test]
fn a_fifo_in_the_folder_itself_is_no_plain_file_and_is_never_waited_on() {
    let repo = tempfile::tempdir().unwrap();
    let d = repo.path();
    let mut files = vec![write(d, "src/fine.ts", "export function fine() {}\n")];
    for path in ["routes/web.php", "src/pipe.ts", "README.md", "CHANGELOG.md", "docs/decisions/0001-pipe.md", ".mcp.json", "openapi.json"] {
        fifo(&d.join(path));
        files.push(path.to_string());
    }
    let (repo_path, all) = (d.to_path_buf(), files.clone());
    let out = within(Duration::from_secs(20), "reading a repository of FIFOs", move || everything(&repo_path, &all));
    assert!(out.contains("fine"), "{out}");
    assert!(source(d, &files, "src/pipe.ts").is_none());
}

#[test]
fn a_plain_file_still_reads_and_a_path_out_of_the_folder_is_refused() {
    let parent = tempfile::tempdir().unwrap();
    let d = parent.path().join("repo");
    write(parent.path(), "secret.ts", "export class OutsideSecret {}\n");
    let files = vec![write(&d, "src/a.ts", "export class A {}\n"), "../secret.ts".to_string(), "src/../../secret.ts".to_string()];
    let r = reference(&d, &files);
    assert_eq!(r.entries.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["A"], "{r:?}");
    let abs = parent.path().join("secret.ts").to_string_lossy().into_owned();
    let r = reference(&d, &[abs.clone()]);
    assert!(r.entries.is_empty(), "an absolute path isn't inside the folder: {r:?}");
    assert!(source(&d, &files, "src/a.ts").is_some());
    assert!(source(&d, &files, "../secret.ts").is_none());
    assert!(source(&d, &[abs.clone()], &abs).is_none());
}

#[test]
fn a_guide_over_its_limit_is_cut_short_and_a_declaration_file_over_it_is_left_out() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    let big = format!("# Big\n\n{}", "words ".repeat(MAX_BYTES as usize / 4));
    let files = vec![write(d, "README.md", &big), write(d, "src/big.ts", &format!("export class Big {{}}\n{}", "// pad\n".repeat(MAX_BYTES as usize / 6)))];
    let guides = gizai_docs::guides::guides(d, &files);
    assert_eq!(guides.len(), 1);
    assert_eq!(guides[0].body_md.len(), gizai_docs::guides::MAX_BYTES, "read at most its limit");
    assert!(reference(d, &files).entries.is_empty(), "a declaration file over the limit is left out, not cut short");
    assert!(source(d, &files, "src/big.ts").is_none());
}

/// 156 KB of nested `[`, which took tree-sitter 15 s in a release build before GA-362.
fn nested() -> String {
    format!("export const nested = {};\n", "[".repeat(156 * 1024))
}

fn timed<T>(f: impl FnOnce() -> T) -> (T, Duration) {
    let started = Instant::now();
    let out = f();
    (out, started.elapsed())
}

#[test]
fn a_file_written_to_slow_the_parser_is_cut_off_at_its_limit_and_the_next_view_takes_it_from_what_was_kept() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path().to_path_buf();
    let slow = write(&d, "src/slow.ts", &nested());
    let fine = write(&d, "src/fine.ts", "export function fine() {}\n");
    let files = vec![fine.clone(), slow.clone()];

    let (repo, all) = (d.clone(), files.clone());
    let ((first, mut kept), took) = within(Duration::from_secs(60), "the first view of a slow file", move || {
        let mut kept = Kept::default();
        let (r, took) = timed(|| reference_kept(&repo, &all, &mut kept));
        ((r, kept), took)
    });
    assert!(took < FILE_TIME * 8, "stopped at its limit, not after the whole parse: {took:?}");
    assert_eq!(first.cut_off, [slow.clone()], "named as cut off: {:?}", first.cut_off);
    assert!(first.partial.is_empty(), "{:?}", first.partial);
    assert_eq!(first.files, 2);
    assert!(first.entries.iter().any(|e| e.name == "fine"), "the other files are read as usual: {:?}", first.entries);
    assert_eq!(kept.len(), 2);

    // the next view, same texts: from what the first kept, without parsing the slow file again
    let (second, again) = timed(|| reference_kept(&d, &files, &mut kept));
    assert_eq!(second, first, "the same Reference");
    assert!(again < FILE_TIME, "from what was kept, not cut off again after {FILE_TIME:?}: {again:?} (first view {took:?})");

    // a file whose text changed is read again, committed or not; the slow one still comes from what was kept
    write(&d, "src/fine.ts", "export function fine() {}\nexport function finer() {}\n");
    let (third, quick) = timed(|| reference_kept(&d, &files, &mut kept));
    assert!(third.entries.iter().any(|e| e.name == "finer"), "the changed file read again: {:?}", third.entries);
    assert_eq!(third.cut_off, [slow.clone()]);
    assert!(quick < FILE_TIME, "{quick:?}");

    // a file no longer read isn't kept; one changed back to a fast text is read whole
    write(&d, "src/slow.ts", "export const nested = [[1]];\n");
    let fourth = reference_kept(&d, &files, &mut kept);
    assert!(fourth.cut_off.is_empty(), "{:?}", fourth.cut_off);
    assert!(fourth.entries.iter().any(|e| e.name == "nested"), "{:?}", fourth.entries);
    reference_kept(&d, std::slice::from_ref(&fine), &mut kept);
    assert_eq!(kept.len(), 1, "only this view's files");
}

#[test]
fn a_file_cut_off_is_named_on_home_and_left_out_with_the_reference_switched_off() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path().to_path_buf();
    let slow = write(&d, "src/slow.ts", &nested());
    let cut: Reference = within(Duration::from_secs(60), "a view of a slow file", move || reference(&d, &[slow]));
    let problems = |off: Vec<String>| {
        build(Input { reference: cut.clone(), off, ..Input::default() }).problems.into_iter()
            .map(|p| (p.file, p.message)).collect::<Vec<_>>()
    };
    assert_eq!(problems(vec![]), [("src/slow.ts".to_string(), "took too long to read, so its declarations may be incomplete".to_string())]);
    assert!(problems(vec!["reference".into()]).is_empty(), "the Reference's own problem goes with it");
    let index = build(Input { reference: cut, ..Input::default() });
    assert_eq!(index.stats.problems, 1);
}
