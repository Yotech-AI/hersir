//! GA-352 QA (GA-326 #3, Medium, CWE-269): from chat, create_agent and update_agent keep the whole call for the user's
//! Approve, as GA-323's new commands are, when it
//! - puts an agent on a CLI that doesn't keep it to its allowed commands (Codex, Gemini; an Other CLI stays refused from
//!   chat outright), or
//! - gives it a permission mode that isn't stricter than the one it runs with (for a new agent, or one moved to another
//!   kind of CLI: that CLI's usual one). Claude Code plan < manual < acceptEdits = dontAsk < auto; Codex read-only <
//!   workspace-write; Gemini plan < default < auto_edit (bypassPermissions, danger-full-access and yolo stay refused).
//!
//! The kept call's preview names each change, from and to. With no chat (a call Hersir makes itself, Claude Desktop) it is
//! refused with the agent-form sentence and nothing changes; a board check can't change agents at all. The agent form saves
//! any of these, as before; a move to a CLI that keeps to the list, or to a stricter mode, saves at once.
//! Tools are called the way the MCP server calls them, in a chat thread (`tools::call_in`).
use std::collections::HashMap;

use gizai_core::model::*;
use gizai_core::{chat, team};
use gizai_lib::{AppState, chat as app_chat, pending, tools};
use serde_json::{Value, json};

const FAKE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../crates/gizai-agents/tests/fake-claude.sh");

/// The sentence every kept or refused call of GA-352 carries, after what it does.
const RULE: &str = "From chat, an agent gets a CLI that doesn't keep it to its allowed commands, or a permission mode that isn't stricter \
                    than the one it has, only with the user's OK.";

/// What a call kept for its CLI or its mode returns, word for word: `what` names each change.
fn kept_for(tool: &str, what: &str) -> String {
    format!("{tool} wasn't run: {what}. {RULE} Hersir keeps the call for the user to approve with one click. Tell them in a sentence what \
             it does, and name each change, from and to.")
}

/// What such a call returns with no chat to keep it in: nothing changed, and the user makes it in the agent form (`places`).
fn refused_for(tool: &str, what: &str, places: &str) -> String {
    let change = if places.contains(" and ") { "these changes" } else { "the change" };
    format!("{tool} wasn't run: {what}. {RULE} Hersir couldn't keep the call for the user to approve (there is no chat to keep it in), so \
             nothing changed. Tell the user, so they can make {change} themselves in the agent form ({places}).")
}

/// The preview's fields: (label, from, value).
fn fields(p: &pending::PendingView) -> Vec<(String, Option<String>, String)> {
    let v = serde_json::to_value(p).unwrap();
    v["preview"]["fields"].as_array().unwrap().iter().map(|f| (f["label"].as_str().unwrap().to_string(),
        f["from"].as_str().map(str::to_string), f["value"].as_str().unwrap().to_string())).collect()
}

fn ok_field(value: &str) -> (String, Option<String>, String) {
    ("Needs your OK".into(), None, value.into())
}

fn change(label: &str, from: &str, to: &str) -> (String, Option<String>, String) {
    (label.into(), Some(from.into()), to.into())
}

struct T {
    st: AppState,
    lead: String,
    team_id: String,
    /// The CLIs added in Settings, their ids by name: Codex, Gemini, Claude 2 (a second Claude Code account) and Crush (Other).
    clis: HashMap<String, String>,
    _dir: tempfile::TempDir,
}

