//! GA-323 QA (GA-270 #6): from chat, an agent's commands only come from an allow list.
//! - create_agent and update_agent save at once only the allowed commands the agent has already (without a list of its own
//!   it runs with Hersir's default list) and the 14 entries of that list that run no other command (`seed::CHAT_MAY_ADD`).
//! - Any other entry (each command of the audit's probe, the rest of the default list, the role's own extras) keeps the
//!   whole call for the user's Approve, in any chat, marked `new commands`, with a sentence that names it; outside a chat
//!   it can't wait, and nothing changes. Approve runs it as kept, with every other rule; Discard changes nothing.
//! - create_agent without allowed_tools (or with an empty list) gets its role's list at once.
//! - The agent form (`team::add_agent`, `team::update_agent`, what its commands call) saves any entry, as before.
//! - The tool's description and docs/agent-tools.md name the same 14 entries.
//! Tools are called the way the MCP server calls them, in a chat thread (`tools::call_in`).
use gizai_core::model::*;
use gizai_core::seed::{CHAT_MAY_ADD, DEFAULT_TOOLS, role_tools};
use gizai_core::{chat, team};
use gizai_lib::{AppState, chat as app_chat, pending, tools};
use serde_json::{Value, json};

const FAKE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../crates/gizai-agents/tests/fake-claude.sh");

/// What the Backend Agent may run: a list of its own, narrowed in the agent form.
const NARROW: [&str; 2] = ["Bash(git status:*)", "Bash(ls:*)"];

/// The 14 entries the hand-over names, in its order.
const MAY_ADD: [&str; 14] = ["Bash(git status:*)", "Bash(git add:*)", "Bash(ls:*)", "Bash(cat:*)", "Bash(head:*)", "Bash(tail:*)", "Bash(wc:*)",
                             "Bash(cut:*)", "Bash(diff:*)", "Bash(grep:*)", "Bash(jq:*)", "Bash(pwd:*)", "Bash(which:*)", "Bash(sleep:*)"];

