//! GA-311 QA: PDF copies through LibreOffice (`gizai_agents::libreoffice`) for Pages, Keynote, Numbers, .doc, .dot, .ppt
//! and .pps files: which files get one, where it goes, finding LibreOffice, the profile URL, and `ensure_pdf` with a fake
//! soffice (found, not found, fails, too slow, leaves a child running, two at once) and, where it is installed, with the
//! real LibreOffice on a .doc and a .ppt it made itself.
#[path = "../support/fake_soffice.rs"]
mod fake_soffice;

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use fake_soffice::*;
use gizai_agents::libreoffice::{self, EXTENSIONS, LIMIT, LibreOffice, Program};

#[test]
fn the_seven_extensions_get_a_pdf_copy_with_case_ignored_and_no_other_file_does() {
    assert_eq!(EXTENSIONS, ["pages", "key", "numbers", "doc", "dot", "ppt", "pps"]);
    for (name, ext) in [("Plan.pages", ".pages"), ("Deck.KEY", ".key"), ("Budget 2026.Numbers", ".numbers"), ("old.doc", ".doc"),
                        ("letter.DOT", ".dot"), ("slides.Ppt", ".ppt"), ("show.PPS", ".pps"), ("v1.2 final.doc", ".doc")] {
        assert!(libreoffice::has_pdf(name), "{name}");
        assert_eq!(libreoffice::pdf_extension(name).as_deref(), Some(ext), "{name}");
    }
    for name in ["a.docx", "a.dotx", "a.pptx", "a.ppsx", "a.xlsx", "a.odt", "a.odp", "a.rtf", "a.pdf", "a.txt", "a.png", "doc",
                 "pages", "a.doc.txt", "a.keynote", "a.", "notes"] {
        assert!(!libreoffice::has_pdf(name), "{name}");
        assert_eq!(libreoffice::pdf_extension(name), None, "{name}");
    }
}

#[test]
fn the_pdf_goes_beside_the_copy_with_pdf_added_to_its_name_and_the_limit_is_120_s() {
    assert_eq!(libreoffice::pdf_path(Path::new("/d/lead/files/f1/Brief 2026.doc")), PathBuf::from("/d/lead/files/f1/Brief 2026.doc.pdf"));
    assert_eq!(libreoffice::pdf_path(Path::new("/d/lead/files/f2/Deck.KEY")), PathBuf::from("/d/lead/files/f2/Deck.KEY.pdf"));
    assert_eq!(LIMIT, Duration::from_secs(120));
    let lo = LibreOffice::new(PathBuf::from("/d/libreoffice"));
    assert_eq!((lo.limit, lo.program, lo.profile), (LIMIT, Program::Find, PathBuf::from("/d/libreoffice")));
}

#[test]
fn without_libreoffice_the_reason_is_word_for_word() {
    assert_eq!(libreoffice::not_installed(".pages"),
               "Hersir needs LibreOffice to read .pages files: install it from https://www.libreoffice.org, or save the file as PDF or .docx");
}

#[cfg(unix)]
#[test]
fn the_profile_folder_is_a_file_url_with_spaces_and_other_bytes_percent_encoded() {
    assert_eq!(libreoffice::file_url(Path::new("/home/you/.local/share/gizai/libreoffice")), "file:///home/you/.local/share/gizai/libreoffice");
    assert_eq!(libreoffice::file_url(Path::new("/home/you/My Data/#1 100%/caf\u{e9}~x")), "file:///home/you/My%20Data/%231%20100%25/caf%C3%A9~x");
    // a relative folder from the current one
    let here = std::env::current_dir().unwrap();
    assert_eq!(libreoffice::file_url(Path::new("lo")), libreoffice::file_url(&here.join("lo")));
}