/// A Chief (Chat on), the CLIs above, and a Backend Agent made in the agent form on Claude Code with acceptEdits. The
/// queue is paused, so nothing starts by itself.
fn setup() -> T {
    let dir = tempfile::tempdir().unwrap();
    let st = gizai_lib::test_state(dir.path());
    gizai_core::settings::set(&st.db, "claude_bin", &FAKE.to_string()).unwrap();
    gizai_core::settings::set(&st.db, "agents_paused", &true).unwrap();
    let team_id = team::list(&st.db).unwrap()[0].id.clone();
    let lead = team::add_agent(&st.db, &st.you_id, &team_id, AgentInput {
        name: "Team Lead".into(), role_key: "lead".into(), chat_enabled: Some(true), ..Default::default() }).unwrap();
    let list: Vec<gizai_core::clis::Cli> = [("Codex", "codex"), ("Gemini", "gemini"), ("Claude 2", "claude_code"), ("Crush", "other")].iter()
        .map(|(name, kind)| gizai_core::clis::Cli { name: name.to_string(), kind: kind.to_string(), command: format!("/usr/bin/{kind}"),
            ..Default::default() }).collect();
    let clis = gizai_core::clis::save(&st.db, list).unwrap().into_iter().map(|c| (c.name, c.id)).collect();
    let t = T { st, lead, team_id, clis, _dir: dir };
    t.form_add("Backend Agent", "backend", "", "acceptEdits");
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
    fn cli(&self, name: &str) -> String {
        self.clis[name].clone()
    }
    /// The agent form's Add (`team::add_agent`, what the form's command calls), on the CLI `adapter` (empty: Claude Code).
    fn form_add(&self, name: &str, role: &str, adapter: &str, mode: &str) -> String {
        team::add_agent(&self.st.db, &self.st.you_id, &self.team_id, AgentInput { name: name.into(), role_key: role.into(),
            adapter: adapter.into(), permission_mode: mode.into(), ..Default::default() }).unwrap()
    }
    /// The agent form's Save (`team::update_agent`) of the CLI and the permission mode.
    fn form_save(&self, name: &str, adapter: &str, mode: &str) {
        let m = self.agent(name).unwrap();
        team::update_agent(&self.st.db, &self.st.you_id, &m.actor_id, AgentInput { name: m.name.clone(), role_key: m.role_key.clone(),
            adapter: adapter.into(), permission_mode: mode.into(), allowed_tools: m.allowed_tools.clone(), ..Default::default() }).unwrap();
    }
    fn agent(&self, name: &str) -> Option<team::Member> {
        team::all_agents(&self.st.db).unwrap().into_iter().map(|(_, m)| m).find(|m| m.name == name)
    }
    fn agents_called(&self, name: &str) -> usize {
        team::all_agents(&self.st.db).unwrap().into_iter().filter(|(_, m)| m.name == name).count()
    }
    /// The agent's CLI id (Claude Code's when none is saved) and the permission mode saved for it.
    fn runs(&self, name: &str) -> (String, String) {
        let m = self.agent(name).unwrap_or_else(|| panic!("{name}"));
        let cli = m.adapter.filter(|a| !a.is_empty()).unwrap_or_else(|| "claude_code".into());
        (cli, m.permission_mode.unwrap_or_default())
    }
    /// The calls waiting in the chat.
    fn pending(&self, thread: &str) -> Vec<pending::PendingView> {
        pending::list(&self.st, thread).unwrap().into_iter().filter(|p| p.action.status == "pending").collect()
    }
    /// Every call kept anywhere, waiting or decided.
    fn all_kept(&self) -> i64 {
        self.st.db.read(|c| Ok(c.query_row("SELECT COUNT(*) FROM pending_actions", [], |r| r.get(0))?)).unwrap()
    }
    /// The one call of `tool` waiting in the chat, kept by the Chief for what chat gives only with an OK, with exactly `args`.
    fn kept(&self, thread: &str, tool: &str, args: &Value) -> pending::PendingView {
        let all = self.pending(thread);
        let p = all.into_iter().rev().find(|p| p.action.tool == tool && &p.action.args == args).unwrap_or_else(|| panic!("{tool} {args} kept"));
        assert_eq!((p.action.outside_tool.as_str(), p.action.actor_id.as_str()), (gizai_core::pending::NEW_COMMANDS, self.lead.as_str()));
        p
    }
}

// ---- AC1: update_agent with runs_on Codex (or Gemini) for a Claude Code agent ----