fn strs(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

/// "A", "A and B", "A, B and C", as the result and the preview name entries.
fn listed(v: &[&str]) -> String {
    match v.split_last() {
        None => "none".into(),
        Some((last, [])) => last.to_string(),
        Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
    }
}

/// What a call kept for its new commands returns, word for word.
fn kept_for(tool: &str, entries: &[&str], who: &str) -> String {
    let list = listed(entries);
    let is = if entries.len() == 1 { "is" } else { "are" };
    format!("{tool} wasn't run: {list} {is} new for {who}, and from chat an agent gets a command it doesn't have at once only when it is one of \
             Hersir's default list that runs no other command and writes no file. Hersir keeps the call for the user to approve with one click. \
             Tell them in a sentence what it does, and name {list}.")
}

/// The preview's fields: (label, from, value).
fn fields(p: &pending::PendingView) -> Vec<(String, Option<String>, String)> {
    let v = serde_json::to_value(p).unwrap();
    v["preview"]["fields"].as_array().unwrap().iter().map(|f| (f["label"].as_str().unwrap().to_string(),
        f["from"].as_str().map(str::to_string), f["value"].as_str().unwrap().to_string())).collect()
}

struct T {
    st: AppState,
    lead: String,
    team_id: String,
    _dir: tempfile::TempDir,
}

/// A Chief (Chat on) and a Backend Agent whose allowed commands the user narrowed to `NARROW` in the agent form. The
/// queue is paused, so nothing starts by itself.
fn setup() -> T {
    let dir = tempfile::tempdir().unwrap();
    let st = gizai_lib::test_state(dir.path());
    gizai_core::settings::set(&st.db, "claude_bin", &FAKE.to_string()).unwrap();
    gizai_core::settings::set(&st.db, "agents_paused", &true).unwrap();
    let team_id = team::list(&st.db).unwrap()[0].id.clone();
    let lead = team::add_agent(&st.db, &st.you_id, &team_id, AgentInput {
        name: "Team Lead".into(), role_key: "lead".into(), chat_enabled: Some(true), ..Default::default() }).unwrap();
    let t = T { st, lead, team_id, _dir: dir };
    t.form_add("Backend Agent", "backend", &NARROW);
    t
}

impl T {
    async fn call(&self, thread: Option<&str>, name: &str, args: Value) -> Result<Value, String> {
        tools::call_in(&self.st, &self.lead, thread, name, args).await
    }
    async fn ok(&self, thread: Option<&str>, name: &str, args: Value) -> Value {
        self.call(thread, name, args).await.unwrap_or_else(|e| panic!("{name} failed: {e}"))
    }
    /// A chat where you said `text`; no tool from outside Hersir was used there.
    fn thread(&self, text: &str) -> String {
        let id = chat::create_thread(&self.st.db, &self.st.you_id, &self.lead, text).unwrap();
        chat::add_message(&self.st.db, chat::NewMessage { thread_id: id.clone(), role: "user".into(), author_id: Some(self.st.you_id.clone()),
            body_md: Some(text.into()), ..Default::default() }).unwrap();
        id
    }
    /// The agent form's Add (`team::add_agent`, what the form's command calls).
    fn form_add(&self, name: &str, role: &str, tools: &[&str]) -> String {
        team::add_agent(&self.st.db, &self.st.you_id, &self.team_id, AgentInput { name: name.into(), role_key: role.into(),
            allowed_tools: strs(tools), ..Default::default() }).unwrap()
    }
    fn agent(&self, name: &str) -> Option<team::Member> {
        team::all_agents(&self.st.db).unwrap().into_iter().map(|(_, m)| m).find(|m| m.name == name)
    }
    fn backend(&self) -> team::Member {
        self.agent("Backend Agent").unwrap()
    }
    /// The calls waiting in the chat.
    fn pending(&self, thread: &str) -> Vec<pending::PendingView> {
        pending::list(&self.st, thread).unwrap().into_iter().filter(|p| p.action.status == "pending").collect()
    }
    /// The one call of `tool` waiting in the chat, kept for its new commands by the Chief, with exactly `args`.
    fn kept(&self, thread: &str, tool: &str, args: &Value) -> pending::PendingView {
        let all = self.pending(thread);
        let p = all.into_iter().rev().find(|p| p.action.tool == tool && &p.action.args == args).unwrap_or_else(|| panic!("{tool} {args} kept"));
        assert_eq!((p.action.outside_tool.as_str(), p.action.actor_id.as_str()), (gizai_core::pending::NEW_COMMANDS, self.lead.as_str()));
        p
    }
}

/// AC1: from chat, `entry` for an agent that doesn't have it waits for Approve, in update_agent and in create_agent, with a
/// sentence and a preview that name it; nothing changes until you approve, and approving saves the call as kept.
async fn a_new_command_from_chat_waits_for_approve(entry: &str) {
    let t = setup();
    let thread = t.thread("Give the Backend Agent what it needs");
    assert!(!t.backend().allowed_tools.iter().any(|e| e == entry) && !CHAT_MAY_ADD.contains(&entry), "{entry}: the agent doesn't have it");
    let given: Vec<&str> = NARROW.iter().copied().chain([entry]).collect();
    let update = json!({"agent": "Backend Agent", "title": "Builder", "allowed_tools": given});
    let e = t.call(Some(&thread), "update_agent", update.clone()).await.expect_err("update_agent waits");
    assert_eq!(e, kept_for("update_agent", &[entry], "Backend Agent"), "{entry}");
    let m = t.backend();
    assert_eq!((m.allowed_tools, m.title), (strs(&NARROW), None), "{entry}: nothing changed yet, the title neither");
    let u = t.kept(&thread, "update_agent", &update);
    let f = fields(&u);
    assert!(f.contains(&("Needs your OK".into(), None, entry.into())), "{entry}: {f:?}");
    assert!(f.contains(&("Allowed commands".into(), Some(NARROW.join(", ")), given.join(", "))), "{entry}: {f:?}");
    // the chat isn't marked by it: the call waited for its commands, not for a tool from outside Hersir
    assert_eq!(app_chat::used_outside(&t.st, &thread), None);

    let create = json!({"name": "New Agent", "role": "backend", "allowed_tools": [entry]});
    let e = t.call(Some(&thread), "create_agent", create.clone()).await.expect_err("create_agent waits");
    assert_eq!(e, kept_for("create_agent", &[entry], "New Agent"), "{entry}");
    assert!(t.agent("New Agent").is_none(), "{entry}: no agent made yet");
    let c = t.kept(&thread, "create_agent", &create);
    assert!(fields(&c).contains(&("Needs your OK".into(), None, entry.into())), "{entry}: {:?}", fields(&c));
    assert_eq!(t.pending(&thread).len(), 2);

    // Approve runs each call as kept, once
    assert_eq!(pending::approve(&t.st, &u.action.id).await.unwrap().status, "approved");
    let m = t.backend();
    assert_eq!((m.allowed_tools, m.title.as_deref()), (strs(&given), Some("Builder")), "{entry}");
    assert_eq!(pending::approve(&t.st, &c.action.id).await.unwrap().status, "approved");
    assert_eq!(t.agent("New Agent").map(|m| m.allowed_tools), Some(strs(&[entry])), "{entry}");
    assert!(t.pending(&thread).is_empty(), "{entry}: nothing waits again");
}

macro_rules! waits_for_approve {
    ($($test:ident: $entry:literal),* $(,)?) => { $(
        #[tokio::test]
        async fn $test() {
            a_new_command_from_chat_waits_for_approve($entry).await
        }
    )* };
}

waits_for_approve! {
    awk_from_chat_waits_for_approve: "Bash(awk:*)",
    find_from_chat_waits_for_approve: "Bash(find:*)",
    ssh_from_chat_waits_for_approve: "Bash(ssh:*)",
    make_from_chat_waits_for_approve: "Bash(make:*)",
    node_from_chat_waits_for_approve: "Bash(node:*)",
    npx_from_chat_waits_for_approve: "Bash(npx:*)",
    php_from_chat_waits_for_approve: "Bash(php:*)",
    systemd_run_from_chat_waits_for_approve: "Bash(systemd-run:*)",
    script_from_chat_waits_for_approve: "Bash(script:*)",
    parallel_from_chat_waits_for_approve: "Bash(parallel:*)",
}

#[tokio::test]
async fn the_audits_other_commands_the_rest_of_the_default_list_and_the_roles_own_extras_wait_too() {
    let t = setup();
    let thread = t.thread("More commands");
    let rest: Vec<&str> = DEFAULT_TOOLS.iter().copied().filter(|e| !CHAT_MAY_ADD.contains(e)).collect();
    assert_eq!(rest, ["Bash(git diff:*)", "Bash(git log:*)", "Bash(git commit:*)", "Bash(git merge:*)", "Bash(npm:*)", "Bash(npx:*)",
                      "Bash(composer:*)", "Bash(php:*)", "Bash(./vendor/bin/*)", "Bash(cargo:*)", "Bash(pytest:*)", "Bash(rg:*)", "Bash(sort:*)",
                      "Bash(uniq:*)", "Bash(tree:*)"]);
    // the audit's probe; git push and node are on a builder's own list (`role_tools`), which no longer counts
    let probe = ["Bash(git:*)", "Bash(flock:*)", "Bash(ionice:*)", "Bash(chrt:*)", "Bash(taskset:*)", "Bash(git push:*)", "Bash(npm test:*)"];
    assert!(role_tools("backend").iter().any(|e| e == "Bash(git push:*)"));
    for entry in rest.iter().chain(&probe) {
        let args = json!({"agent": "Backend Agent", "allowed_tools": NARROW.iter().chain([entry]).collect::<Vec<_>>()});
        let e = t.call(Some(&thread), "update_agent", args).await.expect_err(entry);
        assert_eq!(e, kept_for("update_agent", &[*entry], "Backend Agent"));
    }
    assert_eq!(t.pending(&thread).len(), rest.len() + probe.len());
    assert_eq!(t.backend().allowed_tools, strs(&NARROW), "nothing changed");

    // several in one call: each named once, in the order given; the entries it has and may get at once aren't named
    let args = json!({"agent": "Backend Agent", "allowed_tools": ["Bash(awk:*)", "Bash(ls:*)", "Bash(find:*)", "Bash(grep:*)", " Bash(awk:*) "]});
    let e = t.call(Some(&thread), "update_agent", args.clone()).await.unwrap_err();
    assert_eq!(e, kept_for("update_agent", &["Bash(awk:*)", "Bash(find:*)"], "Backend Agent"));
    assert!(fields(&t.kept(&thread, "update_agent", &args)).contains(&("Needs your OK".into(), None, "Bash(awk:*) and Bash(find:*)".into())));

    // an empty list is Hersir's default list: for a narrowed agent it waits, naming what it doesn't have and may not get at once
    let args = json!({"agent": "Backend Agent", "allowed_tools": []});
    let e = t.call(Some(&thread), "update_agent", args.clone()).await.unwrap_err();
    assert_eq!(e, kept_for("update_agent", &rest, "Backend Agent"));
    assert_eq!(t.backend().allowed_tools, strs(&NARROW), "nothing changed");
}

#[tokio::test]
async fn entries_the_agent_has_and_the_default_lists_entries_that_run_no_other_command_save_at_once() {
    let t = setup();
    let thread = t.thread("Tidy the commands");
    assert_eq!(CHAT_MAY_ADD, MAY_ADD, "the 14 entries of the hand-over");
    assert!(CHAT_MAY_ADD.iter().all(|e| DEFAULT_TOOLS.contains(e)), "each is on Hersir's default list");

    // all 14, for an agent that has two of them, at once
    t.ok(Some(&thread), "update_agent", json!({"agent": "Backend Agent", "allowed_tools": CHAT_MAY_ADD})).await;
    assert_eq!(t.backend().allowed_tools, strs(&CHAT_MAY_ADD));
    // narrowing saves at once, spaces around an entry are taken off
    t.ok(Some(&thread), "update_agent", json!({"agent": "Backend Agent", "allowed_tools": [" Bash(grep:*) ", "Bash(ls:*)"]})).await;
    assert_eq!(t.backend().allowed_tools, ["Bash(grep:*)", "Bash(ls:*)"]);

    // entries the user gave it in the agent form stay and come back at once, with entries of the 14
    let form = ["Bash(make:*)", "Bash(ssh:*)", "Bash(awk:*)", "Bash(git status:*)"];
    let id = t.backend().actor_id;
    team::update_agent(&t.st.db, &t.st.you_id, &id, AgentInput { name: "Backend Agent".into(), role_key: "backend".into(),
        allowed_tools: strs(&form), ..Default::default() }).unwrap();
    let given = ["Bash(ssh:*)", "Bash(make:*)", "Bash(git status:*)", "Bash(jq:*)", "Bash(sleep:*)"];
    t.ok(Some(&thread), "update_agent", json!({"agent": "Backend Agent", "title": "Builder", "allowed_tools": given})).await;
    let m = t.backend();
    assert_eq!((m.allowed_tools, m.title.as_deref()), (strs(&given), Some("Builder")));
    // a list given back that it had before (awk was taken off just now) is new again
    let e = t.call(Some(&thread), "update_agent", json!({"agent": "Backend Agent", "allowed_tools": ["Bash(awk:*)", "Bash(ssh:*)"]})).await
        .unwrap_err();
    assert_eq!(e, kept_for("update_agent", &["Bash(awk:*)"], "Backend Agent"));

    // an agent without a list of its own runs with Hersir's default list, so every entry of it is one it has
    t.form_add("Default Agent", "backend", &[]);
    t.ok(Some(&thread), "update_agent", json!({"agent": "Default Agent", "allowed_tools": []})).await;
    assert!(t.agent("Default Agent").unwrap().allowed_tools.is_empty());
    let has = ["Bash(npm:*)", "Bash(cargo:*)", "Bash(git commit:*)", "Bash(php:*)", "Bash(./vendor/bin/*)"];
    t.ok(Some(&thread), "update_agent", json!({"agent": "Default Agent", "allowed_tools": has})).await;
    assert_eq!(t.agent("Default Agent").unwrap().allowed_tools, strs(&has));

    // a new agent with entries of the 14 only, at once
    t.ok(Some(&thread), "create_agent", json!({"name": "Reader Agent", "role": "qa", "allowed_tools": ["Bash(cat:*)", "Bash(git add:*)"]})).await;
    assert_eq!(t.agent("Reader Agent").unwrap().allowed_tools, ["Bash(cat:*)", "Bash(git add:*)"]);
    // a call without allowed_tools never waits for them
    t.ok(Some(&thread), "update_agent", json!({"agent": "Backend Agent", "title": "Backend"})).await;
    assert_eq!(t.pending(&thread).len(), 1, "only the awk call waits");
}

#[tokio::test]
async fn create_agent_from_chat_without_allowed_tools_gets_its_roles_list_at_once() {
    let t = setup();
    let thread = t.thread("Hire a few agents");
    for (name, role, args) in [("Backend Agent 2", "backend", json!({})), ("QA Agent", " QA ", json!({})), ("DevOps Agent", "devops", json!({})),
                               ("Second Lead", "lead", json!({})), ("Docs Agent", "docs", json!({})),
                               ("Frontend Agent", "frontend", json!({"allowed_tools": []}))] {
        let mut a = json!({"name": name, "role": role});
        a.as_object_mut().unwrap().extend(args.as_object().unwrap().clone());
        let r = t.ok(Some(&thread), "create_agent", a).await;
        assert_eq!(r["ok"], true, "{name}: {r}");
        let want = role_tools(&team::role_key(role));
        assert_eq!(t.agent(name).unwrap().allowed_tools, want, "{name}: its role's list");
    }
    // the role's list has entries chat gives no agent at once (git push, node, gh pr create): saved, nothing waits
    assert!(t.agent("QA Agent").unwrap().allowed_tools.iter().any(|e| e == "Bash(gh pr create:*)"));
    assert!(t.agent("Backend Agent 2").unwrap().allowed_tools.iter().any(|e| e == "Bash(git push:*)"));
    assert!(t.pending(&thread).is_empty());
    assert_eq!(app_chat::used_outside(&t.st, &thread), None);
}

#[tokio::test]
async fn the_agent_form_saves_any_entry_at_once() {
    let t = setup();
    let any = ["Bash(awk:*)", "Bash(find:*)", "Bash(ssh:*)", "Bash(make:*)", "Bash(node:*)", "Bash(npx:*)", "Bash(php:*)", "Bash(systemd-run:*)",
               "Bash(script:*)", "Bash(parallel:*)", "Bash(git:*)", "Bash(git push:*)", "Bash(bash:*)", "Bash(python3:*)"];
    // Add, as the form's add_agent command calls it
    t.form_add("Form Agent", "backend", &any);
    assert_eq!(t.agent("Form Agent").unwrap().allowed_tools, strs(&any));
    // Save, as the form's update_agent command calls it
    let id = t.backend().actor_id;
    team::update_agent(&t.st.db, &t.st.you_id, &id, AgentInput { name: "Backend Agent".into(), role_key: "backend".into(),
        allowed_tools: strs(&any), ..Default::default() }).unwrap();
    assert_eq!(t.backend().allowed_tools, strs(&any));
    // and chat may then keep them all, at once: the agent has them
    let thread = t.thread("Keep its commands");
    t.ok(Some(&thread), "update_agent", json!({"agent": "Backend Agent", "title": "Builder", "allowed_tools": any})).await;
    assert_eq!(t.backend().title.as_deref(), Some("Builder"));
    assert!(t.pending(&thread).is_empty());
}

#[tokio::test]
async fn discard_changes_nothing_and_the_chiefs_next_answer_hears_it_without_a_tool_from_outside() {
    let t = setup();
    let thread = t.thread("Give it awk");
    let args = json!({"agent": "Backend Agent", "allowed_tools": ["Bash(git status:*)", "Bash(awk:*)"]});
    t.call(Some(&thread), "update_agent", args.clone()).await.unwrap_err();
    let id = t.kept(&thread, "update_agent", &args).action.id;
    // a kept call doesn't mark the chat: the Chief's other tools that act still run at once there
    t.ok(Some(&thread), "set_agent_status", json!({"agent": "Backend Agent", "status": "paused"})).await;
    assert_eq!(t.pending(&thread).len(), 1);
    assert_eq!(pending::discard(&t.st, &id).unwrap().status, "discarded");
    assert_eq!(t.backend().allowed_tools, strs(&NARROW), "nothing changed");
    assert!(t.pending(&thread).is_empty());
    let heard = pending::for_turn(&t.st, &thread);
    assert_eq!(heard.ids, [id]);
    assert!(heard.line.starts_with("[Hersir: ") && heard.line.contains(" decided on calls of yours that waited for their approval:\n- "), "{}", heard.line);
    assert!(heard.line.contains("(your update_agent call)]") && !heard.line.contains("outside Hersir"), "{}", heard.line);
}

#[tokio::test]
async fn after_a_tool_from_outside_the_call_is_kept_once_and_approve_keeps_every_other_rule() {
    let t = setup();
    let thread = t.thread("Read Otus, then give it commands");
    app_chat::mark_outside(&t.st, &thread, "mcp__otus__search");
    // kept once, for the tool from outside Hersir (that rule comes first); approving it saves the new command, no second wait
    let awk = json!({"agent": "Backend Agent", "allowed_tools": ["Bash(git status:*)", "Bash(awk:*)"]});
    let e = t.call(Some(&thread), "update_agent", awk.clone()).await.unwrap_err();
    assert!(e.starts_with("update_agent wasn't run: this chat used mcp__otus__search, a tool from outside Hersir."), "{e}");
    let kept = t.pending(&thread);
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].action.outside_tool, "mcp__otus__search");
    assert!(fields(&kept[0]).contains(&("Needs your OK".into(), None, "Bash(awk:*)".into())), "the preview names it: {:?}", fields(&kept[0]));
    assert_eq!(pending::approve(&t.st, &kept[0].action.id).await.unwrap().status, "approved");
    assert_eq!(t.backend().allowed_tools, ["Bash(git status:*)", "Bash(awk:*)"]);
    assert!(t.pending(&thread).is_empty());
    let heard = pending::for_turn(&t.st, &thread);
    assert!(heard.line.contains("waited for their approval after a tool from outside Hersir:\n"), "{}", heard.line);
    pending::told(&t.st, &heard.ids);

    // a shell is kept there too (every call of update_agent is), and approving it is still refused: only the agent form adds one
    let bash = json!({"agent": "Backend Agent", "allowed_tools": ["Bash(git status:*)", "Bash(bash:*)"]});
    t.call(Some(&thread), "update_agent", bash.clone()).await.unwrap_err();
    let id = t.pending(&thread)[0].action.id.clone();
    let done = pending::approve(&t.st, &id).await.unwrap();
    assert_eq!(done.status, "failed");
    assert!(done.result_md.as_deref().is_some_and(|r| r.contains("Bash(bash:*) can't be given from chat")), "{:?}", done.result_md);
    assert_eq!(t.backend().allowed_tools, ["Bash(git status:*)", "Bash(awk:*)"], "nothing changed");
}