#[cfg(unix)]
#[test]
fn find_takes_soffice_else_libreoffice_on_the_path_it_is_given() {
    let d = tempfile::tempdir().unwrap();
    let (a, b, none) = (d.path().join("a"), d.path().join("b"), d.path().join("none"));
    install_fake(&a.join("soffice"));
    install_fake(&a.join("libreoffice"));
    install_fake(&b.join("libreoffice"));
    std::fs::create_dir_all(&none).unwrap();
    // a file called soffice that can't be run isn't LibreOffice
    std::fs::write(none.join("soffice"), "not a program").unwrap();
    let path = |dirs: &[&Path]| std::env::join_paths(dirs).unwrap();
    assert_eq!(libreoffice::find(&path(&[&a])), Some(a.join("soffice")));
    assert_eq!(libreoffice::find(&path(&[&b])), Some(b.join("libreoffice")));
    assert_eq!(libreoffice::find(&path(&[&b, &a])), Some(a.join("soffice")), "soffice first, wherever it is on the PATH");
    assert_eq!(libreoffice::find(&path(&[&none, &b])), Some(b.join("libreoffice")));
    // Linux looks only on the PATH (macOS and Windows also look in LibreOffice's usual place)
    #[cfg(target_os = "linux")]
    assert_eq!(libreoffice::find(&path(&[&none])), None);
}

// ---- ensure_pdf with the fake soffice ----

/// A LibreOffice that runs the fake soffice, with its profile in `<d>/data/libreoffice`, and the folder `d`.
#[cfg(unix)]
fn fake_lo() -> (tempfile::TempDir, LibreOffice) {
    let d = tempfile::tempdir().unwrap();
    let fake = install_fake(&d.path().join("bin").join("soffice"));
    let lo = LibreOffice { profile: d.path().join("data").join("libreoffice"), limit: LIMIT, program: Program::At(fake) };
    (d, lo)
}

/// The Chief's copy of a file called `name` with `body`, in a folder of its own under `d`, as `chat::lead_copies` makes it.
fn copy(d: &Path, name: &str, body: &[u8]) -> PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let dir = d.join("data").join("lead").join("files").join(format!("f{}", N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(name), body).unwrap();
    dir.join(name)
}

/// The PATH for a LibreOffice whose program is set: never asked for.
fn no_path() -> OsString {
    panic!("PATH asked for while the program is set")
}

/// The names in the copy's folder.
fn folder(copy: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(copy.parent().unwrap()).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    v.sort();
    v
}

#[cfg(unix)]
#[test]
fn a_doc_gets_a_pdf_beside_its_copy_made_in_hersirs_own_profile_once_and_again_only_when_it_is_missing() {
    let (d, lo) = fake_lo();
    let c = copy(d.path(), "Brief 2026.doc", b"Brief for Kade\nsecond line");
    let pdf = lo.ensure_pdf("Brief 2026.doc", &c, no_path).unwrap();
    assert_eq!(pdf, c.with_file_name("Brief 2026.doc.pdf"));
    let body = std::fs::read(&pdf).unwrap();
    assert!(whole_pdf(&body) && String::from_utf8_lossy(&body).contains("% Brief for Kade"), "the PDF LibreOffice made of this file");
    assert_eq!(folder(&c), ["Brief 2026.doc", "Brief 2026.doc.pdf"], "only the copy and its PDF: no temporary folder is left");
    assert_eq!(std::fs::read(&c).unwrap(), b"Brief for Kade\nsecond line", "the copy itself is as it was");

    // what LibreOffice was given, word for word
    let calls = calls(&lo.profile);
    assert_eq!(calls.len(), 1, "{calls:?}");
    let a = &calls[0];
    let profile = format!("-env:UserInstallation=file://{}", lo.profile.display());
    assert_eq!(a[..5], ["--headless", "--norestore", profile.as_str(), "--convert-to", "pdf"]);
    assert_eq!(a[5], "--outdir");
    let out = Path::new(&a[6]);
    let out_name = out.file_name().unwrap().to_string_lossy();
    assert_eq!(out.parent(), c.parent(), "a new folder beside the copy: {}", out.display());
    assert!(out_name.starts_with(".pdf-") && out_name.ends_with(".part"), "{out_name}");
    assert!(!out.exists(), "removed once the PDF is in place");
    assert_eq!(Path::new(&a[7]), c);
    assert_eq!(a.len(), 9, "{a:?}");

    // a second call reuses it, as it is
    std::fs::write(&pdf, "marked").unwrap();
    assert_eq!(lo.ensure_pdf("Brief 2026.doc", &c, no_path).unwrap(), pdf);
    assert_eq!(std::fs::read_to_string(&pdf).unwrap(), "marked");
    assert_eq!(fake_soffice::calls(&lo.profile).len(), 1, "LibreOffice isn't run again");
    // also once LibreOffice is gone
    let gone = LibreOffice { program: Program::Missing, ..lo.clone() };
    assert_eq!(gone.ensure_pdf("Brief 2026.doc", &c, no_path).unwrap(), pdf);

    // made again when it is missing
    std::fs::remove_file(&pdf).unwrap();
    assert_eq!(lo.ensure_pdf("Brief 2026.doc", &c, no_path).unwrap(), pdf);
    assert!(whole_pdf(&std::fs::read(&pdf).unwrap()));
    assert_eq!(fake_soffice::calls(&lo.profile).len(), 2);
}

