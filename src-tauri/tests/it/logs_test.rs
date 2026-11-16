//! GA-281 QA: Hersir's own log, its parts that need no running app (src-tauri/src/logs.rs). The file's name and first
//! line, the alive and panic lines, the unexpected end that `running.json` leaves for the next start (kept in
//! `unexpected-end.json` until it is dismissed, also after a restart), `get_logs_info`'s JSON, the folder
//! `open_logs_folder` opens, the cleanup that keeps the 20 newest logs, Hersir's own memory from /proc, and the self-test
//! panic's refusal outside a self-test run. The start itself (stdout and stderr into the file, an abrupt end, the panic
//! hook) is in tests/logs_start_test.rs, a program of its own: it changes process-wide state.
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use gizai_lib::logs::{
    self, alive_line, dismiss, file_name, first_line, folder, info, mark_running, os_release_name, own_pages, panic_command,
    panic_text, parse_stat, previous_end, remove_old, utc, Proc, Running, UnexpectedEnd,
};

/// 2026-11-13T14:50:26Z, the card's example.
const STARTED: i64 = 1_794_581_426_000;

fn touch(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn set_mtime(path: &Path, ms: i64) {
    let t = SystemTime::UNIX_EPOCH + Duration::from_millis(ms as u64);
    fs::File::options().write(true).open(path).unwrap().set_modified(t).unwrap();
}

#[test]
fn times_are_iso_8601_utc_to_the_second() {
    assert_eq!(utc(0), "1970-01-01T00:00:00Z");
    assert_eq!(utc(STARTED), "2026-11-13T14:50:26Z");
    assert_eq!(utc(STARTED + 999), "2026-11-13T14:50:26Z");
    // a leap day, a second before midnight
    assert_eq!(utc(1_709_251_199_999), "2024-02-29T23:59:59Z");
}

#[test]
fn a_starts_log_is_named_after_its_utc_start_with_no_colon_for_windows() {
    assert_eq!(file_name(STARTED), "hersir-2026-11-13T14-50-26Z.log");
    assert!(!file_name(STARTED).contains(':'));
}

#[test]
fn the_first_line_names_the_version_the_utc_start_the_pid_and_the_system() {
    assert_eq!(first_line("0.17.0", STARTED, 4242, "Linux 6.17.8 x86_64 (Arch Linux; wayland, Hyprland; WebKitGTK 2.50.1)"),
               "Hersir 0.17.0 started 2026-11-13T14:50:26Z, pid 4242, Linux 6.17.8 x86_64 (Arch Linux; wayland, Hyprland; WebKitGTK 2.50.1)");
}

#[cfg(target_os = "linux")]
#[test]
fn on_linux_the_system_names_linux_the_machine_and_webkitgtk() {
    let system = logs::system();
    assert!(system.starts_with("Linux "), "{system}");
    assert!(system.contains(std::env::consts::ARCH), "{system}");
    assert!(system.contains("WebKitGTK 2."), "{system}");
}

#[test]
fn os_release_gives_its_pretty_name_else_its_name_without_quotes() {
    assert_eq!(os_release_name("NAME=\"Arch Linux\"\nPRETTY_NAME=\"Arch Linux\"\nID=arch\n").as_deref(), Some("Arch Linux"));
    assert_eq!(os_release_name("NAME='Fedora Linux'\nVERSION_ID=41\n").as_deref(), Some("Fedora Linux"));
    assert_eq!(os_release_name("PRETTY_NAME=Ubuntu 24.04.1 LTS\nNAME=Ubuntu").as_deref(), Some("Ubuntu 24.04.1 LTS"));
    // a key that only starts like PRETTY_NAME isn't it
    assert_eq!(os_release_name("PRETTY_NAMES=x\nNAME=Debian").as_deref(), Some("Debian"));
    assert_eq!(os_release_name("PRETTY_NAME=\"\"\n"), None);
    assert_eq!(os_release_name("ID=arch\n"), None);
}

#[test]
fn the_alive_line_has_the_utc_time_the_runs_at_work_hersirs_memory_and_the_free_memory() {
    let gb = 1u64 << 30;
    let now = STARTED + 60_000;
    assert_eq!(alive_line(now, 2, Some(gb * 14 / 10), Some(gb * 217 / 10)),
               "2026-11-13T14:51:26Z alive: 2 runs at work, Hersir 1.4 GB, free 21.7 GB");
    assert_eq!(alive_line(now, 1, Some(gb / 2), Some(gb * 3)), "2026-11-13T14:51:26Z alive: 1 run at work, Hersir 0.5 GB, free 3.0 GB");
    assert_eq!(alive_line(now, 0, Some(gb), Some(gb)), "2026-11-13T14:51:26Z alive: 0 runs at work, Hersir 1.0 GB, free 1.0 GB");
}

#[test]
fn what_the_system_doesnt_say_is_left_out_of_the_alive_line() {
    let gb = 1u64 << 30;
    assert_eq!(alive_line(STARTED, 3, None, Some(gb * 8)), "2026-11-13T14:50:26Z alive: 3 runs at work, free 8.0 GB");
    assert_eq!(alive_line(STARTED, 3, Some(gb * 2), None), "2026-11-13T14:50:26Z alive: 3 runs at work, Hersir 2.0 GB");
    assert_eq!(alive_line(STARTED, 0, None, None), "2026-11-13T14:50:26Z alive: 0 runs at work");
}

#[test]
fn a_panic_line_has_the_time_its_message_its_place_and_the_backtrace_after_it() {
    let text = panic_text(STARTED, "boom", "src/runs.rs:42", "   0: std::backtrace\n   1: gizai_lib::runs::x\n\n");
    let mut lines = text.lines();
    assert_eq!(lines.next(), Some("2026-11-13T14:50:26Z panic: boom at src/runs.rs:42"));
    assert_eq!(lines.next(), Some("   0: std::backtrace"));
    assert_eq!(lines.next(), Some("   1: gizai_lib::runs::x"));
    assert_eq!(lines.next(), None, "the backtrace's trailing blank lines go: {text:?}");
}

#[test]
fn open_logs_folder_opens_exactly_the_logs_folder_in_the_data_folder() {
    let data = Path::new("/home/you/.local/share/hersir");
    assert_eq!(folder(data), PathBuf::from("/home/you/.local/share/hersir/logs"));
    assert_eq!(logs::FOLDER, "logs");
    // and get_logs_info names the same folder, as the system writes it (`\` before logs on Windows)
    let named = info(data, None).folder;
    assert_eq!(PathBuf::from(&named), folder(data));
    assert_eq!(named, folder(data).display().to_string());
}

#[test]
fn get_logs_info_has_the_shape_the_page_reads() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    let current = folder(data).join(file_name(STARTED));
    let none = serde_json::to_value(info(data, Some(&current))).unwrap();
    assert_eq!(none, serde_json::json!({
        "folder": folder(data).display().to_string(),
        "current": current.display().to_string(),
        "unexpectedEnd": null,
    }));
    // a Hersir that keeps no log this time says "" as its current one
    assert_eq!(info(data, None).current, "");

    let end = UnexpectedEnd { file: "/x/logs/hersir-a.log".into(), started_at: "2026-11-13T14:50:26Z".into(),
                              last_written_at: "2026-11-13T15:03:01Z".into() };
    touch(&folder(data).join(logs::UNEXPECTED_END), &serde_json::to_string(&end).unwrap());
    let some = serde_json::to_value(info(data, Some(&current))).unwrap();
    assert_eq!(some["unexpectedEnd"], serde_json::json!({
        "file": "/x/logs/hersir-a.log", "startedAt": "2026-11-13T14:50:26Z", "lastWrittenAt": "2026-11-13T15:03:01Z",
    }));
    let keys: Vec<&String> = some.as_object().unwrap().keys().collect();
    assert_eq!(keys.len(), 3, "{some}");
}