#[tokio::test]
async fn moving_a_claude_code_agent_to_codex_or_gemini_from_chat_waits_and_approve_saves_it_once() {
    let t = setup();
    let thread = t.thread("Put the Backend Agent on Codex");
    assert_eq!(t.runs("Backend Agent"), ("claude_code".into(), "acceptEdits".into()));

    let to_codex = json!({"agent": "Backend Agent", "runs_on": "Codex", "title": "Builder"});
    let e = t.call(Some(&thread), "update_agent", to_codex.clone()).await.expect_err("update_agent waits");
    assert_eq!(e, "update_agent wasn't run: it moves Backend Agent from Claude Code to Codex, which doesn't keep an agent to its allowed \
                   commands. From chat, an agent gets a CLI that doesn't keep it to its allowed commands, or a permission mode that isn't \
                   stricter than the one it has, only with the user's OK. Hersir keeps the call for the user to approve with one click. Tell \
                   them in a sentence what it does, and name each change, from and to.");
    let m = t.agent("Backend Agent").unwrap();
    assert_eq!((t.runs("Backend Agent"), m.title), (("claude_code".into(), "acceptEdits".into()), None), "nothing changed, the title neither");
    // the preview names each change, from and to: the CLI, and the mode Codex starts it with
    let u = t.kept(&thread, "update_agent", &to_codex);
    let f = fields(&u);
    assert!(f.contains(&change("Runs on", "Claude Code", "Codex")), "{f:?}");
    assert!(f.contains(&change("Permission mode", "acceptEdits", "workspace-write")), "{f:?}");
    assert!(f.contains(&ok_field("Runs on: Claude Code → Codex, which doesn't keep the agent to its allowed commands")), "{f:?}");
    assert_eq!(f.iter().filter(|(l, _, _)| l == "Needs your OK").count(), 1, "only the CLI needs an OK: {f:?}");
    // the chat isn't marked by it: the call waited for the CLI, not for a tool from outside Hersir
    assert_eq!(app_chat::used_outside(&t.st, &thread), None);

    // Gemini, by its id, too
    let to_gemini = json!({"agent": "Backend Agent", "runs_on": t.cli("Gemini")});
    let e = t.call(Some(&thread), "update_agent", to_gemini.clone()).await.expect_err("update_agent waits");
    assert_eq!(e, kept_for("update_agent", "it moves Backend Agent from Claude Code to Gemini, which doesn't keep an agent to its allowed commands"));
    let g = t.kept(&thread, "update_agent", &to_gemini);
    let f = fields(&g);
    assert!(f.contains(&change("Runs on", "Claude Code", "Gemini")) && f.contains(&change("Permission mode", "acceptEdits", "auto_edit")), "{f:?}");
    assert!(f.contains(&ok_field("Runs on: Claude Code → Gemini, which doesn't keep the agent to its allowed commands")), "{f:?}");
    assert_eq!(t.pending(&thread).len(), 2);
    assert_eq!(t.runs("Backend Agent"), ("claude_code".into(), "acceptEdits".into()));

    // Approve saves the call as kept, once
    assert_eq!(pending::approve(&t.st, &u.action.id).await.unwrap().status, "approved");
    let m = t.agent("Backend Agent").unwrap();
    assert_eq!((t.runs("Backend Agent"), m.title.as_deref()), ((t.cli("Codex"), "workspace-write".into()), Some("Builder")));
    assert!(pending::approve(&t.st, &u.action.id).await.is_err(), "a second Approve runs nothing");
    // Discard changes nothing
    assert_eq!(pending::discard(&t.st, &g.action.id).unwrap().status, "discarded");
    assert_eq!(t.runs("Backend Agent"), (t.cli("Codex"), "workspace-write".into()));
    assert!(t.pending(&thread).is_empty());
}

// ---- AC2: permission_mode from acceptEdits to dontAsk and to auto waits, to plan saves at once ----

#[tokio::test]
async fn from_chat_dontask_and_auto_wait_and_plan_saves_at_once() {
    let t = setup();
    let thread = t.thread("Change the Backend Agent's permission mode");
    for to in ["dontAsk", "auto"] {
        let args = json!({"agent": "Backend Agent", "permission_mode": to});
        let e = t.call(Some(&thread), "update_agent", args.clone()).await.expect_err(to);
        assert_eq!(e, kept_for("update_agent", &format!("it changes Backend Agent's permission mode from acceptEdits to {to}")));
        let f = fields(&t.kept(&thread, "update_agent", &args));
        assert!(f.contains(&change("Permission mode", "acceptEdits", to)), "{to}: {f:?}");
        assert!(f.contains(&ok_field(&format!("Permission mode: acceptEdits → {to}"))), "{to}: {f:?}");
        assert_eq!(t.runs("Backend Agent").1, "acceptEdits", "{to}: nothing changed");
    }
    assert_eq!(t.pending(&thread).len(), 2);

    // a stricter one saves at once
    t.ok(Some(&thread), "update_agent", json!({"agent": "Backend Agent", "permission_mode": "plan"})).await;
    assert_eq!(t.runs("Backend Agent").1, "plan");
    // the mode it has, given again, is no change
    t.ok(Some(&thread), "update_agent", json!({"agent": "Backend Agent", "permission_mode": "plan", "title": "Planner"})).await;
    assert_eq!(t.agent("Backend Agent").unwrap().title.as_deref(), Some("Planner"));
    // from plan, manual and acceptEdits are looser: they wait
    for (to, args) in [("manual", json!({"agent": "Backend Agent", "permission_mode": "manual"})),
                       ("acceptEdits", json!({"agent": "Backend Agent", "permission_mode": "acceptEdits"}))] {
        let e = t.call(Some(&thread), "update_agent", args).await.unwrap_err();
        assert_eq!(e, kept_for("update_agent", &format!("it changes Backend Agent's permission mode from plan to {to}")));
    }
    assert_eq!(t.runs("Backend Agent").1, "plan");

    // an agent the user put on auto in the agent form: chat takes it down at once
    t.form_save("Backend Agent", "", "auto");
    for to in ["acceptEdits", "manual"] {
        t.form_save("Backend Agent", "", "auto");
        t.ok(Some(&thread), "update_agent", json!({"agent": "Backend Agent", "permission_mode": to})).await;
        assert_eq!(t.runs("Backend Agent").1, to);
    }
    // bypassPermissions is still refused, not kept
    let before = t.pending(&thread).len();
    let e = t.call(Some(&thread), "update_agent", json!({"agent": "Backend Agent", "permission_mode": "bypassPermissions"})).await.unwrap_err();
    assert!(e.contains("bypassPermissions") && !e.contains("wasn't run"), "{e}");
    assert_eq!(t.pending(&thread).len(), before);
}