#[cfg(unix)]
#[test]
fn pages_key_numbers_dot_ppt_and_pps_take_the_same_path_with_case_ignored() {
    let (d, lo) = fake_lo();
    for name in ["Plan.pages", "Deck.KEY", "Budget.numbers", "Letter.dot", "Slides.PPT", "Show.pps", "Old.Doc"] {
        let c = copy(d.path(), name, format!("body of {name}").as_bytes());
        let pdf = lo.ensure_pdf(name, &c, no_path).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(pdf, c.with_file_name(format!("{name}.pdf")));
        assert!(String::from_utf8_lossy(&std::fs::read(&pdf).unwrap()).contains(&format!("% body of {name}")), "{name}");
        assert_eq!(folder(&c), [name.to_string(), format!("{name}.pdf")]);
    }
    assert_eq!(fake_soffice::calls(&lo.profile).len(), 7);
}

#[test]
fn without_libreoffice_each_file_says_hersir_needs_it_with_its_own_extension_and_nothing_is_made() {
    let d = tempfile::tempdir().unwrap();
    let lo = LibreOffice { program: Program::Missing, ..LibreOffice::new(d.path().join("data").join("libreoffice")) };
    for (name, ext) in [("Plan.pages", ".pages"), ("Deck.KEY", ".key"), ("Budget.Numbers", ".numbers"), ("old.doc", ".doc"), ("slides.ppt", ".ppt")] {
        let c = copy(d.path(), name, b"a file");
        let why = lo.ensure_pdf(name, &c, no_path).unwrap_err();
        assert_eq!(why, format!("Hersir needs LibreOffice to read {ext} files: install it from https://www.libreoffice.org, or save the file as PDF or .docx"));
        assert_eq!(folder(&c), [name], "the copy alone");
    }
    assert!(!lo.profile.exists(), "no profile folder without LibreOffice");
}

#[cfg(target_os = "linux")]
#[test]
fn find_looks_on_the_path_hersir_gives_when_a_pdf_is_made_and_libreoffice_runs_with_that_path() {
    let d = tempfile::tempdir().unwrap();
    let (bin, empty) = (d.path().join("bin"), d.path().join("empty"));
    // soffice: a real LibreOffice in /usr/bin (on the PATH below) would come before a fake called libreoffice
    install_fake(&bin.join("soffice"));
    std::fs::create_dir_all(&empty).unwrap();
    let lo = LibreOffice::new(d.path().join("data").join("libreoffice"));
    assert_eq!(lo.program, Program::Find);

    // not on that PATH: not installed
    let c = copy(d.path(), "Plan.numbers", b"numbers");
    assert_eq!(lo.ensure_pdf("Plan.numbers", &c, || empty.clone().into_os_string()).unwrap_err(), libreoffice::not_installed(".numbers"));

    // on it: found, and run with it (the fake needs sh's tools, so the system's folders follow)
    let path = std::env::join_paths([bin.clone(), PathBuf::from("/usr/bin"), PathBuf::from("/bin")]).unwrap();
    let asked = std::cell::Cell::new(0);
    let pdf = lo.ensure_pdf("Plan.numbers", &c, || { asked.set(asked.get() + 1); path.clone() }).unwrap();
    assert_eq!(pdf, c.with_file_name("Plan.numbers.pdf"));
    assert_eq!(asked.get(), 1);
    let calls = calls(&lo.profile);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].last().unwrap(), &format!("PATH={}", path.to_string_lossy()));

    // a PDF that is there: the PATH isn't even asked for
    let pdf2 = lo.ensure_pdf("Plan.numbers", &c, || -> OsString { panic!("PATH asked for a PDF that is there") }).unwrap();
    assert_eq!(pdf2, pdf);
}

