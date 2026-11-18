// GA-207: get_client, get_project and get_task list each file on a client, project or task with the path of a copy in
// the Team Lead's own folder (`<data>/lead/files/<file id>/<name>`), to read with Read. A copy is made once and reused;
// a file over 100 MB isn't copied (`note`), and one that can't be copied says why (`error`) while the rest still comes.
use std::path::{Path, PathBuf};

use gizai_core::model::*;
use gizai_core::{clients, files, projects, tasks, team};
use gizai_lib::{AppState, chat as app_chat, tools};
use serde_json::{Value, json};

struct T {
    st: AppState,
    lead: String,
    dir: tempfile::TempDir,
}

fn setup() -> T {
    let dir = tempfile::tempdir().unwrap();
    let st = gizai_lib::test_state(dir.path());
    let team_id = team::list(&st.db).unwrap()[0].id.clone();
    let lead = team::add_agent(&st.db, &st.you_id, &team_id, AgentInput {
        name: "Team Lead".into(), role_key: "lead".into(), chat_enabled: Some(true), ..Default::default() }).unwrap();
    T { st, lead, dir }
}

impl T {
    async fn ok(&self, name: &str, args: Value) -> Value {
        tools::call(&self.st, &self.lead, name, args).await.unwrap_or_else(|e| panic!("{name} failed: {e}"))
    }
    fn project(&self, name: &str, key: &str) -> String {
        projects::create(&self.st.db, &self.st.you_id, ProjectInput { name: name.into(), key: key.into(), ..Default::default() }).unwrap()
    }
    /// Uploads a file with `body` from a folder outside Gizai's (like ~/Downloads) to `owner_type` `owner_id`.
    fn add(&self, owner_type: &str, owner_id: &str, name: &str, body: &[u8]) -> FileRow {
        let d = self.dir.path().join("downloads").join(gizai_core::ids::new_id());
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join(name);
        std::fs::write(&p, body).unwrap();
        files::add_from_path(&self.st.db, &self.st.you_id, &self.st.data_dir, owner_type, owner_id, &p).unwrap()
    }
    fn copy_of(&self, f: &FileRow) -> PathBuf {
        app_chat::lead_file_path(&self.st, f)
    }
    /// Sets a stored file's size in the database (a real upload of over 100 MB is too big for a test of the threshold).
    fn set_size(&self, f: &FileRow, size: i64) {
        let sql = format!("UPDATE files SET size_bytes = {size} WHERE id = ?1");
        self.st.db.write(None, |w| { w.conn().execute(&sql, [f.id.as_str()])?; Ok(()) }).unwrap();
    }
}

/// The line in a get's `files` for `f`.
fn line<'a>(got: &'a Value, f: &FileRow) -> &'a Value {
    got["files"].as_array().unwrap().iter().find(|l| l["id"] == f.id.as_str()).unwrap_or_else(|| panic!("{} not listed: {got}", f.name))
}

/// Checks that `l` lists `f` with every field and a path to a copy with the upload's name and bytes.
fn assert_copied(t: &T, l: &Value, f: &FileRow, body: &[u8]) {
    assert_eq!(l["id"], f.id.as_str(), "{l}");
    assert_eq!(l["name"], f.name.as_str(), "{l}");
    assert_eq!(l["mime"].as_str(), f.mime.as_deref(), "{l}");
    assert_eq!(l["size_bytes"], body.len(), "{l}");
    let added = l["added"].as_str().unwrap_or_else(|| panic!("no added date: {l}"));
    assert_eq!(added.len(), 10, "a yyyy-mm-dd date: {l}");
    assert!(l.get("note").is_none() && l.get("error").is_none(), "{l}");
    let path = PathBuf::from(l["path"].as_str().unwrap_or_else(|| panic!("no path: {l}")));
    assert_eq!(path, t.copy_of(f), "the Team Lead's copy, at chat::lead_file_path");
    assert_eq!(path, t.st.data_dir.join("lead").join("files").join(&f.id).join(&f.name), "<data>/lead/files/<file id>/<name>");
    assert_eq!(path.file_name().unwrap().to_string_lossy(), f.name, "the upload's name");
    assert_eq!(std::fs::read(&path).unwrap(), body, "the same bytes");
}