#[tokio::test]
async fn codex_and_gemini_modes_rank_too_and_a_mode_the_cli_doesnt_have_is_refused_at_once() {
    let t = setup();
    let thread = t.thread("Tune the Codex and Gemini agents");
    t.form_add("Codex Agent", "backend", &t.cli("Codex"), "workspace-write");
    t.form_add("Gemini Agent", "qa", &t.cli("Gemini"), "auto_edit");

    // Codex: read-only is stricter; back to workspace-write waits
    t.ok(Some(&thread), "update_agent", json!({"agent": "Codex Agent", "permission_mode": "read-only"})).await;
    assert_eq!(t.runs("Codex Agent").1, "read-only");
    let args = json!({"agent": "Codex Agent", "permission_mode": "workspace-write"});
    let e = t.call(Some(&thread), "update_agent", args.clone()).await.unwrap_err();
    assert_eq!(e, kept_for("update_agent", "it changes Codex Agent's permission mode from read-only to workspace-write"));
    assert!(fields(&t.kept(&thread, "update_agent", &args)).contains(&ok_field("Permission mode: read-only → workspace-write")));
    assert_eq!(t.runs("Codex Agent").1, "read-only");
    // an update of something else leaves a Codex agent on Codex at once: it is no move
    t.ok(Some(&thread), "update_agent", json!({"agent": "Codex Agent", "runs_on": "Codex", "title": "Codex builder"})).await;
    assert_eq!(t.agent("Codex Agent").unwrap().title.as_deref(), Some("Codex builder"));

    // Gemini: default and plan are stricter than auto_edit; from plan, default waits
    t.ok(Some(&thread), "update_agent", json!({"agent": "Gemini Agent", "permission_mode": "default"})).await;
    t.ok(Some(&thread), "update_agent", json!({"agent": "Gemini Agent", "permission_mode": "plan"})).await;
    assert_eq!(t.runs("Gemini Agent").1, "plan");
    let e = t.call(Some(&thread), "update_agent", json!({"agent": "Gemini Agent", "permission_mode": "default"})).await.unwrap_err();
    assert_eq!(e, kept_for("update_agent", "it changes Gemini Agent's permission mode from plan to default"));
    assert_eq!(t.runs("Gemini Agent").1, "plan");

    // a mode its CLI doesn't have, and the modes that run anything, are refused at once, not kept
    let kept = t.pending(&thread).len();
    for (agent, mode) in [("Codex Agent", "acceptEdits"), ("Gemini Agent", "auto"), ("Codex Agent", "danger-full-access"), ("Gemini Agent", "yolo")] {
        let e = t.call(Some(&thread), "update_agent", json!({"agent": agent, "permission_mode": mode})).await.unwrap_err();
        assert!(e.contains(mode) && !e.contains("wasn't run"), "{agent} {mode}: {e}");
    }
    assert_eq!(t.pending(&thread).len(), kept);
    assert_eq!((t.runs("Codex Agent").1, t.runs("Gemini Agent").1), ("read-only".into(), "plan".into()));
}

// ---- AC3: create_agent with runs_on Codex, or with a loose mode ----