#[tokio::test]
async fn a_call_that_can_never_work_is_refused_at_once_and_a_shell_is_refused_not_kept() {
    let t = setup();
    let thread = t.thread("Change the Backend Agent");
    let e = t.call(Some(&thread), "update_agent", json!({"agent": "Backend Agent", "cards_at_once": 99, "allowed_tools": ["Bash(awk:*)"]})).await
        .unwrap_err();
    assert!(e.contains("between 1 and 10 cards at once") && !e.contains("wasn't run"), "{e}");
    let e = t.call(Some(&thread), "create_agent", json!({"name": "Late Agent", "role": "backend", "board_check_minutes": 2,
        "allowed_tools": ["Bash(awk:*)"]})).await.unwrap_err();
    assert!(e.contains("every 5 to 1440 minutes") && !e.contains("wasn't run"), "{e}");
    let e = t.call(Some(&thread), "update_agent", json!({"agent": "Backend Agent", "allowed_tools": ["Bash(awk:*)", "Bash(ls:*) Bash(*)"]})).await
        .unwrap_err();
    assert!(e.contains("isn't one rule"), "{e}");
    // GA-262's refusals still come first: a shell, an interpreter or a path is refused, not kept
    for bad in ["Bash(bash:*)", "Bash(python3:*)", "Bash(/usr/bin/awk:*)", "Bash(env:*)"] {
        let e = t.call(Some(&thread), "update_agent", json!({"agent": "Backend Agent", "allowed_tools": ["Bash(awk:*)", bad]})).await.unwrap_err();
        assert!(e.starts_with(&format!("{bad} can't be given from chat")), "{bad}: {e}");
        let e = t.call(Some(&thread), "create_agent", json!({"name": "Shell Agent", "role": "backend", "allowed_tools": [bad]})).await.unwrap_err();
        assert!(e.starts_with(&format!("{bad} can't be given from chat")), "{bad}: {e}");
    }
    assert!(t.pending(&thread).is_empty(), "nothing waits");
    assert!(t.agent("Late Agent").is_none() && t.agent("Shell Agent").is_none());
    assert_eq!(t.backend().allowed_tools, strs(&NARROW));
}