#[tokio::test]
async fn get_project_lists_a_md_file_with_its_id_mime_and_the_path_of_a_copy_in_the_team_leads_folder() {
    let t = setup();
    let p = t.project("Stapsgewijs", "SDH");
    let body = "# Adviesplan\n\nDeel 1: het datamodel, met één tabel per stap.\n".as_bytes();
    let f = t.add("project", &p, "Adviesplan deel 1.md", body);
    assert!(!t.copy_of(&f).exists(), "no copy before the Team Lead asks");

    let got = t.ok("get_project", json!({"project": "SDH"})).await;
    assert_eq!(got["project"]["name"], "Stapsgewijs", "the rest of the result: {got}");
    assert_eq!(got["files"].as_array().unwrap().len(), 1, "{got}");
    let l = line(&got, &f);
    assert_eq!(l["mime"], "text/markdown", "{l}");
    assert_copied(&t, l, &f, body);
    // inside the Team Lead's own folder, its working folder in chat and in a board check
    assert!(PathBuf::from(l["path"].as_str().unwrap()).starts_with(t.st.data_dir.join("lead")), "{l}");
}

#[tokio::test]
async fn get_task_and_get_client_do_the_same_for_a_file_on_a_task_and_one_on_a_client() {
    let t = setup();
    let client = clients::create(&t.st.db, &t.st.you_id, ClientInput { name: "Spoorwegmuseum".into(), ..Default::default() }).unwrap();
    let p = t.project("Kade portal", "KADE");
    let task = tasks::create(&t.st.db, &t.st.you_id, TaskInput { project_id: p, title: "Spec review".into(), ..Default::default() }).unwrap();
    let ident = tasks::get(&t.st.db, &task).unwrap().identifier;
    let (on_task, on_client) = (b"id,name\n1,Kade\n".to_vec(), b"%PDF-1.4 the contract".to_vec());
    let tf = t.add("task", &task, "export.csv", &on_task);
    let cf = t.add("client", &client, "contract.pdf", &on_client);

    let got = t.ok("get_task", json!({"task": ident})).await;
    assert_eq!(got["task"]["title"], "Spec review", "{got}");
    assert_eq!(line(&got, &tf)["mime"], "text/csv");
    assert_copied(&t, line(&got, &tf), &tf, &on_task);

    let got = t.ok("get_client", json!({"client": "Spoorwegmuseum"})).await;
    assert_eq!(got["client"]["name"], "Spoorwegmuseum", "{got}");
    assert_eq!(line(&got, &cf)["mime"], "application/pdf");
    assert_copied(&t, line(&got, &cf), &cf, &on_client);
}

#[tokio::test]
async fn a_second_get_gives_the_same_path_and_does_not_copy_the_file_again() {
    let t = setup();
    let p = t.project("Stapsgewijs", "SDH");
    let f = t.add("project", &p, "Adviesplan.md", b"# Adviesplan");
    let first = t.ok("get_project", json!({"project": "SDH"})).await;
    let path = PathBuf::from(line(&first, &f)["path"].as_str().unwrap());
    // mark the copy: a new copy would overwrite it with the stored file
    let mut perm = std::fs::metadata(&path).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    perm.set_readonly(false);
    std::fs::set_permissions(&path, perm).unwrap();
    std::fs::write(&path, "the copy from before").unwrap();

    let again = t.ok("get_project", json!({"project": "SDH"})).await;
    assert_eq!(line(&again, &f)["path"], line(&first, &f)["path"], "{again}");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "the copy from before", "reused, not copied again");
    // also when its stored file has gone since: the copy is there already
    remove_blob(&t.st.data_dir, &f);
    let third = t.ok("get_project", json!({"project": "SDH"})).await;
    assert_eq!(line(&third, &f)["path"], line(&first, &f)["path"], "{third}");
    assert!(line(&third, &f).get("error").is_none(), "{third}");
}

#[tokio::test]
async fn two_gets_at_once_both_give_the_copy() {
    let t = setup();
    let p = t.project("Stapsgewijs", "SDH");
    let body = vec![7u8; 4 << 20];
    let f = t.add("project", &p, "big.txt", &body);
    let (a, b) = tokio::join!(t.ok("get_project", json!({"project": "SDH"})), t.ok("get_project", json!({"project": "SDH"})));
    assert_copied(&t, line(&a, &f), &f, &body);
    assert_copied(&t, line(&b, &f), &f, &body);
}