#[test]
fn without_running_json_the_previous_hersir_quit_or_none_ran_here() {
    let dir = tempfile::tempdir().unwrap();
    assert!(previous_end(dir.path()).is_none());
    assert!(info(dir.path(), None).unexpected_end.is_none());
    assert!(!folder(dir.path()).join(logs::UNEXPECTED_END).exists());
}

#[test]
fn a_running_json_left_behind_is_an_unexpected_end_with_its_log_and_when_that_was_last_written() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    let log = folder(data).join(file_name(STARTED));
    touch(&log, "Hersir 0.17.0 started 2026-11-13T14:50:26Z, pid 4242, Linux\n");
    let last = STARTED + 13 * 60_000 + 7_000;
    set_mtime(&log, last);
    mark_running(data, &Running { pid: 4242, started_at: utc(STARTED), file: log.display().to_string() }).unwrap();
    assert!(folder(data).join(logs::RUNNING).exists());

    let (end, said) = previous_end(data).expect("an unexpected end");
    assert_eq!(end, UnexpectedEnd { file: log.display().to_string(), started_at: "2026-11-13T14:50:26Z".into(),
                                    last_written_at: "2026-11-13T15:03:33Z".into() });
    assert_eq!(said, format!("The previous Hersir (pid 4242, started 2026-11-13T14:50:26Z) ended unexpectedly; its log is {}, \
                              last written 2026-11-13T15:03:33Z", log.display()));
    // kept for the page: get_logs_info returns it
    assert_eq!(info(data, None).unexpected_end, Some(end));
}