#[tokio::test]
async fn outside_a_chat_a_new_command_cant_wait_so_nothing_changes() {
    let t = setup();
    let e = t.call(None, "update_agent", json!({"agent": "Backend Agent", "title": "Builder", "allowed_tools": ["Bash(awk:*)"]})).await.unwrap_err();
    assert_eq!(e, "update_agent wasn't run: Bash(awk:*) is new for Backend Agent, and from chat an agent gets a command it doesn't have at once \
                   only when it is one of Hersir's default list that runs no other command and writes no file. Hersir couldn't keep the call for \
                   the user to approve (there is no chat to keep it in), so nothing changed. Tell the user, so they can add it themselves in the \
                   agent form (Permissions → Allowed commands).");
    let e = t.call(None, "create_agent", json!({"name": "New Agent", "role": "backend", "allowed_tools": ["Bash(awk:*)", "Bash(ssh:*)"]})).await
        .unwrap_err();
    assert!(e.starts_with("create_agent wasn't run: Bash(awk:*) and Bash(ssh:*) are new for New Agent,") && e.contains("add them themselves"), "{e}");
    let m = t.backend();
    assert_eq!((m.allowed_tools, m.title), (strs(&NARROW), None));
    assert!(t.agent("New Agent").is_none());
}

#[test]
fn the_tools_description_and_the_docs_name_the_same_entries_chat_may_add() {
    let names = listed(&CHAT_MAY_ADD);
    for tool in ["create_agent", "update_agent"] {
        let cat = tools::catalog();
        let d = cat.iter().find(|d| d.name == tool).unwrap();
        let field = d.input_schema["properties"]["allowed_tools"]["description"].as_str().unwrap();
        assert!(field.contains(&format!("A command the agent doesn't have yet waits for the user's Approve, the whole call with it, unless it is one \
                                         of {names}.")), "{tool}: {field}");
    }
    let docs = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../docs/agent-tools.md")).unwrap();
    let bullet = docs.lines().find(|l| l.starts_with("- **New commands wait for your Approve** (GA-323).")).expect("the docs' GA-323 bullet");
    let named = bullet.split("(`seed::CHAT_MAY_ADD`)").next().unwrap();
    let entries: Vec<&str> = named.split('`').skip(1).step_by(2).filter(|s| s.starts_with("Bash(")).collect();
    assert_eq!(entries, CHAT_MAY_ADD, "the docs list them in the same order");
    // and say what else waits, the card's examples among them
    for e in ["Bash(awk:*)", "Bash(find:*)", "Bash(ssh:*)", "Bash(make:*)", "Bash(node:*)", "Bash(npx:*)", "Bash(php:*)", "Bash(systemd-run:*)",
              "Bash(script:*)", "Bash(parallel:*)"] {
        assert!(bullet.contains(&format!("`{e}`")), "{e}");
    }
    assert!(bullet.contains("The agent form saves any entry, as before."));
}