#[tokio::test]
async fn two_files_with_the_same_name_on_different_projects_get_different_paths() {
    let t = setup();
    let (a, b) = (t.project("Stapsgewijs", "SDH"), t.project("Kade portal", "KADE"));
    let fa = t.add("project", &a, "README.md", b"SDH's readme");
    let fb = t.add("project", &b, "README.md", b"Kade's readme");
    let (ga, gb) = (t.ok("get_project", json!({"project": "SDH"})).await, t.ok("get_project", json!({"project": "KADE"})).await);
    let (pa, pb) = (line(&ga, &fa)["path"].as_str().unwrap().to_string(), line(&gb, &fb)["path"].as_str().unwrap().to_string());
    assert_ne!(pa, pb);
    assert_copied(&t, line(&ga, &fa), &fa, b"SDH's readme");
    assert_copied(&t, line(&gb, &fb), &fb, b"Kade's readme");
    // the same file (same bytes, same name) uploaded to both is two files too
    let (sa, sb) = (t.add("project", &a, "same.md", b"same"), t.add("project", &b, "same.md", b"same"));
    let (ga, gb) = (t.ok("get_project", json!({"project": "SDH"})).await, t.ok("get_project", json!({"project": "KADE"})).await);
    assert_ne!(line(&ga, &sa)["path"], line(&gb, &sb)["path"]);
    assert_copied(&t, line(&ga, &sa), &sa, b"same");
    assert_copied(&t, line(&gb, &sb), &sb, b"same");
}

#[tokio::test]
async fn a_file_larger_than_100_mb_is_listed_without_a_path_and_is_not_copied() {
    let t = setup();
    let p = t.project("Stapsgewijs", "SDH");
    // a real upload of 100 MB and one byte (sparse where the system allows it)
    let src = t.dir.path().join("downloads").join("recording.mp4");
    std::fs::create_dir_all(src.parent().unwrap()).unwrap();
    std::fs::File::create(&src).unwrap().set_len((100 << 20) + 1).unwrap();
    let big = files::add_from_path(&t.st.db, &t.st.you_id, &t.st.data_dir, "project", &p, &src).unwrap();
    let small = t.add("project", &p, "notes.md", b"notes");

    let got = t.ok("get_project", json!({"project": "SDH"})).await;
    let l = line(&got, &big);
    assert_eq!((l["id"].as_str(), l["name"].as_str(), l["mime"].as_str()), (Some(big.id.as_str()), Some("recording.mp4"), Some("video/mp4")), "{l}");
    assert_eq!(l["size_bytes"], (100 << 20) + 1, "{l}");
    assert!(l["added"].is_string(), "{l}");
    assert!(l.get("path").is_some() && l["path"].is_null(), "path: null: {l}");
    assert_eq!(l["note"], "larger than 100 MB: not copied", "{l}");
    assert!(l.get("error").is_none(), "{l}");
    assert!(!t.copy_of(&big).exists() && !t.copy_of(&big).parent().unwrap().exists(), "nothing is copied for it");
    // the other file still comes with its copy
    assert_copied(&t, line(&got, &small), &small, b"notes");
}

#[tokio::test]
async fn exactly_100_mb_is_still_copied_and_one_byte_more_is_not() {
    let t = setup();
    let p = t.project("Stapsgewijs", "SDH");
    let (at, over) = (t.add("project", &p, "at.md", b"at the limit"), t.add("project", &p, "over.md", b"over the limit"));
    t.set_size(&at, 100 << 20);
    t.set_size(&over, (100 << 20) + 1);
    let got = t.ok("get_project", json!({"project": "SDH"})).await;
    assert_eq!(line(&got, &at)["path"].as_str().map(PathBuf::from), Some(t.copy_of(&at)), "{got}");
    assert!(line(&got, &at).get("note").is_none(), "{got}");
    assert!(line(&got, &over)["path"].is_null(), "{got}");
    assert_eq!(line(&got, &over)["note"], "larger than 100 MB: not copied");
    assert!(!t.copy_of(&over).exists());
}

/// Deletes a file's stored blob (`<data>/files/<sha[0..2]>/<sha>`), which is read-only.
fn remove_blob(data: &Path, f: &FileRow) {
    let blob = files::blob_path(data, &f.sha256);
    let mut perm = std::fs::metadata(&blob).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    perm.set_readonly(false);
    std::fs::set_permissions(&blob, perm).unwrap();
    std::fs::remove_file(&blob).unwrap();
}