#[test]
fn the_unexpected_end_stays_until_it_is_dismissed_also_after_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    let log = folder(data).join(file_name(STARTED));
    touch(&log, "x\n");
    mark_running(data, &Running { pid: 1, started_at: utc(STARTED), file: log.display().to_string() }).unwrap();
    previous_end(data).unwrap();
    // the start that noticed it writes its own running.json; it quits (running.json goes), and the next start again
    mark_running(data, &Running { pid: 2, started_at: utc(STARTED + 1000), file: "/x/hersir-b.log".into() }).unwrap();
    fs::remove_file(folder(data).join(logs::RUNNING)).unwrap();
    assert!(previous_end(data).is_none(), "a Hersir that quit is no unexpected end");
    assert!(info(data, None).unexpected_end.is_some(), "the earlier unexpected end is kept until it is dismissed");

    dismiss(data).unwrap();
    assert!(info(data, None).unexpected_end.is_none());
    assert!(!folder(data).join(logs::UNEXPECTED_END).exists());
    // after a restart (a start whose previous Hersir quit) it stays dismissed
    assert!(previous_end(data).is_none());
    assert!(info(data, None).unexpected_end.is_none());
    // dismissing nothing is fine too
    dismiss(data).unwrap();
}

#[test]
fn a_newer_unexpected_end_replaces_the_older_one() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    let (a, b) = (folder(data).join(file_name(STARTED)), folder(data).join(file_name(STARTED + 3_600_000)));
    touch(&a, "a\n");
    touch(&b, "b\n");
    mark_running(data, &Running { pid: 1, started_at: utc(STARTED), file: a.display().to_string() }).unwrap();
    previous_end(data).unwrap();
    mark_running(data, &Running { pid: 2, started_at: utc(STARTED + 3_600_000), file: b.display().to_string() }).unwrap();
    previous_end(data).unwrap();
    let kept = info(data, None).unexpected_end.unwrap();
    assert_eq!(kept.file, b.display().to_string());
    assert_eq!(kept.started_at, "2026-11-13T15:50:26Z");
}

#[test]
fn the_previous_log_is_found_by_name_in_this_logs_folder_when_the_data_folder_moved() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    let here = folder(data).join(file_name(STARTED));
    touch(&here, "x\n");
    mark_running(data, &Running { pid: 7, started_at: utc(STARTED), file: format!("/old/place/logs/{}", file_name(STARTED)) }).unwrap();
    let (end, said) = previous_end(data).unwrap();
    assert_eq!(end.file, here.display().to_string());
    assert!(said.contains(&here.display().to_string()), "{said}");
}