#[cfg(unix)]
#[test]
fn a_conversion_that_fails_says_what_libreoffice_said_and_leaves_no_pdf_and_no_temporary_folder() {
    let (d, lo) = fake_lo();
    let c = copy(d.path(), "broken.ppt", b"FAKE_FAIL");
    assert_eq!(lo.ensure_pdf("broken.ppt", &c, no_path).unwrap_err(), "LibreOffice couldn't make a PDF of it (Error: source file could not be loaded)");
    assert_eq!(folder(&c), ["broken.ppt"]);

    let c = copy(d.path(), "nothing.doc", b"FAKE_NOPDF");
    assert_eq!(lo.ensure_pdf("nothing.doc", &c, no_path).unwrap_err(), "LibreOffice ended without making a PDF of it");
    assert_eq!(folder(&c), ["nothing.doc"]);

    let c = copy(d.path(), "half.key", b"FAKE_HALF");
    let why = lo.ensure_pdf("half.key", &c, no_path).unwrap_err();
    assert!(why.starts_with("LibreOffice couldn't make a PDF of it ("), "{why}");
    assert_eq!(folder(&c), ["half.key"], "never half a PDF");

    // an empty file isn't sent to LibreOffice
    let c = copy(d.path(), "empty.pages", b"");
    assert_eq!(lo.ensure_pdf("empty.pages", &c, no_path).unwrap_err(), "the file is empty");
    assert_eq!(fake_soffice::calls(&lo.profile).len(), 3);
    // and a file of another kind gets no PDF
    let c = copy(d.path(), "notes.txt", b"text");
    assert!(lo.ensure_pdf("notes.txt", &c, no_path).is_err());
    assert_eq!(folder(&c), ["notes.txt"]);
}

#[cfg(unix)]
#[test]
fn a_conversion_over_its_limit_is_stopped_with_everything_it_started_and_says_so() {
    let (d, mut lo) = fake_lo();
    lo.limit = Duration::from_secs(1);
    let c = copy(d.path(), "slow.doc", b"FAKE_SLOW");
    let started = Instant::now();
    let why = lo.ensure_pdf("slow.doc", &c, no_path).unwrap_err();
    assert_eq!(why, "LibreOffice took more than 1 s to make its PDF, so Hersir stopped it");
    assert!(started.elapsed() < Duration::from_secs(10), "stopped at its limit, not after the fake's 300 s: {:?}", started.elapsed());
    let (parent, child) = (pid_in(&lo.profile.join("parent")), pid_in(&lo.profile.join("child")));
    assert!(wait_for(Duration::from_secs(5), || !runs(parent) && !runs(child)), "soffice ({parent}) and what it started ({child}) are ended");
    assert_eq!(folder(&c), ["slow.doc"], "no PDF and no temporary folder");

    // the limit is said as it is set
    lo.limit = Duration::from_millis(1500);
    let c = copy(d.path(), "slow2.doc", b"FAKE_SLOW");
    assert_eq!(lo.ensure_pdf("slow2.doc", &c, no_path).unwrap_err(), "LibreOffice took more than 1.5 s to make its PDF, so Hersir stopped it");
    let child = pid_in(&lo.profile.join("child"));
    assert!(wait_for(Duration::from_secs(5), || !runs(child)));
}

