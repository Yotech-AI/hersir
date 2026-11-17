//! GA-281 QA: Hersir's log as a start makes it (`logs::start`), with stdout and stderr pointed into the file, an abrupt
//! end, the next start that notices it, quitting, and the panic hook. Each case runs in a child process of this test
//! program (this same program, run again with QA_LOGS_CHILD set): `start` points the process's fds 1 and 2 at the file
//! and sets the panic hook, and the case ends with `_exit`, as GTK ends Hersir. A program of its own (tests/it holds
//! tests that share one process). The release app in a headless cage is checked by hand (the card's QA comment).
#![cfg(unix)]
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use gizai_lib::logs;

const CHILD: &str = "QA_LOGS_CHILD";
const DATA: &str = "QA_LOGS_DATA";

/// Runs `case` in a child process on the data folder `data`.
fn child(case: &str, data: &Path) -> Output {
    Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "child_case", "--nocapture", "--test-threads=1"])
        .env(CHILD, case)
        .env(DATA, data)
        .env("RUST_BACKTRACE", "0")
        .output()
        .unwrap()
}

fn logs_in(data: &Path) -> Vec<PathBuf> {
    let mut logs: Vec<PathBuf> = fs::read_dir(logs::folder(data)).unwrap().flatten().map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "log")).collect();
    logs.sort();
    logs
}

fn text(out: &[u8]) -> String {
    String::from_utf8_lossy(out).into_owned()
}

/// The cases the children run. Without QA_LOGS_CHILD (the test program's own run) it does nothing.
#[test]
fn child_case() {
    let Some(case) = std::env::var_os(CHILD) else { return };
    let data = PathBuf::from(std::env::var_os(DATA).unwrap());
    logs::panic_hook();
    match case.to_str().unwrap() {
        // a release build: everything printed goes to the file, then an abrupt end as GTK's _exit(1)
        "redirect-then-exit" => {
            logs::start(&data, true).unwrap();
            eprintln!("QA eprintln line");
            println!("QA println line");
            let native = b"QA native fd 2 line\n";
            // SAFETY: a write of our own buffer to fd 2, as C code in GTK or WebKit does
            unsafe { libc::write(2, native.as_ptr().cast(), native.len()) };
            Command::new("sh").args(["-c", "echo QA child stdout; echo QA child stderr >&2"]).status().unwrap();
            // SAFETY: ends the process at once, flushing nothing
            unsafe { libc::_exit(1) };
        }
        // the next start: notices the abrupt end, then quits
        "start-then-quit" => {
            logs::start(&data, true).unwrap();
            eprintln!("QA second start");
            logs::quit();
            // a second quit (a start that failed after the log ran, then Exit) writes no second line
            logs::quit();
            std::process::exit(0);
        }
        // a test run or debug build: no redirect, the log's own lines on stderr too
        "no-redirect-then-quit" => {
            logs::start(&data, false).unwrap();
            eprintln!("QA stderr only");
            logs::quit();
            std::process::exit(0);
        }
        "panic" => {
            logs::start(&data, true).unwrap();
            panic!("QA panic on purpose");
        }
        other => panic!("unknown case {other}"),
    }
}

#[test]
fn a_release_start_prints_only_where_its_log_is_and_everything_after_goes_into_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let out = child("redirect-then-exit", &data);
    assert_eq!(out.status.code(), Some(1), "{out:?}");

    let logs = logs_in(&data);
    assert_eq!(logs.len(), 1, "{logs:?}");
    let log = &logs[0];
    let name = log.file_name().unwrap().to_str().unwrap();
    assert!(name.starts_with("hersir-") && name.ends_with("Z.log") && name.len() == "hersir-2026-11-13T14-50-26Z.log".len(), "{name}");

    // the terminal gets one line from Hersir, and none of what follows
    let stderr = text(&out.stderr);
    assert_eq!(stderr, format!("Hersir's log: {}\n", log.display()));
    let stdout = text(&out.stdout);
    assert!(!stdout.contains("QA"), "stdout kept: {stdout}");

    let file = fs::read_to_string(log).unwrap();
    let first = file.lines().next().unwrap();
    let running: logs::Running = serde_json::from_str(&fs::read_to_string(logs::folder(&data).join(logs::RUNNING)).unwrap()).unwrap();
    assert_eq!(first, format!("Hersir {} started {}, pid {}, {}", env!("CARGO_PKG_VERSION"), running.started_at, running.pid, logs::system()));
    assert_eq!(name, format!("hersir-{}.log", running.started_at.replace(':', "-")));
    assert_eq!(running.file, log.display().to_string());
    for want in ["QA eprintln line", "QA println line", "QA native fd 2 line", "QA child stdout", "QA child stderr"] {
        assert!(file.lines().any(|l| l == want), "{want:?} missing from the log:\n{file}");
    }
    assert!(!file.contains(" quit"), "an abrupt end writes no quit line:\n{file}");
    // running.json stays: the next start sees the abrupt end
    assert!(logs::folder(&data).join(logs::RUNNING).exists());
}