#[tokio::test]
async fn a_file_whose_stored_file_is_missing_gets_an_error_and_the_other_files_and_the_rest_still_come() {
    let t = setup();
    let p = projects::create(&t.st.db, &t.st.you_id, ProjectInput {
        name: "Stapsgewijs".into(), key: "SDH".into(), goal_md: Some("Een adviesplan".into()), ..Default::default() }).unwrap();
    let gone = t.add("project", &p, "Adviesplan deel 2.md", b"part two");
    let (one, three) = (t.add("project", &p, "Adviesplan deel 1.md", b"part one"), t.add("project", &p, "Adviesplan deel 3.md", b"part three"));
    remove_blob(&t.st.data_dir, &gone);

    let got = t.ok("get_project", json!({"project": "SDH"})).await;
    assert_eq!(got["project"]["goal_md"], "Een adviesplan", "{got}");
    assert!(got["tasks_by_column"].is_object() && got["docs"].is_array(), "{got}");
    assert_eq!(got["files"].as_array().unwrap().len(), 3, "{got}");
    let l = line(&got, &gone);
    assert!(l.get("path").is_some() && l["path"].is_null(), "{l}");
    let e = l["error"].as_str().unwrap_or_else(|| panic!("an error: {l}"));
    assert!(e.contains("missing"), "says why: {e}");
    assert_eq!((l["name"].as_str(), l["size_bytes"].as_i64()), (Some("Adviesplan deel 2.md"), Some(8)), "{l}");
    assert!(!t.copy_of(&gone).exists(), "no empty or partial copy");
    assert_copied(&t, line(&got, &one), &one, b"part one");
    assert_copied(&t, line(&got, &three), &three, b"part three");
}

#[tokio::test]
async fn when_the_team_leads_folder_cant_take_copies_each_file_says_so_and_the_rest_still_comes() {
    let t = setup();
    let p = t.project("Stapsgewijs", "SDH");
    let f = t.add("project", &p, "Adviesplan.md", b"# Adviesplan");
    // a file where the folder of copies should be
    std::fs::create_dir_all(t.st.data_dir.join("lead")).unwrap();
    std::fs::write(app_chat::lead_files_dir(&t.st), "not a folder").unwrap();
    let got = t.ok("get_project", json!({"project": "SDH"})).await;
    assert_eq!(got["project"]["key"], "SDH", "{got}");
    let l = line(&got, &f);
    assert!(l["path"].is_null(), "{l}");
    let e = l["error"].as_str().unwrap_or_else(|| panic!("an error: {l}"));
    assert!(e.starts_with("couldn't be copied into your folder: ") && !e.contains("missing"), "{e}");
}

