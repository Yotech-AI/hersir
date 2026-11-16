//! GA-253: the update's new steps, in the core. A release's files from GitHub's answer; its signature, checked against a
//! fake release v9.9.9 (a local repository, its files at file:// addresses) signed with a throwaway key by
//! `ssh-keygen -Y sign`, never with Hersir's own keys; and its ready-made build: downloaded, checked against SHA256SUMS,
//! unpacked into the source, with Stop. Nothing here touches a real install.
// Linux and macOS only: these tests run ssh-keygen, tar and shell scripts as the release's programs.
#![cfg(unix)]
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use gizai_agents::release_sig;
use gizai_agents::update::{self as up, Log, Release, Stop};
use sha2::{Digest, Sha256};

const ARCHIVE: &str = "hersir-9.9.9-linux-x86_64.tar.gz";

fn git(dir: &Path, args: &[&str]) -> String {
    let o = Command::new("git").args(args).current_dir(dir).output().unwrap();
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8(o.stdout).unwrap().trim().to_string()
}

fn run(cmd: &mut Command) {
    let o = cmd.stdin(Stdio::null()).output().unwrap();
    assert!(o.status.success(), "{cmd:?}: {}", String::from_utf8_lossy(&o.stderr));
}

/// Writes an executable script through a child sh, so this process never holds it open for writing ("Text file busy").
fn write_script(path: &Path, script: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut sh = Command::new("sh").args(["-c", r#"cat > "$1" && chmod 755 "$1""#, "sh"]).arg(path).stdin(Stdio::piped()).spawn().unwrap();
    sh.stdin.take().unwrap().write_all(script.as_bytes()).unwrap();
    assert!(sh.wait().unwrap().success(), "can't write {}", path.display());
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// A fake release v9.9.9: the repository `repo` with its tag, its files in `files` (the archive, SHA256SUMS and
/// hersir-9.9.9.sig, signed with the throwaway key `key`), and `signers`, the allowed-signers file with that key.
struct Fake {
    _tmp: tempfile::TempDir,
    t: PathBuf,
    repo: PathBuf,
    files: PathBuf,
    key: PathBuf,
    signers: PathBuf,
}

impl Fake {
    fn new() -> Fake {
        let tmp = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
        let t = tmp.path().to_path_buf();
        let (repo, files, keys) = (t.join("repo"), t.join("files"), t.join("keys"));
        for d in [&repo, &files, &keys] {
            std::fs::create_dir_all(d).unwrap();
        }
        git(&repo, &["init", "-q", "-b", "production"]);
        std::fs::write(repo.join("Cargo.toml"), "[workspace.package]\nversion = \"9.9.9\"\n").unwrap();
        std::fs::write(repo.join("install.sh"), "echo the release's own installer\n").unwrap();
        git(&repo, &["add", "-A"]);
        git(&repo, &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "-m", "release 9.9.9"]);
        git(&repo, &["tag", "-a", "-m", "v9.9.9", "v9.9.9"]);
        let key = keygen(&keys, "throwaway");
        let signers = t.join("signers");
        std::fs::write(&signers, format!("hersir-release namespaces=\"hersir-release\" {}", std::fs::read_to_string(key.with_extension("pub")).unwrap())).unwrap();
        let fake = Fake { _tmp: tmp, t, repo, files, key, signers };
        fake.archive("9.9.9", &[]);
        fake.sums(None);
        fake.sign(&fake.key, &fake.commit());
        fake
    }

    /// The commit the tag v9.9.9 points to.
    fn commit(&self) -> String {
        git(&self.repo, &["rev-parse", "v9.9.9^{commit}"])
    }

    /// The ready-made build: target/release/hersir (which says `says`) and hersir-mcp, and `more` (path, contents).
    fn archive(&self, says: &str, more: &[(&str, &str)]) {
        let build = self.t.join("build");
        let _ = std::fs::remove_dir_all(&build);
        write_script(&build.join("target/release/hersir"), &format!("#!/bin/sh\n# the ready-made build\n[ \"$1\" = --version ] && echo 'hersir {says}'\nexit 0\n"));
        write_script(&build.join("target/release/hersir-mcp"), "#!/bin/sh\nexit 0\n");
        let mut paths = vec!["target/release/hersir".to_string(), "target/release/hersir-mcp".to_string()];
        for (path, text) in more {
            std::fs::create_dir_all(build.join(path).parent().unwrap()).unwrap();
            std::fs::write(build.join(path), text).unwrap();
            paths.push(path.to_string());
        }
        let out = self.files.join(ARCHIVE);
        let _ = std::fs::remove_file(&out);
        run(Command::new("tar").arg("-czf").arg(&out).arg("-C").arg(&build).args(&paths));
    }

    /// SHA256SUMS as the release workflow writes it, with `sum` for the archive instead of its own when given.
    fn sums(&self, sum: Option<&str>) {
        let own = sha256(&std::fs::read(self.files.join(ARCHIVE)).unwrap());
        let text = format!("{}  {ARCHIVE}\n{}  hersir-9.9.9-macos-arm64.tar.gz\n{}  hersir-9.9.9-windows-x86_64.zip\n",
                           sum.unwrap_or(&own), "a".repeat(64), "b".repeat(64));
        std::fs::write(self.files.join("SHA256SUMS"), text).unwrap();
    }

    /// hersir-9.9.9.sig by `key`, over the tag line with `commit` and SHA256SUMS as it is now.
    fn sign(&self, key: &Path, commit: &str) {
        let sums = std::fs::read(self.files.join("SHA256SUMS")).unwrap();
        sign(key, &release_sig::message("v9.9.9", commit, &sums), &self.files.join("hersir-9.9.9.sig"));
    }

    /// The release as the check finds it, with the files `names` attached.
    fn release(&self, names: &[&str]) -> Release {
        let assets: Vec<String> = names.iter().map(|n| {
            format!(r#"{{"name": "{n}", "state": "uploaded", "browser_download_url": "file://{}"}}"#, self.files.join(n).display())
        }).collect();
        up::parse_release(&format!(r#"{{"tag_name": "v9.9.9", "assets": [{}]}}"#, assets.join(","))).unwrap()
    }

    fn signed_release(&self) -> Release {
        self.release(&[ARCHIVE, "SHA256SUMS", "hersir-9.9.9.sig"])
    }

    fn signers(&self) -> Vec<release_sig::Signer> {
        release_sig::signers(Some(&self.signers)).unwrap()
    }

    fn dir(&self) -> PathBuf {
        self.t.join("data/update")
    }

    fn source(&self) -> PathBuf {
        self.dir().join("source")
    }

    fn log(&self) -> Log {
        Log::create(&self.dir().join("update.log")).unwrap()
    }

    fn said(&self) -> String {
        std::fs::read_to_string(self.dir().join("update.log")).unwrap()
    }

    /// Gets the release's source as the update does.
    fn get_source(&self, log: &Log) {
        up::get_source(&self.repo.display().to_string(), "v9.9.9", &self.source(), log, &Stop::default()).unwrap();
    }

    fn check(&self, release: &Release, log: &Log) -> Result<Vec<u8>, up::UpdateFailed> {
        up::check_signature(release, &self.source(), &self.dir(), &self.signers(), log, &Stop::default())
    }
}

/// A new ed25519 key without a passphrase at `<dir>/<name>` (and its .pub).
fn keygen(dir: &Path, name: &str) -> PathBuf {
    let key = dir.join(name);
    run(Command::new("ssh-keygen").args(["-q", "-t", "ed25519", "-N", "", "-C", name, "-f"]).arg(&key));
    key
}

/// `ssh-keygen -Y sign -n hersir-release` by `key` over `message`, as scripts/sign-release.sh signs, into `out`.
fn sign(key: &Path, message: &[u8], out: &Path) {
    let file = out.with_extension("message");
    let sig = out.with_extension("message.sig");
    std::fs::write(&file, message).unwrap();
    let _ = std::fs::remove_file(&sig);
    run(Command::new("ssh-keygen").args(["-q", "-Y", "sign", "-n", "hersir-release", "-f"]).arg(key).arg(&file));
    std::fs::rename(&sig, out).unwrap();
}

// ---------- a release's files ----------

#[test]
fn githubs_answer_lists_a_releases_uploaded_files_and_only_a_sig_makes_it_signed() {
    let json = r#"{"tag_name": "v9.9.9", "assets": [
        {"name": "hersir-9.9.9-linux-x86_64.tar.gz", "state": "uploaded", "browser_download_url": "https://example.test/a.tar.gz"},
        {"name": "SHA256SUMS", "browser_download_url": "https://example.test/SHA256SUMS"},
        {"name": "hersir-9.9.9.sig", "state": "starter", "browser_download_url": "https://example.test/sig"},
        {"name": "no-address.txt"}]}"#;
    let r = up::parse_release(json).unwrap();
    let names: Vec<&str> = r.assets.as_deref().unwrap().iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, ["hersir-9.9.9-linux-x86_64.tar.gz", "SHA256SUMS"], "a file still uploading, or without an address, isn't there yet");
    assert_eq!(r.asset("SHA256SUMS").map(|a| a.url.as_str()), Some("https://example.test/SHA256SUMS"));
    assert!(!r.signed(), "its .sig is still uploading");

    let r = up::parse_release(&json.replace("\"starter\"", "\"uploaded\"")).unwrap();
    assert!(r.signed());
    assert_eq!(up::signature_name("9.9.9"), "hersir-9.9.9.sig");
    // a .sig of another version doesn't sign this one
    let r = up::parse_release(r#"{"tag_name": "v9.9.9", "assets": [{"name": "hersir-9.9.8.sig", "browser_download_url": "file:///x"}]}"#).unwrap();
    assert!(!r.signed());
    assert!(!up::parse_release(r#"{"tag_name": "v9.9.9"}"#).unwrap().signed(), "no files at all");
}

#[test]
fn this_systems_build_has_the_name_the_release_workflow_gives_it() {
    let name = up::build_name("1.2.3");
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => assert_eq!(name.as_deref(), Some("hersir-1.2.3-linux-x86_64.tar.gz")),
        ("macos", "aarch64") => assert_eq!(name.as_deref(), Some("hersir-1.2.3-macos-arm64.tar.gz")),
        _ => assert_eq!(name, None, "no ready-made build for {}", up::system_name()),
    }
}

#[test]
fn sha256sums_gives_a_files_checksum_as_sha256sum_writes_it() {
    let (a, b) = ("a".repeat(64), "B".repeat(64));
    let sums = format!("{a}  hersir-1.tar.gz\n{b} *hersir-2.zip\n{a} hersir-3.zip\n{}  hersir-4.zip\n", "c".repeat(63));
    assert_eq!(up::checksum_for(&sums, "hersir-1.tar.gz"), Some(a.clone()));
    assert_eq!(up::checksum_for(&sums, "hersir-2.zip"), Some("b".repeat(64)), "binary mode, in lowercase");
    assert_eq!(up::checksum_for(&sums, "hersir-3.zip"), None, "one space isn't sha256sum's format");
    assert_eq!(up::checksum_for(&sums, "hersir-4.zip"), None, "not a sha256");
    assert_eq!(up::checksum_for(&sums, "hersir-1.tar"), None);
    assert_eq!(up::checksum_for(&sums, "hersir-5.zip"), None);
}

// ---------- the signature ----------

#[test]
fn a_releases_signature_checks_against_the_commit_of_its_tag_as_fetched() {
    let f = Fake::new();
    let log = f.log();
    f.get_source(&log);
    let sums = f.check(&f.signed_release(), &log).expect("a good signature");
    assert_eq!(sums, std::fs::read(f.files.join("SHA256SUMS")).unwrap(), "the SHA256SUMS the signature vouches for");
    // both went into <data folder>/update
    assert!(f.dir().join("hersir-9.9.9.sig").is_file() && f.dir().join("SHA256SUMS").is_file());
    let said = f.said();
    for line in ["$ curl --output", "hersir-9.9.9.sig", &format!("The tag v9.9.9 is commit {}", f.commit()),
                 "The signature checks: hersir-9.9.9.sig is by throwaway", "over v9.9.9 at"] {
        assert!(said.contains(line), "the log has {line:?}:\n{said}");
    }
}

#[test]
fn a_signature_by_another_key_is_refused() {
    let f = Fake::new();
    let other = keygen(&f.t.join("keys"), "other");
    f.sign(&other, &f.commit());
    let log = f.log();
    f.get_source(&log);
    let e = f.check(&f.signed_release(), &log).unwrap_err();
    assert!(e.what.starts_with("hersir-9.9.9.sig is signed with a key that isn't one of Hersir's release keys (SHA256:"), "{e:?}");
    assert!(e.what.ends_with("so Hersir doesn't install it"), "{e:?}");
    // and against the keys built into Hersir, the throwaway key is another key too
    f.sign(&f.key, &f.commit());
    let built_in = release_sig::signers(None).unwrap();
    let e = up::check_signature(&f.signed_release(), &f.source(), &f.dir(), &built_in, &log, &Stop::default()).unwrap_err();
    assert!(e.what.contains("isn't one of Hersir's release keys"), "{e:?}");
}

#[test]
fn a_changed_sha256sums_is_refused() {
    let f = Fake::new();
    f.sums(Some(&"0".repeat(64)));
    let log = f.log();
    f.get_source(&log);
    let e = f.check(&f.signed_release(), &log).unwrap_err();
    assert_eq!(e.what, format!("The signature of v9.9.9 doesn't check: the release's SHA256SUMS or the commit of its tag ({}) isn't what was signed, so Hersir doesn't install it",
                               &f.commit()[..12]));
}

#[test]
fn a_tag_moved_to_another_commit_is_refused() {
    let f = Fake::new();
    let signed = f.commit();
    std::fs::write(f.repo.join("install.sh"), "echo something else\n").unwrap();
    git(&f.repo, &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "-am", "not what was signed"]);
    git(&f.repo, &["tag", "-f", "-a", "-m", "v9.9.9", "v9.9.9"]);
    assert_ne!(f.commit(), signed);
    let log = f.log();
    f.get_source(&log);
    let e = f.check(&f.signed_release(), &log).unwrap_err();
    assert!(e.what.starts_with("The signature of v9.9.9 doesn't check") && e.what.contains(&f.commit()[..12]), "{e:?}");
}

#[test]
fn a_release_without_its_sig_or_sha256sums_is_refused_before_anything_downloads() {
    let f = Fake::new();
    let log = f.log();
    f.get_source(&log);
    let e = f.check(&f.release(&[ARCHIVE, "SHA256SUMS"]), &log).unwrap_err();
    assert_eq!(e.what, "v9.9.9 isn't signed: it has no hersir-9.9.9.sig, so Hersir doesn't install it");
    let e = f.check(&f.release(&[ARCHIVE, "hersir-9.9.9.sig"]), &log).unwrap_err();
    assert_eq!(e.what, "v9.9.9 has no SHA256SUMS, which its signature is over, so Hersir doesn't install it");
    assert!(!f.said().contains("$ curl"), "nothing was downloaded");
    // a .sig that is listed but can't be downloaded
    std::fs::remove_file(f.files.join("hersir-9.9.9.sig")).unwrap();
    let e = f.check(&f.signed_release(), &log).unwrap_err();
    assert!(e.what.starts_with("Couldn't download hersir-9.9.9.sig"), "{e:?}");
    assert!(!f.dir().join("hersir-9.9.9.sig").exists(), "nothing is left that passes for it");
}

#[test]
fn a_source_that_isnt_at_the_commit_of_the_tag_is_refused() {
    let f = Fake::new();
    let log = f.log();
    f.get_source(&log);
    assert_eq!(up::tag_commit(&f.source(), "v9.9.9").unwrap(), f.commit());
    git(&f.source(), &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "--allow-empty", "-m", "something else"]);
    let e = f.check(&f.signed_release(), &log).unwrap_err();
    assert_eq!(e.what, "The source isn't at the commit of the tag v9.9.9, so Hersir doesn't install it");
    assert!(up::tag_commit(&f.source(), "v1.0.0").unwrap_err().what.contains("Can't tell which commit the tag v1.0.0 points to"));
}

#[test]
fn an_earlier_updates_downloads_go_first() {
    let f = Fake::new();
    let dir = f.dir();
    std::fs::create_dir_all(dir.join("unpacked/target")).unwrap();
    for name in ["hersir-9.9.8.sig", "SHA256SUMS", "hersir-9.9.8-linux-x86_64.tar.gz", "hersir-1.zip"] {
        std::fs::write(dir.join(name), "old").unwrap();
    }
    std::fs::write(dir.join("update.log"), "old log").unwrap();
    std::fs::write(dir.join("notes.txt"), "mine").unwrap();
    up::clear_downloads(&dir);
    let mut left: Vec<String> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
    left.sort();
    assert_eq!(left, ["notes.txt", "update.log"]);
}

// ---------- the ready-made build ----------

fn get_build(f: &Fake, release: &Release, log: &Log) -> Result<(), up::UpdateFailed> {
    let sums = std::fs::read(f.files.join("SHA256SUMS")).unwrap();
    up::get_build(release.asset(ARCHIVE).unwrap(), "9.9.9", &sums, &f.source(), &f.dir(), log, &Stop::default())
}

#[test]
fn a_ready_made_build_is_downloaded_checked_and_unpacked_into_the_source() {
    let f = Fake::new();
    let log = f.log();
    f.get_source(&log);
    // an earlier build left its own programs there
    write_script(&f.source().join("target/release/hersir"), "#!/bin/sh\necho 'hersir 9.9.8'\n");
    std::fs::create_dir_all(f.source().join("target/release/deps")).unwrap();
    get_build(&f, &f.signed_release(), &log).unwrap();
    let hersir = std::fs::read_to_string(f.source().join("target/release/hersir")).unwrap();
    assert!(hersir.contains("# the ready-made build"), "{hersir}");
    assert!(f.source().join("target/release/hersir-mcp").is_file());
    assert!(f.source().join("target/release/deps").is_dir(), "what the archive doesn't have stays");
    assert!(!f.dir().join(ARCHIVE).exists() && !f.dir().join("unpacked").exists(), "the archive and the unpacked folder are gone");
    assert_eq!(std::fs::read_to_string(f.source().join("install.sh")).unwrap(), "echo the release's own installer\n");
    let said = f.said();
    let sum = sha256(&std::fs::read(f.files.join(ARCHIVE)).unwrap());
    for line in [format!("{ARCHIVE}: its sha256 is {sum}, as the release's SHA256SUMS says."), format!("Unpacked {ARCHIVE} into"),
                 "Its target/release/hersir --version says 9.9.9.".to_string()] {
        assert!(said.contains(&line), "the log has {line:?}:\n{said}");
    }
}

#[test]
fn a_wrong_checksum_leaves_the_source_as_it_was() {
    let f = Fake::new();
    let log = f.log();
    f.get_source(&log);
    write_script(&f.source().join("target/release/hersir"), "#!/bin/sh\necho 'hersir 9.9.8'\n");
    f.sums(Some(&"0".repeat(64)));
    let e = get_build(&f, &f.signed_release(), &log).unwrap_err();
    assert_eq!(e.what, format!("{ARCHIVE} doesn't match the release's SHA256SUMS"));
    assert!(f.said().contains(&format!("but the release's SHA256SUMS says {}", "0".repeat(64))));
    assert!(!std::fs::read_to_string(f.source().join("target/release/hersir")).unwrap().contains("ready-made"), "nothing was unpacked");
    // no line for the archive at all
    let sums = format!("{}  hersir-9.9.9-macos-arm64.tar.gz\n", "a".repeat(64));
    let e = up::get_build(f.signed_release().asset(ARCHIVE).unwrap(), "9.9.9", sums.as_bytes(), &f.source(), &f.dir(), &log, &Stop::default()).unwrap_err();
    assert_eq!(e.what, format!("The release's SHA256SUMS has no line for {ARCHIVE}"));
}

#[test]
fn an_archive_with_files_outside_target_release_is_refused_and_unpacks_nothing() {
    let f = Fake::new();
    let log = f.log();
    f.get_source(&log);
    // an archive that would replace the release's installer, which runs next
    f.archive("9.9.9", &[("install.sh", "echo not the release's installer\n")]);
    f.sums(None);
    let e = get_build(&f, &f.signed_release(), &log).unwrap_err();
    assert!(e.what.starts_with(&format!("Couldn't unpack {ARCHIVE}: install.sh isn't under target/release")), "{e:?}");
    assert_eq!(std::fs::read_to_string(f.source().join("install.sh")).unwrap(), "echo the release's own installer\n");
    assert!(!f.source().join("target/release/hersir").exists(), "nothing of it moved into the source");
    assert!(!f.dir().join("unpacked").exists());
}

#[test]
fn an_archive_with_a_link_out_of_its_folder_is_refused() {
    let f = Fake::new();
    let log = f.log();
    f.get_source(&log);
    let build = f.t.join("build");
    std::os::unix::fs::symlink("/etc/passwd", build.join("target/release/passwd")).unwrap();
    std::os::unix::fs::symlink("../../install.sh", build.join("target/release/up")).unwrap();
    for (link, to) in [("passwd", "/etc/passwd"), ("up", "../../install.sh")] {
        let out = f.files.join(ARCHIVE);
        let _ = std::fs::remove_file(&out);
        run(Command::new("tar").arg("-czf").arg(&out).arg("-C").arg(&build).args(["target/release/hersir", &format!("target/release/{link}")]));
        f.sums(None);
        let e = get_build(&f, &f.signed_release(), &log).unwrap_err();
        assert_eq!(e.what, format!("Couldn't unpack {ARCHIVE}: the link target/release/{link} leads out of its folder, to {to}"));
        assert!(!f.source().join("target/release").join(link).exists());
    }
}

#[test]
fn a_build_that_says_another_version_or_doesnt_run_is_refused() {
    let f = Fake::new();
    let log = f.log();
    f.get_source(&log);
    f.archive("9.9.8", &[]);
    f.sums(None);
    let e = get_build(&f, &f.signed_release(), &log).unwrap_err();
    assert_eq!(e.what, "The ready-made build says it is version 9.9.8, not 9.9.9");
    // a build whose hersir doesn't run here
    let build = f.t.join("build");
    write_script(&build.join("target/release/hersir"), "#!/bin/sh\necho 'cannot execute binary file' >&2\nexit 126\n");
    let out = f.files.join(ARCHIVE);
    std::fs::remove_file(&out).unwrap();
    run(Command::new("tar").arg("-czf").arg(&out).arg("-C").arg(&build).arg("target/release/hersir"));
    f.sums(None);
    let e = get_build(&f, &f.signed_release(), &log).unwrap_err();
    assert_eq!(e.what, "The ready-made build doesn't run on this system: it failed: cannot execute binary file");
}

#[test]
fn a_download_that_fails_says_why() {
    let f = Fake::new();
    let log = f.log();
    f.get_source(&log);
    std::fs::remove_file(f.files.join(ARCHIVE)).unwrap();
    let e = get_build(&f, &f.signed_release(), &log).unwrap_err();
    assert!(e.what.starts_with(&format!("Couldn't download {ARCHIVE}: ")), "{e:?}");
    assert!(!f.dir().join(ARCHIVE).exists());
    // a server that answers 404
    let (url, _server) = serve(Some("HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"));
    let to = f.dir().join("x.tar.gz");
    let e = up::download(&url, &to, &log, &Stop::default()).unwrap_err();
    assert!(e.what.starts_with("Couldn't download x.tar.gz: ") && e.what.contains("404"), "{e:?}");
    assert!(!to.exists());
}

/// A local HTTP server for one request. With `answer`, it sends that; without, it sends the start of a 100 MB file
/// and then nothing more, as a download that takes long.
fn serve(answer: Option<&'static str>) -> (String, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://127.0.0.1:{}/hersir.tar.gz", listener.local_addr().unwrap().port());
    let handle = std::thread::spawn(move || {
        let Ok((mut conn, _)) = listener.accept() else { return };
        let mut buf = [0u8; 4096];
        let _ = conn.read(&mut buf);
        match answer {
            Some(a) => {
                let _ = conn.write_all(a.as_bytes());
            }
            None => {
                let _ = conn.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 104857600\r\n\r\n");
                let _ = conn.write_all(&[0u8; 65536]);
                let _ = conn.flush();
                std::thread::sleep(Duration::from_secs(60));
            }
        }
    });
    (url, handle)
}

#[test]
fn stop_ends_a_download_soon_and_leaves_no_part_of_it() {
    let tmp = tempfile::tempdir().unwrap();
    let log = Log::create(&tmp.path().join("update.log")).unwrap();
    let to = tmp.path().join(ARCHIVE);
    let (url, _server) = serve(None);
    let stop = Stop::default();
    let stopper = {
        let (stop, to) = (stop.clone(), to.clone());
        std::thread::spawn(move || {
            let until = Instant::now() + Duration::from_secs(10);
            while std::fs::metadata(&to).map(|m| m.len()).unwrap_or(0) == 0 && Instant::now() < until {
                std::thread::sleep(Duration::from_millis(20));
            }
            stop.stop();
        })
    };
    let started = Instant::now();
    let e = up::download(&url, &to, &log, &stop).unwrap_err();
    stopper.join().unwrap();
    assert!(started.elapsed() < Duration::from_secs(15), "it stopped soon: {:?}", started.elapsed());
    assert!(stop.asked());
    assert!(e.what == "Stopped" || e.what.contains("was ended by a signal"), "{e:?}");
    assert!(!to.exists(), "the part it downloaded is removed");
    // after Stop, no other download starts
    let e = up::download(&url, &to, &log, &stop).unwrap_err();
    assert_eq!(e.what, "Stopped");
}

/// A .zip as the Windows build is packed, with `entries` (path, contents; a path ending in `@link` is a link to its
/// contents), into `out`.
fn zip(out: &Path, entries: &[(&str, &str)]) {
    use zip::write::SimpleFileOptions;
    let mut z = zip::ZipWriter::new(std::fs::File::create(out).unwrap());
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).unix_permissions(0o755);
    for (path, text) in entries {
        match path.strip_suffix("@link") {
            Some(link) => z.add_symlink(link, *text, options).unwrap(),
            None => {
                z.start_file(*path, options).unwrap();
                z.write_all(text.as_bytes()).unwrap();
            }
        }
    }
    z.finish().unwrap();
}

#[test]
fn a_zip_build_unpacks_the_same_way_and_nothing_outside_target_release() {
    let f = Fake::new();
    let log = f.log();
    f.get_source(&log);
    let name = "hersir-9.9.9-windows-x86_64.zip";
    let release = f.release(&[name]);
    let get = |entries: &[(&str, &str)]| {
        zip(&f.files.join(name), entries);
        let sums = format!("{}  {name}\n", sha256(&std::fs::read(f.files.join(name)).unwrap()));
        up::get_build(release.asset(name).unwrap(), "9.9.9", sums.as_bytes(), &f.source(), &f.dir(), &log, &Stop::default())
    };
    let hersir = "#!/bin/sh\n# the ready-made zip\n[ \"$1\" = --version ] && echo 'hersir 9.9.9'\nexit 0\n";
    let e = get(&[("target/release/hersir", hersir), ("../install.sh", "echo not the release's installer\n")]).unwrap_err();
    assert_eq!(e.what, format!("Couldn't unpack {name}: ../install.sh isn't under target/release"));
    let e = get(&[("target/release/hersir", hersir), ("install.sh", "echo not the release's installer\n")]).unwrap_err();
    assert_eq!(e.what, format!("Couldn't unpack {name}: install.sh isn't under target/release"));
    let e = get(&[("target/release/hersir", hersir), ("target/release/up@link", "../../install.sh")]).unwrap_err();
    assert_eq!(e.what, format!("Couldn't unpack {name}: target/release/up is a link"));
    assert_eq!(std::fs::read_to_string(f.source().join("install.sh")).unwrap(), "echo the release's own installer\n");
    assert!(!f.source().join("target").exists(), "nothing of them moved into the source");
    get(&[("target/release/hersir", hersir), ("target/release/hersir-mcp", "#!/bin/sh\nexit 0\n")]).unwrap();
    assert!(std::fs::read_to_string(f.source().join("target/release/hersir")).unwrap().contains("# the ready-made zip"));
    assert!(f.said().contains("Its target/release/hersir --version says 9.9.9."), "it runs: its mode came with it");
}