#[tokio::test]
async fn create_agent_from_chat_on_codex_or_gemini_or_with_a_loose_mode_waits_and_no_agent_is_made() {
    let t = setup();
    let thread = t.thread("Hire agents");
    let cases = [
        (json!({"name": "Codex Agent", "role": "backend", "runs_on": "Codex"}),
         "it puts Codex Agent on Codex, which doesn't keep an agent to its allowed commands",
         "Runs on: Codex, which doesn't keep the agent to its allowed commands"),
        // a stricter mode doesn't take the wait off a CLI that doesn't keep to the list
        (json!({"name": "Reader Agent", "role": "qa", "runs_on": "Codex", "permission_mode": "read-only"}),
         "it puts Reader Agent on Codex, which doesn't keep an agent to its allowed commands",
         "Runs on: Codex, which doesn't keep the agent to its allowed commands"),
        (json!({"name": "Gemini Agent", "role": "qa", "runs_on": "Gemini", "permission_mode": "plan"}),
         "it puts Gemini Agent on Gemini, which doesn't keep an agent to its allowed commands",
         "Runs on: Gemini, which doesn't keep the agent to its allowed commands"),
        (json!({"name": "Free Agent", "role": "backend", "permission_mode": "auto"}),
         "it gives Free Agent the permission mode auto, where the usual one is acceptEdits",
         "Permission mode: acceptEdits (the usual one) → auto"),
        (json!({"name": "Quiet Agent", "role": "backend", "permission_mode": "dontAsk"}),
         "it gives Quiet Agent the permission mode dontAsk, where the usual one is acceptEdits",
         "Permission mode: acceptEdits (the usual one) → dontAsk"),
        // a second Claude Code account keeps to the list, but auto is still looser than its usual mode
        (json!({"name": "Second Agent", "role": "backend", "runs_on": "Claude 2", "permission_mode": "auto"}),
         "it gives Second Agent the permission mode auto, where the usual one is acceptEdits",
         "Permission mode: acceptEdits (the usual one) → auto"),
    ];
    for (args, what, ok) in &cases {
        let name = args["name"].as_str().unwrap();
        let e = t.call(Some(&thread), "create_agent", args.clone()).await.expect_err(name);
        assert_eq!(&e, &kept_for("create_agent", what), "{name}");
        assert!(t.agent(name).is_none(), "{name}: no agent made");
        let f = fields(&t.kept(&thread, "create_agent", args));
        assert!(f.contains(&ok_field(ok)), "{name}: {f:?}");
        assert_eq!(f.iter().filter(|(l, _, _)| l == "Needs your OK").count(), 1, "{name}: {f:?}");
    }
    assert_eq!(t.pending(&thread).len(), cases.len());
    assert_eq!(app_chat::used_outside(&t.st, &thread), None);

    // Approve makes the agent as kept, once
    let codex = t.kept(&thread, "create_agent", &cases[0].0);
    assert_eq!(pending::approve(&t.st, &codex.action.id).await.unwrap().status, "approved");
    assert_eq!(t.runs("Codex Agent"), (t.cli("Codex"), "workspace-write".into()));
    assert!(pending::approve(&t.st, &codex.action.id).await.is_err(), "a second Approve runs nothing");
    let free = t.kept(&thread, "create_agent", &cases[3].0);
    assert_eq!(pending::approve(&t.st, &free.action.id).await.unwrap().status, "approved");
    assert_eq!(t.runs("Free Agent"), ("claude_code".into(), "auto".into()));
    assert_eq!((t.agents_called("Codex Agent"), t.agents_called("Free Agent")), (1, 1));
    assert_eq!(t.pending(&thread).len(), cases.len() - 2);
}

#[tokio::test]
async fn create_agent_from_chat_on_claude_code_with_its_usual_or_a_stricter_mode_is_made_at_once() {
    let t = setup();
    let thread = t.thread("Hire agents");
    for (name, args, want) in [
        ("Plain Agent", json!({}), ("claude_code".to_string(), "acceptEdits")),
        ("Edit Agent", json!({"permission_mode": "acceptEdits"}), ("claude_code".to_string(), "acceptEdits")),
        ("Plan Agent", json!({"permission_mode": "plan"}), ("claude_code".to_string(), "plan")),
        ("Manual Agent", json!({"permission_mode": "manual"}), ("claude_code".to_string(), "manual")),
        ("Second Agent", json!({"runs_on": "Claude 2"}), (t.cli("Claude 2"), "acceptEdits")),
        ("Second Plan Agent", json!({"runs_on": "Claude 2", "permission_mode": "plan"}), (t.cli("Claude 2"), "plan")),
    ] {
        let mut a = json!({"name": name, "role": "backend"});
        a.as_object_mut().unwrap().extend(args.as_object().unwrap().clone());
        let r = t.ok(Some(&thread), "create_agent", a).await;
        assert_eq!(r["ok"], true, "{name}: {r}");
        assert_eq!(t.runs(name), (want.0, want.1.to_string()), "{name}");
    }
    assert!(t.pending(&thread).is_empty());
    // an Other CLI is still refused from chat outright, not kept
    let e = t.call(Some(&thread), "create_agent", json!({"name": "Crush Agent", "role": "backend", "runs_on": "Crush"})).await.unwrap_err();
    assert!(e.contains("Crush runs with its own permissions") && !e.contains("wasn't run"), "{e}");
    let e = t.call(Some(&thread), "update_agent", json!({"agent": "Backend Agent", "runs_on": "Crush"})).await.unwrap_err();
    assert!(e.contains("Crush runs with its own permissions"), "{e}");
    assert!(t.pending(&thread).is_empty() && t.agent("Crush Agent").is_none());
}