#[test]
fn the_next_start_names_the_abrupt_end_and_a_quit_leaves_none() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    assert_eq!(child("redirect-then-exit", &data).status.code(), Some(1));
    let first = logs_in(&data).remove(0);
    let first_started: logs::Running = serde_json::from_str(&fs::read_to_string(logs::folder(&data).join(logs::RUNNING)).unwrap()).unwrap();
    let last_written = logs::utc(fs::metadata(&first).unwrap().modified().unwrap()
        .duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64);
    // a second start in the same second gets -2: wait a second so the names sort as the starts did
    std::thread::sleep(std::time::Duration::from_millis(1100));

    let out = child("start-then-quit", &data);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let logs = logs_in(&data);
    assert_eq!(logs.len(), 2, "{logs:?}");
    let second = fs::read_to_string(&logs[1]).unwrap();
    let said = format!("The previous Hersir (pid {}, started {}) ended unexpectedly; its log is {}, last written {last_written}",
                       first_started.pid, first_started.started_at, first.display());
    let lines: Vec<&str> = second.lines().collect();
    assert_eq!(lines.get(1).copied(), Some(said.as_str()), "{second}");
    assert!(lines.contains(&"QA second start"), "{second}");
    // the last line says it quit, once
    let last = lines.last().unwrap();
    assert!(last.ends_with("Z quit") && last.len() == "2026-11-13T14:50:26Z quit".len(), "{second}");
    assert_eq!(lines.iter().filter(|l| l.ends_with("Z quit")).count(), 1, "{second}");
    assert!(!logs::folder(&data).join(logs::RUNNING).exists(), "quitting removes running.json");

    // get_logs_info's unexpectedEnd
    let end = logs::info(&data, None).unexpected_end.expect("kept");
    assert_eq!(end, logs::UnexpectedEnd { file: first.display().to_string(), started_at: first_started.started_at.clone(),
                                          last_written_at: last_written.clone() });

    // a start after a quit names no unexpected end; the earlier one is kept until it is dismissed
    std::thread::sleep(std::time::Duration::from_millis(1100));
    let out = child("no-redirect-then-quit", &data);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let third_path = logs_in(&data).pop().unwrap();
    let third = fs::read_to_string(&third_path).unwrap();
    assert!(!third.contains("ended unexpectedly"), "{third}");
    assert_eq!(logs::info(&data, None).unexpected_end, Some(end));

    // dismissed, and still gone after a further start
    logs::dismiss(&data).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(1100));
    assert_eq!(child("no-redirect-then-quit", &data).status.code(), Some(0));
    assert_eq!(logs::info(&data, None).unexpected_end, None);
}

#[test]
fn a_test_run_or_debug_build_keeps_printing_to_stderr_and_its_file_gets_the_logs_own_lines() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let out = child("no-redirect-then-quit", &data);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let log = logs_in(&data).remove(0);
    let file = fs::read_to_string(&log).unwrap();
    let stderr = text(&out.stderr);
    let first = file.lines().next().unwrap();
    assert!(first.starts_with(&format!("Hersir {} started ", env!("CARGO_PKG_VERSION"))), "{file}");
    // stderr: where the log is, the first line, Hersir's own line and the quit line
    assert!(stderr.contains(&format!("Hersir's log: {}\n", log.display())), "{stderr}");
    assert!(stderr.contains(&format!("{first}\n")), "{stderr}");
    assert!(stderr.contains("QA stderr only\n"), "{stderr}");
    assert!(stderr.lines().any(|l| l.ends_with("Z quit")), "{stderr}");
    // the file: the log's own lines, not what else was printed
    assert!(!file.contains("QA stderr only"), "{file}");
    assert!(file.lines().last().unwrap().ends_with("Z quit"), "{file}");
}

#[test]
fn a_panic_writes_its_message_place_and_backtrace_in_the_log_and_the_usual_hook_still_runs() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let out = child("panic", &data);
    assert_ne!(out.status.code(), Some(0), "{out:?}");
    let file = fs::read_to_string(logs_in(&data).remove(0)).unwrap();
    let at = file.lines().position(|l| l.contains(" panic: QA panic on purpose at ")).unwrap_or_else(|| panic!("no panic line:\n{file}"));
    let line = file.lines().nth(at).unwrap();
    // "<UTC time> panic: <message> at <file>:<line>"
    let time = &line[..20];
    assert!(time.as_bytes()[10] == b'T' && time.ends_with('Z') && line[20..].starts_with(" panic: "), "{line}");
    let place = line.rsplit_once(" at ").unwrap().1;
    let (path, number) = place.rsplit_once(':').unwrap();
    assert!(path.ends_with("logs_start_test.rs"), "{line}");
    assert!(number.parse::<u32>().is_ok(), "{line}");
    // a backtrace, forced even with RUST_BACKTRACE=0: numbered frames after the line, naming this test's case
    let after: Vec<&str> = file.lines().skip(at + 1).collect();
    assert!(after.iter().any(|l| l.trim_start().starts_with("0: ")), "no backtrace:\n{file}");
    assert!(after.iter().any(|l| l.contains("child_case")), "the backtrace doesn't reach the case:\n{file}");
    // then the usual hook's own words, in the log too (stderr points there)
    assert!(file.contains("panicked at") && file.contains("QA panic on purpose"), "{file}");
    // and nothing of it on the terminal
    assert_eq!(text(&out.stderr).lines().filter(|l| !l.starts_with("Hersir's log: ")).count(), 0, "{}", text(&out.stderr));
    let _ = std::io::stdout().flush();
}