#[cfg(unix)]
#[test]
fn what_libreoffice_started_and_left_running_after_it_made_its_pdf_is_ended_too() {
    let (d, lo) = fake_lo();
    let c = copy(d.path(), "leaves.ppt", b"FAKE_LEAVE");
    let pdf = lo.ensure_pdf("leaves.ppt", &c, no_path).unwrap();
    assert!(whole_pdf(&std::fs::read(&pdf).unwrap()));
    let child = pid_in(&lo.profile.join("child"));
    assert!(wait_for(Duration::from_secs(5), || !runs(child)), "the child it left ({child}) is ended");
}

#[cfg(unix)]
#[test]
fn two_files_at_once_both_get_their_pdf_one_conversion_at_a_time() {
    let (d, lo) = fake_lo();
    let (a, b) = (copy(d.path(), "one.doc", b"FAKE_WAIT one"), copy(d.path(), "two.ppt", b"FAKE_WAIT two"));
    let got = std::thread::scope(|s| {
        let ha = s.spawn(|| lo.ensure_pdf("one.doc", &a, no_path));
        let hb = s.spawn(|| lo.ensure_pdf("two.ppt", &b, no_path));
        (ha.join().unwrap(), hb.join().unwrap())
    });
    assert_eq!(got.0.unwrap(), a.with_file_name("one.doc.pdf"));
    assert_eq!(got.1.unwrap(), b.with_file_name("two.ppt.pdf"));
    assert_eq!(fake_soffice::calls(&lo.profile).len(), 2);
    assert!(!lo.profile.join("overlap").exists(), "a second LibreOffice ran while the first one worked");

    // the same file twice at once: one PDF, made once
    let c = copy(d.path(), "same.pages", b"FAKE_WAIT same");
    let got = std::thread::scope(|s| {
        let h1 = s.spawn(|| lo.ensure_pdf("same.pages", &c, no_path));
        let h2 = s.spawn(|| lo.ensure_pdf("same.pages", &c, no_path));
        [h1.join().unwrap(), h2.join().unwrap()]
    });
    for r in got {
        assert_eq!(r.unwrap(), c.with_file_name("same.pages.pdf"));
    }
    assert_eq!(fake_soffice::calls(&lo.profile).len(), 3, "made once");
}

// ---- the real LibreOffice, where it is installed ----

/// LibreOffice as Hersir runs it (found on this computer's PATH), with its profile in `d`.
fn real_lo(d: &Path) -> LibreOffice {
    LibreOffice::new(d.join("data").join("libreoffice"))
}

fn env_path() -> OsString {
    std::env::var_os("PATH").unwrap_or_default()
}

#[test]
fn with_libreoffice_a_doc_and_a_ppt_it_made_get_a_pdf_of_all_their_pages_and_nothing_is_left_running() {
    let Some(soffice) = real() else { return skipped("real doc and ppt") };
    let (doc, ppt) = old_doc_and_ppt(&soffice);
    let d = tempfile::tempdir().unwrap();
    let lo = real_lo(d.path());
    let (cd, cp) = (copy(d.path(), "Brief.doc", doc), copy(d.path(), "Deck.PPT", ppt));
    let pd = lo.ensure_pdf("Brief.doc", &cd, env_path).unwrap();
    let pp = lo.ensure_pdf("Deck.PPT", &cp, env_path).unwrap();
    assert_eq!((pd.clone(), pp.clone()), (cd.with_file_name("Brief.doc.pdf"), cp.with_file_name("Deck.PPT.pdf")));
    for (pdf, n) in [(&pd, 2), (&pp, 2)] {
        let b = std::fs::read(pdf).unwrap();
        assert!(whole_pdf(&b), "{}", pdf.display());
        assert_eq!(pages(&b), n, "every page of {}", pdf.display());
    }
    assert_eq!(folder(&cd), ["Brief.doc", "Brief.doc.pdf"]);
    assert_eq!(folder(&cp), ["Deck.PPT", "Deck.PPT.pdf"]);
    assert!(lo.profile.join("user").is_dir(), "LibreOffice made Hersir's own profile");
    #[cfg(target_os = "linux")]
    {
        let profile = lo.profile.to_string_lossy().into_owned();
        assert!(wait_for(Duration::from_secs(5), || running_with(&profile).is_empty()), "left running: {:?}", running_with(&profile));
    }
}