// ---- AC4: with no chat these calls are refused with the agent-form sentence ----

#[tokio::test]
async fn with_no_chat_these_calls_are_refused_with_the_agent_form_sentence_and_nothing_changes() {
    let t = setup();
    let e = t.call(None, "update_agent", json!({"agent": "Backend Agent", "runs_on": "Codex", "title": "Builder"})).await.unwrap_err();
    assert_eq!(e, "update_agent wasn't run: it moves Backend Agent from Claude Code to Codex, which doesn't keep an agent to its allowed \
                   commands. From chat, an agent gets a CLI that doesn't keep it to its allowed commands, or a permission mode that isn't \
                   stricter than the one it has, only with the user's OK. Hersir couldn't keep the call for the user to approve (there is no \
                   chat to keep it in), so nothing changed. Tell the user, so they can make the change themselves in the agent form (Agent → \
                   Runs on).");
    for to in ["dontAsk", "auto"] {
        let e = t.call(None, "update_agent", json!({"agent": "Backend Agent", "permission_mode": to})).await.unwrap_err();
        assert_eq!(e, refused_for("update_agent", &format!("it changes Backend Agent's permission mode from acceptEdits to {to}"),
                                  "Permissions → Permission mode"));
    }
    let e = t.call(None, "create_agent", json!({"name": "Gemini Agent", "role": "qa", "runs_on": "Gemini"})).await.unwrap_err();
    assert_eq!(e, refused_for("create_agent", "it puts Gemini Agent on Gemini, which doesn't keep an agent to its allowed commands", "Agent → Runs on"));
    let e = t.call(None, "create_agent", json!({"name": "Free Agent", "role": "backend", "permission_mode": "auto"})).await.unwrap_err();
    assert_eq!(e, refused_for("create_agent", "it gives Free Agent the permission mode auto, where the usual one is acceptEdits",
                              "Permissions → Permission mode"));
    // a CLI and a new command in one call: both named, and both places in the agent form
    let e = t.call(None, "update_agent", json!({"agent": "Backend Agent", "runs_on": "Codex", "allowed_tools": ["Bash(awk:*)"]})).await.unwrap_err();
    assert!(e.starts_with("update_agent wasn't run: it moves Backend Agent from Claude Code to Codex, which doesn't keep an agent to its allowed \
                           commands; Bash(awk:*) is new for Backend Agent. "), "{e}");
    assert!(e.ends_with("so they can make these changes themselves in the agent form (Agent → Runs on and Permissions → Allowed commands)."), "{e}");

    // a board check can't change agents at all
    let check = gizai_core::board::create_run(&t.st.db, &t.lead, "S", "/tmp", "/tmp/c.jsonl", &[]).unwrap();
    for (tool, args) in [("update_agent", json!({"agent": "Backend Agent", "runs_on": "Codex"})),
                         ("create_agent", json!({"name": "Free Agent", "role": "backend", "permission_mode": "auto"}))] {
        let e = tools::call_check(&t.st, &t.lead, &check, tool, args).await.unwrap_err();
        assert!(e.starts_with(&format!("{tool} can't be used in a board check")), "{e}");
    }

    let m = t.agent("Backend Agent").unwrap();
    assert_eq!((t.runs("Backend Agent"), m.title, m.allowed_tools.iter().any(|e| e == "Bash(awk:*)")),
               (("claude_code".into(), "acceptEdits".into()), None, false), "nothing changed");
    assert!(t.agent("Gemini Agent").is_none() && t.agent("Free Agent").is_none(), "no agent made");
    assert_eq!(t.all_kept(), 0, "nothing kept");
}