#[test]
fn a_previous_log_that_is_gone_was_last_written_when_it_started() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    fs::create_dir_all(folder(data)).unwrap();
    mark_running(data, &Running { pid: 7, started_at: utc(STARTED), file: folder(data).join("hersir-gone.log").display().to_string() }).unwrap();
    let (end, _) = previous_end(data).unwrap();
    assert_eq!(end.last_written_at, "2026-11-13T14:50:26Z");
}

#[test]
fn a_running_json_that_cant_be_read_is_no_unexpected_end() {
    let dir = tempfile::tempdir().unwrap();
    touch(&folder(dir.path()).join(logs::RUNNING), "{not json");
    assert!(previous_end(dir.path()).is_none());
    assert!(info(dir.path(), None).unexpected_end.is_none());
}

#[test]
fn running_json_holds_the_pid_the_start_time_and_the_log() {
    let dir = tempfile::tempdir().unwrap();
    // `start` makes the logs folder before it marks this start as running
    fs::create_dir_all(folder(dir.path())).unwrap();
    mark_running(dir.path(), &Running { pid: 4242, started_at: utc(STARTED), file: "/d/logs/hersir-x.log".into() }).unwrap();
    let json: serde_json::Value = serde_json::from_str(&fs::read_to_string(folder(dir.path()).join("running.json")).unwrap()).unwrap();
    assert_eq!(json, serde_json::json!({"pid": 4242, "startedAt": "2026-11-13T14:50:26Z", "file": "/d/logs/hersir-x.log"}));
    // no half-written file is left beside it
    let names: Vec<String> = fs::read_dir(folder(dir.path())).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(names, vec!["running.json".to_string()]);
}

/// `n` logs a minute apart from STARTED, oldest first, written in a shuffled order so the file times don't follow the names.
fn logs_in(dir: &Path, n: usize) -> Vec<PathBuf> {
    let paths: Vec<PathBuf> = (0..n).map(|i| dir.join(file_name(STARTED + i as i64 * 60_000))).collect();
    for i in (0..n).rev() {
        touch(&paths[i], "x\n");
    }
    paths
}

fn names_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    names
}

#[test]
fn only_the_20_newest_logs_stay_by_their_start_time() {
    assert_eq!(logs::KEEP, 20);
    let dir = tempfile::tempdir().unwrap();
    let paths = logs_in(dir.path(), 25);
    // files that aren't Hersir's logs stay
    for other in ["running.json", "unexpected-end.json", "notes.txt", "hersir-notes.txt", "gizai-2026.log"] {
        touch(&dir.path().join(other), "x");
    }
    assert_eq!(remove_old(dir.path(), &[]), 5);
    for (i, p) in paths.iter().enumerate() {
        assert_eq!(p.exists(), i >= 5, "{}", p.display());
    }
    for other in ["running.json", "unexpected-end.json", "notes.txt", "hersir-notes.txt", "gizai-2026.log"] {
        assert!(dir.path().join(other).exists(), "{other}");
    }
    // a second cleanup removes nothing more
    assert_eq!(remove_old(dir.path(), &[]), 0);
}

#[test]
fn twenty_logs_or_fewer_all_stay() {
    let dir = tempfile::tempdir().unwrap();
    logs_in(dir.path(), 20);
    assert_eq!(remove_old(dir.path(), &[]), 0);
    assert_eq!(names_in(dir.path()).len(), 20);
    assert_eq!(remove_old(&dir.path().join("missing"), &[]), 0);
}

#[test]
fn the_cleanup_never_removes_a_log_it_is_told_to_keep() {
    let dir = tempfile::tempdir().unwrap();
    let paths = logs_in(dir.path(), 24);
    // the oldest (an unexpected end not dismissed yet) and the third oldest stay, as named by a full path elsewhere
    let elsewhere = Path::new("/somewhere/else/logs").join(paths[2].file_name().unwrap());
    assert_eq!(remove_old(dir.path(), &[&paths[0], &elsewhere]), 2);
    assert!(paths[0].exists() && paths[2].exists());
    assert!(!paths[1].exists() && !paths[3].exists());
    assert_eq!(names_in(dir.path()).len(), 22);
}