#[cfg(target_os = "linux")]
#[test]
fn with_libreoffice_a_conversion_over_its_limit_leaves_nothing_running_and_the_next_one_still_works() {
    let Some(soffice) = real() else { return skipped("real limit") };
    let (doc, _) = old_doc_and_ppt(&soffice);
    let d = tempfile::tempdir().unwrap();
    let mut lo = real_lo(d.path());
    let profile = lo.profile.to_string_lossy().into_owned();
    lo.limit = Duration::from_millis(300);
    let c = copy(d.path(), "Brief.doc", doc);
    assert_eq!(lo.ensure_pdf("Brief.doc", &c, env_path).unwrap_err(), "LibreOffice took more than 0.3 s to make its PDF, so Hersir stopped it");
    assert!(wait_for(Duration::from_secs(5), || running_with(&profile).is_empty()), "left running: {:?}", running_with(&profile));
    assert_eq!(folder(&c), ["Brief.doc"]);

    // the profile it was stopped in still works
    lo.limit = LIMIT;
    let pdf = lo.ensure_pdf("Brief.doc", &c, env_path).unwrap();
    assert_eq!(pages(&std::fs::read(&pdf).unwrap()), 2);
    assert!(wait_for(Duration::from_secs(5), || running_with(&profile).is_empty()), "left running: {:?}", running_with(&profile));
}

#[cfg(target_os = "linux")]
#[test]
fn with_another_libreoffice_open_the_conversion_still_works_and_that_one_keeps_running() {
    let Some(soffice) = real() else { return skipped("real open LibreOffice") };
    let (_, ppt) = old_doc_and_ppt(&soffice);
    let d = tempfile::tempdir().unwrap();
    // "your" LibreOffice: one that stays open on a profile of its own (headless here: no window on this screen)
    let yours = d.path().join("your-profile");
    let mut cmd = gizai_agents::os::command(&soffice);
    cmd.args(["--headless", "--norestore", "--nologo"]).arg(format!("-env:UserInstallation={}", libreoffice::file_url(&yours)))
        .arg(format!("--accept=pipe,name=ga311qa{};urp;", std::process::id()))
        .stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    let (mut child, tree) = gizai_agents::os::spawn_tree(&mut cmd, false).unwrap();
    let yours_s = yours.to_string_lossy().into_owned();
    assert!(wait_for(Duration::from_secs(30), || yours.join("user").is_dir() && !running_with(&yours_s).is_empty()), "your LibreOffice started");
    std::thread::sleep(Duration::from_secs(2));
    let before = { let mut v = running_with(&yours_s); v.sort(); v };

    let lo = real_lo(d.path());
    let c = copy(d.path(), "Deck.ppt", ppt);
    let pdf = lo.ensure_pdf("Deck.ppt", &c, env_path);
    let after = { let mut v = running_with(&yours_s); v.sort(); v };
    tree.end(gizai_agents::os::End::Kill);
    let _ = child.wait();

    let pdf = pdf.unwrap();
    assert_eq!(pages(&std::fs::read(&pdf).unwrap()), 2, "made by Hersir's own LibreOffice");
    assert!(!before.is_empty());
    assert_eq!(after, before, "your LibreOffice runs on as it was");
    assert!(!yours.join("Deck.pdf").exists() && !d.path().join("Deck.pdf").exists(), "your LibreOffice didn't take the job");
    let profile = lo.profile.to_string_lossy().into_owned();
    assert!(wait_for(Duration::from_secs(5), || running_with(&profile).is_empty()));
}