// ---- AC5 and the card's point 4: the agent form, and moves that keep to the list ----

#[tokio::test]
async fn the_agent_form_saves_any_cli_and_mode_and_chat_then_leaves_them_as_they_are() {
    let t = setup();
    // Add and Save, as the form's commands call them
    t.form_add("Codex Form", "backend", &t.cli("Codex"), "workspace-write");
    t.form_add("Gemini Form", "qa", &t.cli("Gemini"), "");
    t.form_add("Auto Form", "backend", "", "auto");
    t.form_add("Crush Form", "backend", &t.cli("Crush"), "");
    t.form_save("Backend Agent", &t.cli("Codex"), "read-only");
    assert_eq!(t.runs("Codex Form"), (t.cli("Codex"), "workspace-write".into()));
    assert_eq!(t.runs("Gemini Form"), (t.cli("Gemini"), "auto_edit".into()));
    assert_eq!(t.runs("Auto Form"), ("claude_code".into(), "auto".into()));
    assert_eq!(t.runs("Crush Form").0, t.cli("Crush"));
    assert_eq!(t.runs("Backend Agent"), (t.cli("Codex"), "read-only".into()));
    t.form_save("Auto Form", "", "dontAsk");
    assert_eq!(t.runs("Auto Form").1, "dontAsk");
    t.form_save("Auto Form", "", "auto");

    // from chat, a call that changes neither saves at once
    let thread = t.thread("Rename a few");
    for (agent, extra) in [("Codex Form", json!({})), ("Gemini Form", json!({"runs_on": "Gemini"})), ("Auto Form", json!({"permission_mode": "auto"})),
                           ("Crush Form", json!({}))] {
        let mut a = json!({"agent": agent, "title": format!("{agent} renamed")});
        a.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        t.ok(Some(&thread), "update_agent", a).await;
        assert_eq!(t.agent(agent).unwrap().title, Some(format!("{agent} renamed")));
    }
    assert_eq!(t.runs("Auto Form").1, "auto");
    assert!(t.pending(&thread).is_empty());

    // a move to a CLI that keeps to the list, with its usual or a stricter mode, saves at once
    t.ok(Some(&thread), "update_agent", json!({"agent": "Codex Form", "runs_on": "Claude Code"})).await;
    assert_eq!(t.runs("Codex Form"), ("claude_code".into(), "acceptEdits".into()));
    t.ok(Some(&thread), "update_agent", json!({"agent": "Gemini Form", "runs_on": "Claude 2", "permission_mode": "plan"})).await;
    assert_eq!(t.runs("Gemini Form"), (t.cli("Claude 2"), "plan".into()));
    t.ok(Some(&thread), "update_agent", json!({"agent": "Auto Form", "runs_on": "Claude 2"})).await;
    assert_eq!(t.runs("Auto Form"), (t.cli("Claude 2"), "auto".into()), "another Claude Code account keeps its mode");
    assert!(t.pending(&thread).is_empty());

    // but a move to Claude Code with a looser mode than its usual one waits
    let args = json!({"agent": "Backend Agent", "runs_on": "Claude Code", "permission_mode": "auto"});
    let e = t.call(Some(&thread), "update_agent", args.clone()).await.unwrap_err();
    assert!(e.starts_with("update_agent wasn't run: it changes Backend Agent's permission mode from read-only to auto. "), "{e}");
    assert!(!e.contains("moves Backend Agent"), "Claude Code keeps to the list: {e}");
    t.kept(&thread, "update_agent", &args);
    // and a move between Gemini and Codex waits too
    t.form_add("Gemini Too", "qa", &t.cli("Gemini"), "plan");
    let e = t.call(Some(&thread), "update_agent", json!({"agent": "Gemini Too", "runs_on": "Codex", "permission_mode": "read-only"})).await
        .unwrap_err();
    assert_eq!(e, kept_for("update_agent", "it moves Gemini Too from Gemini to Codex, which doesn't keep an agent to its allowed commands"));
    assert_eq!(t.runs("Backend Agent"), (t.cli("Codex"), "read-only".into()));
    assert_eq!(t.runs("Gemini Too"), (t.cli("Gemini"), "plan".into()));
}

// ---- With GA-323 and GA-261 ----