#[test]
fn starts_in_the_same_second_count_as_newer_by_their_number() {
    let dir = tempfile::tempdir().unwrap();
    // 19 newer logs, then three in one second: …26Z.log, …26Z-2.log and …26Z-10.log (10 after 2, not before)
    let older = dir.path().join("hersir-2026-11-13T14-50-26Z.log");
    let second = dir.path().join("hersir-2026-11-13T14-50-26Z-2.log");
    let tenth = dir.path().join("hersir-2026-11-13T14-50-26Z-10.log");
    for p in [&older, &second, &tenth] {
        touch(p, "x\n");
    }
    for i in 1..=19 {
        touch(&dir.path().join(file_name(STARTED + i * 60_000)), "x\n");
    }
    assert_eq!(remove_old(dir.path(), &[]), 2);
    assert!(tenth.exists(), "the newest start in that second stays");
    assert!(!second.exists() && !older.exists());
}

#[test]
fn proc_stat_gives_the_parent_the_name_and_the_resident_pages() {
    let stat = "4242 (WebKitWebProcess) S 4100 4242 4100 0 -1 4194560 1 0 0 0 1 2 0 0 20 0 9 0 100 2000000 31337 18446744073709551615";
    assert_eq!(parse_stat(4242, stat), Some(Proc { pid: 4242, ppid: 4100, name: "WebKitWebProcess".into(), rss_pages: 31337 }));
    // a name with spaces and brackets ends at the last ')'
    let odd = "77 (a (b) c) R 1 77 77 0 -1 0 0 0 0 0 0 0 0 0 20 0 1 0 5 100 12 0";
    assert_eq!(parse_stat(77, odd), Some(Proc { pid: 77, ppid: 1, name: "a (b) c".into(), rss_pages: 12 }));
    assert_eq!(parse_stat(1, ""), None);
    assert_eq!(parse_stat(1, "1 (x) S 0 1"), None);
}

#[cfg(target_os = "linux")]
#[test]
fn proc_stat_of_this_test_process_reads_and_names_its_parent() {
    let me = std::process::id();
    let p = parse_stat(me, &fs::read_to_string(format!("/proc/{me}/stat")).unwrap()).unwrap();
    assert_eq!(p.pid, me);
    assert_eq!(p.ppid, std::os::unix::process::parent_id());
    assert!(p.rss_pages > 0);
    assert!(logs::own_memory().is_some_and(|b| b > 0));
}

fn proc(pid: u32, ppid: u32, name: &str, rss_pages: u64) -> Proc {
    Proc { pid, ppid, name: name.into(), rss_pages }
}

#[test]
fn hersirs_own_memory_is_its_process_and_the_webkit_processes_below_it() {
    let procs = vec![
        proc(1, 0, "systemd", 1_000),
        proc(100, 1, "hersir", 50),
        // its page and network processes, and the GPU process a sandbox starts through bwrap
        proc(101, 100, "WebKitWebProces", 20),
        proc(102, 100, "WebKitNetworkPr", 4),
        proc(103, 100, "bwrap", 1),
        proc(104, 103, "WebKitGPUProces", 6),
        // an agent run Hersir started isn't WebKit: not counted
        proc(200, 100, "claude", 300),
        // another app's WebKit is not Hersir's
        proc(300, 1, "epiphany", 70),
        proc(301, 300, "WebKitWebProces", 90),
    ];
    assert_eq!(own_pages(100, &procs), Some(50 + 20 + 4 + 6));
    assert_eq!(own_pages(999, &procs), None, "a Hersir missing from /proc says nothing");
}

#[test]
fn a_loop_in_the_parents_ends_the_walk_up() {
    let procs = vec![proc(100, 1, "hersir", 10), proc(5, 6, "WebKitWebProces", 7), proc(6, 5, "WebKitWebProces", 8)];
    assert_eq!(own_pages(100, &procs), Some(10));
}

#[test]
fn the_self_test_panic_is_refused_outside_a_self_test_run() {
    assert_eq!(panic_command(false), Err("only in a self-test run".to_string()));
    assert_eq!(panic_command(true), Ok(()));
}