#[tokio::test]
async fn a_removed_file_is_not_listed() {
    let t = setup();
    let p = t.project("Stapsgewijs", "SDH");
    let (kept, removed) = (t.add("project", &p, "kept.md", b"kept"), t.add("project", &p, "removed.md", b"removed"));
    files::remove(&t.st.db, &t.st.you_id, &removed.id).unwrap();
    let got = t.ok("get_project", json!({"project": "SDH"})).await;
    let names: Vec<&str> = got["files"].as_array().unwrap().iter().map(|l| l["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["kept.md"], "{got}");
    assert_copied(&t, line(&got, &kept), &kept, b"kept");
    assert!(!t.copy_of(&removed).exists(), "no copy of a removed file");
}

#[tokio::test]
async fn without_files_the_gets_list_none_and_make_no_folder() {
    let t = setup();
    t.project("Stapsgewijs", "SDH");
    let got = t.ok("get_project", json!({"project": "SDH"})).await;
    assert_eq!(got["files"], json!([]), "{got}");
    assert!(!app_chat::lead_files_dir(&t.st).exists());
}

#[tokio::test]
async fn a_board_check_gets_the_same_paths() {
    let t = setup();
    let p = t.project("Stapsgewijs", "SDH");
    let f = t.add("project", &p, "Adviesplan.md", b"# Adviesplan");
    let got = tools::call_check(&t.st, &t.lead, "check-1", "get_project", json!({"project": "SDH"})).await.unwrap();
    assert_copied(&t, line(&got, &f), &f, b"# Adviesplan");
}

#[test]
fn the_three_descriptions_say_each_file_comes_with_the_path_of_a_copy_to_read_with_read() {
    let cat = tools::catalog();
    for name in ["get_client", "get_project", "get_task"] {
        let d = &cat.iter().find(|t| t.name == name).unwrap().description;
        assert!(d.contains("Each file comes with the path of a copy in your folder, to read with Read"), "{name}: {d}");
        assert!(d.contains("over 100 MB"), "{name}: {d}");
    }
}

// ---- GA-310 QA: readable copies of Word, Excel, PowerPoint, OpenDocument and RTF files in get_client, get_project and
// get_task ----

use super::office_docs::{damaged_xlsx, docx, docx_md, locked_docx};

/// Checks that `l` has a readable_path beside its copy (`<path>.md`) with `text`, and no readable_error.
fn assert_readable(l: &Value, text: &str) -> PathBuf {
    let path = PathBuf::from(l["path"].as_str().unwrap_or_else(|| panic!("no path: {l}")));
    let readable = PathBuf::from(l["readable_path"].as_str().unwrap_or_else(|| panic!("no readable_path: {l}")));
    assert_eq!(readable, path.with_file_name(format!("{}.md", path.file_name().unwrap().to_string_lossy())), "beside the copy: {l}");
    assert!(l.get("readable_error").is_none(), "{l}");
    assert_eq!(std::fs::read_to_string(&readable).unwrap(), text, "{l}");
    readable
}

#[tokio::test]
async fn get_task_get_project_and_get_client_give_a_docx_a_readable_path_and_a_second_call_reuses_it() {
    let t = setup();
    let client = clients::create(&t.st.db, &t.st.you_id, ClientInput { name: "Spoorwegmuseum".into(), ..Default::default() }).unwrap();
    let p = t.project("Kade portal", "KADE");
    let task = tasks::create(&t.st.db, &t.st.you_id, TaskInput { project_id: p.clone(), title: "Spec review".into(), ..Default::default() }).unwrap();
    let ident = tasks::get(&t.st.db, &task).unwrap().identifier;
    let (tb, pb, cb) = (docx("Spec", "On the task."), docx("Plan", "On the project."), docx("Contract", "On the client."));
    let tf = t.add("task", &task, "Spec v2.docx", &tb);
    let pf = t.add("project", &p, "Plan.DOCX", &pb);
    let cf = t.add("client", &client, "Contract.docx", &cb);

    let got = t.ok("get_task", json!({"task": ident})).await;
    assert_copied(&t, line(&got, &tf), &tf, &tb);
    let on_task = assert_readable(line(&got, &tf), &docx_md("Spec v2.docx", "Spec", "On the task."));
    let got = t.ok("get_project", json!({"project": "KADE"})).await;
    assert_copied(&t, line(&got, &pf), &pf, &pb);
    assert_readable(line(&got, &pf), &docx_md("Plan.DOCX", "Plan", "On the project."));
    let got = t.ok("get_client", json!({"client": "Spoorwegmuseum"})).await;
    assert_copied(&t, line(&got, &cf), &cf, &cb);
    assert_readable(line(&got, &cf), &docx_md("Contract.docx", "Contract", "On the client."));

    // a second call gives the same path and keeps the readable copy as it is (a new one would overwrite the mark)
    std::fs::write(&on_task, "marked").unwrap();
    let again = t.ok("get_task", json!({"task": ident})).await;
    assert_eq!(line(&again, &tf)["readable_path"].as_str(), Some(on_task.display().to_string().as_str()), "{again}");
    assert_eq!(std::fs::read_to_string(&on_task).unwrap(), "marked");
}

#[tokio::test]
async fn a_damaged_xlsx_a_locked_docx_and_a_renamed_txt_have_their_copy_and_a_readable_error_and_the_other_files_still_come() {
    let t = setup();
    let p = t.project("Stapsgewijs", "SDH");
    let (bad, locked, renamed, good) = (damaged_xlsx(), locked_docx(), b"plain text, renamed".to_vec(), docx("Fine", "Still read."));
    let fb = t.add("project", &p, "broken.xlsx", &bad);
    let fl = t.add("project", &p, "locked.docx", &locked);
    let fr = t.add("project", &p, "renamed.docx", &renamed);
    let fg = t.add("project", &p, "good.docx", &good);
    let fp = t.add("project", &p, "photo.png", b"\x89PNG\r\n\x1a\n not really");

    let got = t.ok("get_project", json!({"project": "SDH"})).await;
    assert_eq!(got["files"].as_array().unwrap().len(), 5, "{got}");
    for (f, body, says) in [(&fb, &bad, "Excel workbook"), (&fl, &locked, "password"), (&fr, &renamed, "Word document")] {
        let l = line(&got, f);
        assert_copied(&t, l, f, body);
        let why = l["readable_error"].as_str().unwrap_or_else(|| panic!("no readable_error: {l}"));
        assert!(why.contains(says), "{l}");
        assert!(l.get("readable_path").is_none(), "{l}");
        assert!(!t.copy_of(f).with_file_name(format!("{}.md", f.name)).exists(), "{l}");
    }
    assert_readable(line(&got, &fg), &docx_md("good.docx", "Fine", "Still read."));
    assert!(line(&got, &fp)["path"].is_string(), "{got}");
}

#[tokio::test]
async fn png_pdf_txt_md_csv_and_other_files_have_exactly_the_fields_they_had() {
    let t = setup();
    let p = t.project("Stapsgewijs", "SDH");
    let mut fs = vec![];
    for name in ["shot.png", "contract.pdf", "notes.txt", "README.md", "export.csv", "data.json", "old.doc", "slides.ppt", "a.pages", "archive.zip"] {
        fs.push(t.add("project", &p, name, format!("body of {name}").as_bytes()));
    }
    let got = t.ok("get_project", json!({"project": "SDH"})).await;
    for f in &fs {
        let l = line(&got, f);
        let mut keys: Vec<&str> = l.as_object().unwrap().keys().map(String::as_str).collect();
        keys.sort();
        assert_eq!(keys, ["added", "id", "mime", "name", "path", "size_bytes"], "{l}");
        assert_copied(&t, l, f, format!("body of {}", f.name).as_bytes());
        let dir: Vec<_> = std::fs::read_dir(t.copy_of(f).parent().unwrap()).unwrap().collect();
        assert_eq!(dir.len(), 1, "only the copy, no readable copy beside it: {}", f.name);
    }
}

#[tokio::test]
async fn a_file_copied_before_readable_copies_existed_gets_one_the_next_time_it_is_listed() {
    let t = setup();
    let p = t.project("Stapsgewijs", "SDH");
    let body = docx("Earlier", "Copied before.");
    let f = t.add("project", &p, "earlier.docx", &body);
    // the Chief's copy as Hersir made it before GA-310: the file alone
    let copy = files::copy_out(&t.st.data_dir, &f, &t.st.data_dir.join("lead").join("files")).unwrap();
    assert_eq!(copy, t.copy_of(&f));
    let md = copy.with_file_name("earlier.docx.md");
    assert!(!md.exists());

    let got = t.ok("get_project", json!({"project": "SDH"})).await;
    assert_copied(&t, line(&got, &f), &f, &body);
    assert_eq!(assert_readable(line(&got, &f), &docx_md("earlier.docx", "Earlier", "Copied before.")), md);
}

#[test]
fn the_three_descriptions_say_a_word_excel_powerpoint_opendocument_or_rtf_file_has_a_readable_path() {
    let cat = tools::catalog();
    for name in ["get_client", "get_project", "get_task"] {
        let d = &cat.iter().find(|t| t.name == name).unwrap().description;
        assert!(d.contains("Each file comes with the path of a copy in your folder, to read with Read"), "{name}: {d}");
        assert!(d.contains("A Word, Excel, PowerPoint, OpenDocument or RTF file also has readable_path: a copy of its text as Markdown, to read with Read."),
                "{name}: {d}");
    }
}

#[test]
fn the_chiefs_docs_end_the_files_line_with_the_cards_sentence() {
    let doc = include_str!("../../../docs/features/chief.md");
    let line = doc.lines().find(|l| l.starts_with("- **Files:**")).expect("the Files line");
    assert!(line.trim_end().ends_with("Word, Excel, PowerPoint, OpenDocument and RTF files also get a text copy for the Chief, with headings, lists and tables as Markdown."),
            "{line}");
}