#[tokio::test]
async fn a_cli_a_mode_and_a_new_command_in_one_call_are_each_named_and_wait_once() {
    let t = setup();
    let thread = t.thread("Move it to Codex with awk");
    let args = json!({"agent": "Backend Agent", "runs_on": "Codex", "allowed_tools": ["Bash(git status:*)", "Bash(awk:*)"]});
    let e = t.call(Some(&thread), "update_agent", args.clone()).await.unwrap_err();
    assert_eq!(e, "update_agent wasn't run: it moves Backend Agent from Claude Code to Codex, which doesn't keep an agent to its allowed \
                   commands; Bash(awk:*) is new for Backend Agent. From chat, an agent gets a CLI that doesn't keep it to its allowed commands, \
                   or a permission mode that isn't stricter than the one it has, only with the user's OK, and a command it doesn't have at once \
                   only when it is one of Hersir's default list that runs no other command and writes no file. Hersir keeps the call for the \
                   user to approve with one click. Tell them in a sentence what it does, and name each change, from and to, and Bash(awk:*).");
    let f = fields(&t.kept(&thread, "update_agent", &args));
    assert!(f.contains(&ok_field("Runs on: Claude Code → Codex, which doesn't keep the agent to its allowed commands")), "{f:?}");
    assert!(f.contains(&ok_field("Bash(awk:*)")), "{f:?}");

    let args = json!({"agent": "Backend Agent", "permission_mode": "auto", "allowed_tools": ["Bash(git status:*)", "Bash(find:*)"]});
    let e = t.call(Some(&thread), "update_agent", args.clone()).await.unwrap_err();
    assert!(e.starts_with("update_agent wasn't run: it changes Backend Agent's permission mode from acceptEdits to auto; Bash(find:*) is new \
                           for Backend Agent. "), "{e}");
    let f = fields(&t.kept(&thread, "update_agent", &args));
    assert!(f.contains(&ok_field("Permission mode: acceptEdits → auto")) && f.contains(&ok_field("Bash(find:*)")), "{f:?}");
    assert_eq!(t.pending(&thread).len(), 2, "each call waits once");
    assert_eq!(t.runs("Backend Agent"), ("claude_code".into(), "acceptEdits".into()));

    // new commands alone still read as GA-323's sentence, word for word
    let e = t.call(Some(&thread), "update_agent", json!({"agent": "Backend Agent", "allowed_tools": ["Bash(awk:*)"]})).await.unwrap_err();
    assert!(e.starts_with("update_agent wasn't run: Bash(awk:*) is new for Backend Agent, and from chat an agent gets a command"), "{e}");
}

#[tokio::test]
async fn after_a_tool_from_outside_the_move_is_kept_once_and_approve_saves_it() {
    let t = setup();
    let thread = t.thread("Read Otus, then move the agent");
    app_chat::mark_outside(&t.st, &thread, "mcp__otus__search");
    let args = json!({"agent": "Backend Agent", "runs_on": "Codex"});
    let e = t.call(Some(&thread), "update_agent", args).await.unwrap_err();
    assert!(e.starts_with("update_agent wasn't run: this chat used mcp__otus__search, a tool from outside Hersir."), "{e}");
    let kept = t.pending(&thread);
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].action.outside_tool, "mcp__otus__search");
    assert!(fields(&kept[0]).contains(&ok_field("Runs on: Claude Code → Codex, which doesn't keep the agent to its allowed commands")),
            "the preview names it: {:?}", fields(&kept[0]));
    // approving it saves the move, with no second wait
    assert_eq!(pending::approve(&t.st, &kept[0].action.id).await.unwrap().status, "approved");
    assert_eq!(t.runs("Backend Agent"), (t.cli("Codex"), "workspace-write".into()));
    assert!(t.pending(&thread).is_empty());
}

#[test]
fn the_tools_description_and_the_docs_say_what_waits() {
    let cat = tools::catalog();
    for tool in ["create_agent", "update_agent"] {
        let d = cat.iter().find(|d| d.name == tool).unwrap();
        let runs_on = d.input_schema["properties"]["runs_on"]["description"].as_str().unwrap();
        assert!(runs_on.contains("Codex and Gemini don't keep an agent to its allowed commands: putting an agent on one waits for the user's Approve"),
                "{tool}: {runs_on}");
        let mode = d.input_schema["properties"]["permission_mode"]["description"].as_str().unwrap();
        assert!(mode.contains("Strict to loose: Claude Code plan, manual, acceptEdits and dontAsk, auto; Codex read-only, workspace-write; Gemini \
                               plan, default, auto_edit."), "{tool}: {mode}");
    }
    let docs = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../docs/agent-tools.md")).unwrap();
    assert!(docs.contains("GA-352"), "docs/agent-tools.md names GA-352's rule");
}
