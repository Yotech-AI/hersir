//! Tools that change things. Each one returns `{ok, …, link}` so the chat can show a card that opens the item,
//! and tells open screens what changed. Updates merge: only the fields given change.
use gizai_core::model::*;
use gizai_core::{clients, comments, docs, files, projects, tasks, team, users};
use serde_json::{Value, json};

use super::{Args, Cx, err, resolve, short};

fn link(page: &str, id: &str, label: &str) -> Value {
    json!({"page": page, "id": id, "label": label})
}

fn task_json(t: &Task) -> Value {
    json!({"id": t.id, "identifier": t.identifier, "title": t.title, "column": t.state_name, "assignee": t.assignee_name,
           "labels": t.labels.iter().map(|l| l.name.clone()).collect::<Vec<_>>(), "hold": t.hold, "testing": t.testing})
}

/// The result of a change to a card. `held_after`: the tool from outside Gizai after which the change put the card on
/// hold (GA-101); the result says so, for the Chief to tell the user.
fn task_done(cx: &Cx, id: &str, what: &str, held_after: Option<&str>) -> Result<Value, String> {
    let t = tasks::get(cx.db(), id).map_err(err)?;
    cx.changed("tasks");
    cx.wake(&t.id);
    let mut task = task_json(&t);
    // GA-228: the cards it builds on, and those it waits for (not merged yet): it starts by itself once they are.
    let on = gizai_core::builds_on::of(cx.db(), &t.id).map(|b| b.builds_on).unwrap_or_default();
    if !on.is_empty() {
        task["builds_on"] = json!(on.iter().map(|l| l.identifier.clone()).collect::<Vec<_>>());
    }
    if !t.waits_for.is_empty() {
        task["waits_for"] = json!(t.waits_for);
    }
    let mut v = json!({"ok": true, "done": what, "task": task, "link": link("task", &t.id, &format!("{} {}", t.identifier, short(&t.title, 60)))});
    // GA-312: the Chat page lists the card under the answer with an OK, so the user needn't open it; `hold_at`, when this
    // call put it on that hold, so it shows there only while the card is on this hold, not on one another chat put on later.
    if let Some(tool) = held_after {
        v["task"]["hold_at"] = json!(t.hold_at);
        v["on_hold"] = json!(format!("{} waits on hold for the user's OK, so no agent starts it: you used {tool}, a tool from outside \
            Hersir, in this chat. Tell the user they can OK it under your answer, or take it off hold on the card; don't try to take \
            the hold off yourself.", t.identifier));
    }
    Ok(v)
}

/// GA-101: why a card waits on hold `needs_decision` when the Chief put it where an agent would start it, or changed it
/// there, after using `tool`, a tool from outside Gizai.
fn waiting_for_ok(tool: &str) -> String {
    format!("Waiting for your OK: the Chief put this card here after using {tool}, a tool from outside Hersir. Check it, then take it off hold.")
}

/// GA-101's hold, `needs_decision` with `reason` (`waiting_for_ok`), of its own kind (GA-261): only the user takes it off.
fn waiting_hold(reason: &str) -> tasks::NewHold<'_> {
    tasks::NewHold { hold: "needs_decision", reason, kind: Some(HOLD_WAITING_FOR_OK) }
}

/// An Auto column: its agents pick its cards up by themselves (`workflow::waiting_for`).
fn auto(col: &team::WorkflowState) -> bool {
    col.auto && gizai_core::columns::takes_agents(&col.category)
}

/// The card is in an Auto column.
fn in_auto(cx: &Cx, t: &Task) -> Result<bool, String> {
    let project = t.project_id.as_ref().and_then(|p| projects::get(cx.db(), p).ok());
    Ok(resolve::team_of(cx, project.as_ref())?.states.iter().any(|s| s.id == t.state_id && auto(s)))
}

/// The patch gives `new` and it isn't `now` (trimmed; None is empty).
fn differs(new: &Option<String>, now: Option<&str>) -> bool {
    new.as_deref().is_some_and(|n| n.trim() != now.unwrap_or_default().trim())
}

/// GA-101: the patch changes what the card's agent is told (its description or acceptance criteria) or who starts it (its
/// assignee). So does a new Builds on (GA-228, `builds_on`: card ids): its run prompt names those cards, and taking one
/// off can start it. GA-319: so does a new title, its run prompt's `# Task` line. GA-261: such a change outside an Auto
/// column waits for the user's Approve after a tool from outside Gizai in the chat.
fn steers(cx: &Cx, t: &Task, p: &TaskPatch, builds_on: Option<&[String]>) -> bool {
    let new_builds_on = builds_on.is_some_and(|new| {
        let now: Vec<String> = gizai_core::builds_on::of(cx.db(), &t.id).map(|b| b.builds_on).unwrap_or_default().into_iter().map(|l| l.id).collect();
        new.len() != now.len() || new.iter().any(|id| !now.contains(id))
    });
    differs(&p.title, Some(&t.title)) || differs(&p.description_md, Some(&t.description_md)) || differs(&p.acceptance_md, t.acceptance_md.as_deref())
        || differs(&p.assignee_id, t.assignee_id.as_deref()) || new_builds_on
}

/// GA-319: the patch gives the card's hold a new reason (or takes its reason off). A board check reads it (check_board), and
/// so does a chat the Chief starts about the card, so after a tool from outside Gizai in the chat the call waits for the
/// user's Approve, in any column: GA-101's hold would put its own reason in its place.
fn rewords_hold(t: &Task, p: &TaskPatch) -> bool {
    differs(&p.hold_reason, t.hold_reason.as_deref())
}

/// GA-319: a card's title as a tool gives it, checked as the write checks it (`tasks::check_title`: one line, at most 200
/// characters), so a call that can never work is refused at once, before it waits for a stream or the user's Approve.
fn title_arg(a: &Args) -> Result<Option<String>, String> {
    a.opt("title").map(|t| tasks::check_title(&t).map_err(err)).transpose()
}

/// GA-228: `builds_on`, the cards the card builds on by identifier, as their ids (a card named twice counts once); None
/// when not given. An unknown card is refused here; the rest (the card itself, more than 20, a loop) in the write that
/// makes the change (`builds_on::set_in`), so a refusal changes nothing.
fn builds_on_arg(cx: &Cx, a: &Args) -> Result<Option<Vec<String>>, String> {
    let Some(refs) = a.list("builds_on") else { return Ok(None) };
    let mut ids: Vec<String> = vec![];
    for id in resolve::task_ids(cx, &refs)? {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    Ok(Some(ids))
}

/// For an update: the given value (empty clears), else the current one.
fn keep(a: &Args, k: &str, current: &Option<String>) -> Option<String> {
    match a.text(k) {
        Some(v) if v.is_empty() => None,
        Some(v) => Some(v),
        None => current.clone(),
    }
}

/// The argument a project's repository link is in: `repository`, or `github`, its old name.
fn repository_arg(a: &Args) -> &'static str {
    if a.text("repository").is_some() { "repository" } else { "github" }
}

// ---- guards: what the chat may not do, whatever it is asked ----

/// The Chief reads text other people and agents wrote, so it can't hand an agent unlimited powers: no
/// bypassPermissions, no "any command" (bare Bash). People set those in the agent form.
fn guard_agent_powers(a: &Args) -> Result<(), String> {
    // Claude Code's bypassPermissions, Codex without its sandbox, Gemini's yolo: each lets an agent run anything.
    if let Some(m @ ("bypassPermissions" | "danger-full-access" | "yolo")) = a.opt("permission_mode").as_deref() {
        return Err(format!("{m} can't be set from chat: it lets an agent run anything. Set it yourself in the agent form if you really want it."));
    }
    let allowed = a.list("allowed_tools").unwrap_or_default();
    // GA-262: each entry one rule, as the agent form saves them: Claude Code would read `Bash(ls:*) Bash(*)` or `Bash(a) b`
    // as two. One without a `)`, like `Bash(git status:*`, is no command at all, and the check below says so.
    if let Some(t) = allowed.iter().find(|t| t.contains(')') && !team::one_rule(t)) {
        return Err(team::not_one_rule(t));
    }
    for t in &allowed {
        let t = t.trim().to_lowercase().replace(' ', "");
        if t == "bash" || t == "bash(*)" || t == "bash(:*)" || t == "bash(*:*)" {
            return Err("an agent can't be allowed to run any command from chat; name the commands, like Bash(npm test:*) or Bash(git commit:*)".into());
        }
    }
    // Only commands: web search, fetching pages and the CLI's other tools are the user's switches (agent form → Tools), and a
    // rule for another tool, like Read(//…), could reach past the worktree.
    if let Some(t) = allowed.iter().find(|t| !is_command(t)) {
        return Err(format!("allowed_tools takes only commands, like Bash(npm test:*); {t} isn't one. Web search, fetching pages, the browser, \
                            MCP servers and the CLI's other tools can't be switched on from chat: only the user does that, in the agent form → \
                            Tools. Nothing changed."));
    }
    // MCP servers and their tools: only the user switches them, in the agent form → Tools. GA-271: so is After the Chief uses
    // it (`trusted`, Ask for my OK or Allow), for a server and the browser.
    for k in ["mcp_servers", "mcp", "tools", "mcp_tools", "trusted"] {
        if a.0.get(k).is_some_and(|v| !v.is_null()) {
            return Err("an agent's MCP servers and their tools can't be switched from chat: only the user does that, in the agent form → Tools".into());
        }
    }
    // Web search, fetching pages, the browser and the CLI's built-in tools (slash commands and skills among them): the
    // same, only the user (agent form → Tools). GA-271: Web's After the Chief uses it too (`web_trusted`).
    for k in ["web_search", "web_fetch", "fetch_domains", "web", "browser", "insecure_certs", "builtin_tools", "builtin", "cli_tools",
              "slash_commands", "slash_commands_and_skills", "skills", "web_trusted", "webTrusted"] {
        if a.0.get(k).is_some_and(|v| !v.is_null()) {
            return Err("an agent's web search, fetching pages, browser and built-in tools (slash commands and skills among them) can't be \
                        switched from chat: only the user does that, in the agent form → Tools. Nothing changed.".into());
        }
    }
    // The folders an agent may read or change: only the user sets them, in the agent form.
    if a.0.get("folders").is_some_and(|v| !v.is_null()) {
        return Err("an agent's folders can't be changed from chat: the user sets them in the agent form (Team page → the agent → Permissions → Folders). Nothing changed.".into());
    }
    Ok(())
}

/// Chief may merge (GA-86) is the user's switch, set in the app: create_project and update_project never set it.
fn guard_lead_may_merge(a: &Args) -> Result<(), String> {
    for k in ["lead_may_merge", "team_lead_may_merge", "leadMayMerge", "may_merge"] {
        if a.0.get(k).is_some_and(|v| !v.is_null()) {
            return Err("Chief may merge can't be switched from chat: only the user switches it, in the app (the project page → Edit). \
                        Nothing changed.".into());
        }
    }
    Ok(())
}

/// A command for an agent's allowed commands: `Bash(…)` with something in it.
fn is_command(t: &str) -> bool {
    t.trim().strip_prefix("Bash(").and_then(|r| r.strip_suffix(')')).is_some_and(|inner| !inner.trim().is_empty())
}

/// GA-262: commands that run any other command: shells, interpreters, and commands that run the command after them, or run
/// it as another user. The chat gives an agent none of these (`guard_commands`).
const RUNS_ANY_COMMAND: &[&str] = &[
    "bash", "sh", "zsh", "fish", "dash", "ksh", "csh", "tcsh", "pwsh", "powershell", "cmd", "busybox",
    "env", "sudo", "doas", "su", "pkexec", "run0", "runas", "eval", "exec", "command", "builtin", "source", ".",
    "xargs", "nohup", "timeout", "nice", "time", "setsid", "stdbuf", "watch",
    "python", "python3", "perl", "ruby", "deno", "bun", "osascript",
];

/// The command a `Bash(…)` entry lets an agent run: its first word, without the `:*` of a prefix rule (`Bash(npm test:*)`
/// → npm, `Bash(bash:*)` → bash).
fn command_word(entry: &str) -> Option<&str> {
    let inner = entry.trim().strip_prefix("Bash(")?.strip_suffix(')')?.trim();
    inner.strip_suffix(":*").unwrap_or(inner).split_whitespace().next()
}

/// GA-262: why the chat can't give an agent the command `entry`; None when it can. Its command (`command_word`) is one of
/// `RUNS_ANY_COMMAND`, a wildcard or a path. A name with quotes, `$`, `=` or the like could be one of those spelled
/// another way, so it counts too, and so do capitals, `.exe` and a version (`BASH`, `bash.exe`, `python3.12`).
fn runs_any_command(entry: &str) -> Option<String> {
    let word = command_word(entry)?;
    if word.contains(['/', '\\']) {
        return Some(format!("it runs {word} by its path"));
    }
    if word.contains(['*', '?']) {
        return Some(format!("{word} is a wildcard, which matches any command"));
    }
    if !word.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '+')) {
        return Some(format!("{word} isn't a plain command name"));
    }
    let name = word.to_ascii_lowercase();
    let name = name.strip_suffix(".exe").unwrap_or(&name);
    let unversioned = name.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.');
    (RUNS_ANY_COMMAND.contains(&name) || RUNS_ANY_COMMAND.contains(&unversioned)).then(|| format!("{word} can run any command"))
}

/// GA-262: the Chief reads text other people and agents wrote, so it gives an agent no shell or interpreter
/// (`runs_any_command`), unless the entry is in `kept`: the agent's own list or its role's default list. Only the user
/// adds one, in the agent form. GA-323: what passes here and isn't in the allow list (`new_commands`) waits for the user's
/// Approve.
fn guard_commands(a: &Args, kept: &[String]) -> Result<(), String> {
    for t in a.list("allowed_tools").unwrap_or_default() {
        if kept.iter().any(|k| k.trim() == t) {
            continue;
        }
        if let Some(why) = runs_any_command(&t) {
            return Err(format!("{t} can't be given from chat: {why}. Only the user can add it, in the agent form (Permissions → Allowed \
                                commands). Nothing changed."));
        }
    }
    Ok(())
}

/// GA-323: the chat's allow list. Of `given`, the commands an agent gets from chat only with the user's Approve: those not
/// in `has` (what the agent runs with now; nothing for a new agent) and not one of Gizai's default list that runs no other
/// command and writes no file (`seed::CHAT_MAY_ADD`). In the order given, once each; empty: the call saves at once.
pub(crate) fn new_commands(given: &[String], has: &[String]) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for t in given.iter().map(|t| t.trim()) {
        let allowed = has.iter().any(|h| h.trim() == t) || gizai_core::seed::CHAT_MAY_ADD.contains(&t);
        if !allowed && !out.iter().any(|o| o == t) {
            out.push(t.to_string());
        }
    }
    out
}

/// What an agent with this list of allowed commands runs with: its list, else (empty) Gizai's default list.
fn runs_with(list: &[String]) -> Vec<String> {
    if list.is_empty() { gizai_core::seed::DEFAULT_TOOLS.map(String::from).to_vec() } else { list.to_vec() }
}

/// GA-323: the commands a call of create_agent gives the new agent only with the user's Approve (`new_commands`). None
/// without allowed_tools or with an empty list: the new agent gets its role's own list (`seed::role_tools`), at once.
pub(crate) fn create_agent_new_commands(a: &Args) -> Vec<String> {
    a.list("allowed_tools").map(|l| new_commands(&l, &[])).unwrap_or_default()
}

/// GA-323: the commands a call of update_agent gives the agent `m` only with the user's Approve (`new_commands`): what
/// its allowed_tools (an empty list is Gizai's default list) has that `m` doesn't run with now. None without allowed_tools.
pub(crate) fn update_agent_new_commands(m: &team::Member, a: &Args) -> Vec<String> {
    a.list("allowed_tools").map(|l| new_commands(&runs_with(&l), &runs_with(&m.allowed_tools))).unwrap_or_default()
}

/// GA-352: whether a coding CLI of this kind keeps an agent to its allowed commands. Only Claude Code does: it gets them as
/// its `--allowedTools` (`gizai_agents::cli::task_exec`). Codex gets no list at all: approval_policy "never" and its sandbox
/// decide, with the network on in workspace-write. Gemini gets each entry only as the start of a command
/// (`cli::gemini_tool`: an exact `Bash(npm test)` lets it run any `npm test …`), next to its own settings and policies. An
/// Other CLI runs with its own permissions (from chat it can't be picked at all, `runs_on`). An unknown kind neither.
pub(crate) fn keeps_to_the_list(kind: &str) -> bool {
    kind == "claude_code"
}

/// GA-352: the permission modes of a CLI of `kind` in tiers, from strict to loose. The modes of one tier rank the same, so a
/// move between them isn't a stricter one either (`mode_needs_ok`). From what each CLI does with a mode
/// (`gizai_agents::cli`) and the agent form's words for it (`src/lib/clis.ts`):
/// - Claude Code: plan changes nothing; manual asks for every action, which a run can't answer, so only the allowed commands
///   run; acceptEdits edits files and runs the allowed commands; dontAsk refuses what isn't allowed (the agent form), but
///   GA-352's card ranks it with auto, so it stands level with acceptEdits and a move either way between the two waits; auto
///   lets Claude decide what to run, outside the list too; bypassPermissions runs anything (never from chat,
///   `guard_agent_powers`).
/// - Codex: read-only changes nothing; workspace-write writes the worktree and uses the network; danger-full-access runs
///   without a sandbox (never from chat).
/// - Gemini: plan reads only; default asks, which a run can't answer, so only the allowed commands run; auto_edit edits files
///   and runs the allowed commands; yolo runs anything (never from chat).
///
/// Empty for an Other CLI, which takes no mode, and for an unknown kind.
fn mode_tiers(kind: &str) -> &'static [&'static [&'static str]] {
    match kind {
        "claude_code" => &[&["plan"], &["manual"], &["acceptEdits", "dontAsk"], &["auto"], &["bypassPermissions"]],
        "codex" => &[&["read-only"], &["workspace-write"], &["danger-full-access"]],
        "gemini" => &[&["plan"], &["default"], &["auto_edit"], &["yolo"]],
        _ => &[],
    }
}

/// GA-352: how loose `mode` is on a CLI of `kind` (`mode_tiers`); None for a mode that CLI doesn't have.
fn mode_rank(kind: &str, mode: &str) -> Option<usize> {
    mode_tiers(kind).iter().position(|tier| tier.contains(&mode))
}

/// GA-352: the permission mode an agent on a CLI of `kind` runs with when `mode` is saved, as `team::add_agent` saves it:
/// `mode`, else (none or empty) the CLI's usual one, the first the agent form offers (`clis::permission_modes`: acceptEdits,
/// workspace-write, auto_edit). Empty for an Other CLI, which takes none.
pub(crate) fn mode_as_run(kind: &str, mode: Option<&str>) -> String {
    let modes = gizai_core::clis::permission_modes(kind);
    match (mode.map(str::trim).filter(|m| !m.is_empty()), modes.first()) {
        (_, None) => String::new(),
        (Some(m), Some(_)) => m.to_string(),
        (None, Some(usual)) => usual.to_string(),
    }
}

/// GA-352: whether chat gives an agent on a CLI of `kind` the permission mode `to` only with the user's Approve, coming from
/// `from` (both as it runs with them, `mode_as_run`): another mode that isn't a stricter one (`mode_tiers`). A mode Hersir
/// doesn't rank counts as one too, unless it is the one the agent has; but one the CLI doesn't have is refused at once, before
/// the call would wait (`team::check_agent`).
fn mode_needs_ok(kind: &str, from: &str, to: &str) -> bool {
    from != to && match (mode_rank(kind, from), mode_rank(kind, to)) {
        (Some(f), Some(t)) => t >= f,
        _ => true,
    }
}

/// GA-352: a setting a call of create_agent or update_agent changes: its value now (None: a new agent has none) and the
/// one the call gives, as the agent form names them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Change {
    pub from: Option<String>,
    pub to: String,
}

/// GA-323, GA-352: what a call of create_agent or update_agent gives an agent that chat gives only with the user's Approve;
/// empty: it saves at once. Otherwise the whole call waits for the Approve (`Cx::keep_for_ok`), or, with no chat to wait in
/// (Claude Desktop, a call Hersir makes itself), isn't run, and nothing changes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct NeedsOk {
    /// GA-323: commands it doesn't run with yet (`new_commands`).
    pub commands: Vec<String>,
    /// GA-352: the CLI it goes on, by name, when that CLI doesn't keep it to its allowed commands (`keeps_to_the_list`).
    pub cli: Option<Change>,
    /// GA-352: the permission mode it runs with now (for a new agent, or one on another kind of CLI, the one the call is
    /// weighed against: the CLI's usual one) and the new one, when that is another mode and not a stricter one
    /// (`mode_needs_ok`).
    pub mode: Option<Change>,
}

impl NeedsOk {
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty() && self.cli.is_none() && self.mode.is_none()
    }
}

/// GA-323, GA-352: what a call of create_agent gives its new agent only with the user's Approve: commands
/// (`create_agent_new_commands`); a CLI that doesn't keep it to its allowed commands (`cli`: the one runs_on names, None for
/// the built-in Claude Code); and a permission mode that isn't stricter than that CLI's usual one (`mode_needs_ok`).
pub(crate) fn create_agent_needs_ok(cx: &Cx, a: &Args, cli: Option<&gizai_core::clis::Cli>) -> NeedsOk {
    let cli = cli.cloned().unwrap_or_else(|| gizai_core::clis::builtin(cx.db()));
    let (usual, mode) = (mode_as_run(&cli.kind, None), mode_as_run(&cli.kind, a.opt("permission_mode").as_deref()));
    NeedsOk {
        commands: create_agent_new_commands(a),
        cli: (!keeps_to_the_list(&cli.kind)).then(|| Change { from: None, to: cli.name.clone() }),
        mode: mode_needs_ok(&cli.kind, &usual, &mode).then_some(Change { from: Some(usual), to: mode }),
    }
}

/// GA-352: the agent `m`'s CLI now (None when it is no longer in Settings), with its id as `m` has it (empty: Claude Code).
fn cli_now(cx: &Cx, m: &team::Member) -> (Option<gizai_core::clis::Cli>, String) {
    let id = m.adapter.as_deref().map(str::trim).filter(|s| !s.is_empty()).unwrap_or(gizai_core::clis::CLAUDE_CODE).to_string();
    (crate::clis::of_agent(cx.st, Some(&id)).ok(), id)
}

/// GA-352: what the agent `m` runs on and with after a call of update_agent: its CLI (`to`: the one runs_on names; None,
/// the one it has) and its permission mode, as `update_agent` saves it. On another kind of CLI the mode it has doesn't carry
/// over: the one given, else that CLI's usual one. None when its CLI is no longer in Settings and none is given (the call
/// can't be saved then).
pub(crate) fn update_agent_runs(cx: &Cx, m: &team::Member, a: &Args, to: Option<&gizai_core::clis::Cli>) -> Option<(gizai_core::clis::Cli, String)> {
    let (now, _) = cli_now(cx, m);
    let to = to.cloned().or_else(|| now.clone())?;
    let moved = now.as_ref().is_none_or(|n| n.kind != to.kind);
    let given = a.opt("permission_mode").or(if moved { None } else { m.permission_mode.clone() });
    let mode = mode_as_run(&to.kind, given.as_deref());
    Some((to, mode))
}

/// GA-323, GA-352: what a call of update_agent gives the agent `m` only with the user's Approve: commands
/// (`update_agent_new_commands`); another CLI (`to`: the one runs_on names) that doesn't keep it to its allowed commands; and
/// another permission mode, as it would run with it (`update_agent_runs`), that isn't stricter than the one it runs with now
/// (`mode_needs_ok`). On another kind of CLI the new mode is weighed against that CLI's usual one, the one it starts from.
pub(crate) fn update_agent_needs_ok(cx: &Cx, m: &team::Member, a: &Args, to: Option<&gizai_core::clis::Cli>) -> NeedsOk {
    let mut out = NeedsOk { commands: update_agent_new_commands(m, a), ..Default::default() };
    let Some((cli, mode)) = update_agent_runs(cx, m, a, to) else { return out };
    let (now, id_now) = cli_now(cx, m);
    let kind_now = now.as_ref().map(|c| c.kind.clone()).unwrap_or_default();
    if cli.id != id_now && !keeps_to_the_list(&cli.kind) {
        out.cli = Some(Change { from: Some(now.as_ref().map(|c| c.name.clone()).unwrap_or(id_now)), to: cli.name.clone() });
    }
    let mode_now = mode_as_run(&kind_now, m.permission_mode.as_deref());
    let against = if cli.kind == kind_now { mode_now.clone() } else { mode_as_run(&cli.kind, None) };
    if mode_needs_ok(&cli.kind, &against, &mode) {
        out.mode = Some(Change { from: Some(mode_now).filter(|m| !m.is_empty()), to: mode });
    }
    out
}

/// A repository path from chat must be a git repository (it has a .git folder or file), and never `/`, your
/// home folder or a folder above it: agents work there and the Chief may read it.
pub(crate) fn checked_repo(raw: &str) -> Result<String, String> {
    let bad = || format!("{raw} is not a git repository: give the folder that holds the project's .git");
    let p = std::path::Path::new(raw).canonicalize().map_err(|_| bad())?;
    let home = std::path::Path::new(&gizai_core::clis::home()).canonicalize().ok();
    if p.parent().is_none() || home.as_ref().is_some_and(|h| h.starts_with(&p)) || !p.join(".git").exists() {
        return Err(bad());
    }
    Ok(p.display().to_string())
}

/// The model and effort must be ones Claude Code offers (when its list can be read): a wrong name like
/// "opus 5.5" would only fail at the agent's next run.
async fn check_model(cx: &Cx<'_>, model: Option<&str>, effort: Option<&str>) -> Result<(), String> {
    let Ok(list) = crate::runs::models(cx.st, false).await else { return Ok(()) };
    let wanted = model.unwrap_or("default");
    let Some(m) = list.iter().find(|m| m.value.eq_ignore_ascii_case(wanted) || m.resolved_model.as_deref().is_some_and(|r| r.eq_ignore_ascii_case(wanted))) else {
        let names: Vec<String> = list.iter().map(|m| match &m.resolved_model { Some(r) => format!("{} ({r})", m.value), None => m.value.clone() }).collect();
        return Err(format!("Claude Code has no model called \"{wanted}\". Use one of: {}.", names.join(", ")));
    };
    if let Some(e) = effort {
        if !m.effort_levels.iter().any(|l| l == e) {
            return Err(if m.effort_levels.is_empty() {
                format!("{} takes no effort level; leave effort empty", m.display_name)
            } else {
                format!("{} takes effort {}, not {e}", m.display_name, m.effort_levels.join(", "))
            });
        }
    }
    Ok(())
}

// ---- clients ----

fn client_input(a: &Args, cur: Option<&Client>) -> Result<ClientInput, String> {
    let c = |k: &str, v: Option<&Option<String>>| match v { Some(cur) => keep(a, k, cur), None => a.opt(k) };
    Ok(ClientInput {
        name: a.opt("name").or_else(|| cur.map(|c| c.name.clone())).unwrap_or_default(),
        legal_name: c("legal_name", cur.map(|x| &x.legal_name)),
        kind: a.opt("kind").or_else(|| cur.map(|x| x.kind.clone())),
        vat_number: c("vat_number", cur.map(|x| &x.vat_number)),
        coc_number: c("coc_number", cur.map(|x| &x.coc_number)),
        iban: c("iban", cur.map(|x| &x.iban)),
        email: c("email", cur.map(|x| &x.email)),
        phone: c("phone", cur.map(|x| &x.phone)),
        website: c("website", cur.map(|x| &x.website)),
        street: c("street", cur.map(|x| &x.street)),
        postal_code: c("postal_code", cur.map(|x| &x.postal_code)),
        city: c("city", cur.map(|x| &x.city)),
        country: a.opt("country").or_else(|| cur.and_then(|x| x.country.clone())),
        payment_terms_days: a.int("payment_terms_days")?.or_else(|| cur.and_then(|x| x.payment_terms_days)),
        status: a.opt("status").or_else(|| cur.map(|x| x.status.clone())),
        notes_md: c("notes_md", cur.map(|x| &x.notes_md)),
    })
}

pub(crate) fn create_client(cx: &Cx, a: &Args) -> Result<Value, String> {
    a.req("name")?;
    let id = clients::create(cx.db(), cx.actor, client_input(a, None)?).map_err(err)?;
    cx.changed("clients");
    let c = clients::get(cx.db(), &id).map_err(err)?;
    Ok(json!({"ok": true, "client": {"id": c.id, "name": c.name}, "link": link("client", &c.id, &c.name)}))
}

pub(crate) fn update_client(cx: &Cx, a: &Args) -> Result<Value, String> {
    let cur = resolve::client(cx, &a.req("client")?)?;
    clients::update(cx.db(), cx.actor, &cur.id, client_input(a, Some(&cur))?).map_err(err)?;
    cx.changed("clients");
    let c = clients::get(cx.db(), &cur.id).map_err(err)?;
    Ok(json!({"ok": true, "client": {"id": c.id, "name": c.name, "city": c.city, "status": c.status}, "link": link("client", &c.id, &c.name)}))
}

pub(crate) fn save_contact(cx: &Cx, a: &Args) -> Result<Value, String> {
    let c = resolve::client(cx, &a.req("client")?)?;
    let existing = clients::contacts(cx.db(), &c.id).map_err(err)?;
    let cur = match a.opt("contact") {
        Some(r) => {
            let cands: Vec<resolve::Cand> = existing.iter().map(|k| resolve::Cand { id: k.id.clone(), names: vec![k.name.clone()], show: k.name.clone() }).collect();
            Some(existing[resolve::pick("contact", "contacts", "full name", &r, &cands)?].clone())
        }
        None => None,
    };
    let contact = Contact {
        id: cur.as_ref().map(|k| k.id.clone()).unwrap_or_default(),
        client_id: c.id.clone(),
        name: a.req("name")?,
        role: keep(a, "role", &cur.as_ref().and_then(|k| k.role.clone())),
        email: keep(a, "email", &cur.as_ref().and_then(|k| k.email.clone())),
        phone: keep(a, "phone", &cur.as_ref().and_then(|k| k.phone.clone())),
        is_primary: a.flag("is_primary").unwrap_or(cur.as_ref().is_some_and(|k| k.is_primary) || existing.is_empty()),
    };
    let id = clients::upsert_contact(cx.db(), cx.actor, contact).map_err(err)?;
    cx.changed("contacts");
    Ok(json!({"ok": true, "contact": {"id": id, "name": a.req("name")?}, "link": link("client", &c.id, &c.name)}))
}

// ---- projects ----

/// GA-319: a call of `create_project` gives one of `STEERS_PROJECT` (`repository` or `github` for the link), as it saves
/// them: not empty.
fn new_project_steers(a: &Args) -> bool {
    STEERS_PROJECT.into_iter().any(|k| a.opt(if k == "repository" { repository_arg(a) } else { k }).is_some())
}

pub(crate) async fn create_project(cx: &Cx<'_>, a: &Args) -> Result<Value, String> {
    guard_lead_may_merge(a)?;
    let name = a.req("name")?;
    let repo_path = a.opt("repo_path").map(|r| checked_repo(&r)).transpose()?;
    let key = match a.opt("key") {
        Some(k) => k.to_uppercase(),
        None => projects::unused_key(cx.db(), &name).map_err(err)?,
    };
    let client_id = match a.opt("client") { Some(c) => Some(resolve::client(cx, &c)?.id), None => None };
    // GA-225: the team whose board its cards go on; left out, the first team.
    let team_id = match a.opt("team") { Some(t) => Some(resolve::team(cx, &t)?.id), None => None };
    // GA-319: a repository, folder, main branch or goal steers the new project's agents as it does an existing project's
    // (`update_project`): after a tool from outside Gizai in this chat, the call waits for the user's Approve.
    if new_project_steers(a) && let Some(tool) = cx.outside("create_project", a).await? {
        return Err(cx.keep("create_project", a, &tool));
    }
    let id = projects::create(cx.db(), cx.actor, ProjectInput {
        client_id, name, key, status: a.opt("status"), goal_md: a.opt("goal_md"), repo_path,
        default_branch: a.opt("default_branch"), color: a.opt("color"), repo_url: a.opt(repository_arg(a)), team_id, ..Default::default()
    }).map_err(err)?;
    cx.changed("projects");
    let p = projects::get(cx.db(), &id).map_err(err)?;
    let label = format!("{} ({})", p.name, p.key);
    Ok(json!({"ok": true, "project": {"id": p.id, "key": p.key, "name": p.name, "number": p.number, "team": p.team_name},
              "link": link("project", &p.id, &label)}))
}

/// What `update_project` saves for the project `cur`, and the team it names: everything given is checked first (the folder
/// is a git repository, the client and the team exist, Test runs at once is in range), so a refusal changes nothing.
pub(crate) fn project_input(cx: &Cx, cur: &Project, a: &Args) -> Result<(ProjectInput, Option<team::Team>), String> {
    // GA-225: another team moves the project and all its cards there, as the project form does.
    let team = match a.opt("team") { Some(t) => Some(resolve::team(cx, &t)?), None => None };
    let repo_path = match a.text("repo_path") {
        Some(r) if r.is_empty() => None,
        Some(r) => Some(checked_repo(&r)?),
        None => cur.repo_path.clone(),
    };
    let client_id = match a.text("client") {
        Some(c) if c.is_empty() => None,
        Some(c) => Some(resolve::client(cx, &c)?.id),
        None => cur.client_id.clone(),
    };
    // GA-212: Test runs at once, 1 to 20; 0 (or an empty string) clears it, left out keeps it.
    let test_runs_at_once = match a.text("test_runs_at_once").as_deref() {
        Some("") => Some(0),
        _ => a.int("test_runs_at_once")?,
    };
    if let Some(n) = test_runs_at_once && n != 0 && !projects::TEST_RUNS_AT_ONCE.contains(&n) {
        return Err(format!("test_runs_at_once is a number from {} to {}, or 0 to clear it, not {n}. Nothing changed.",
                           projects::TEST_RUNS_AT_ONCE.start(), projects::TEST_RUNS_AT_ONCE.end()));
    }
    let input = ProjectInput {
        client_id, name: a.opt("name").unwrap_or_else(|| cur.name.clone()), key: cur.key.clone(),
        status: a.opt("status").or(Some(cur.status.clone())), goal_md: keep(a, "goal_md", &cur.goal_md),
        repo_path, default_branch: a.opt("default_branch").or(Some(cur.default_branch.clone())),
        color: keep(a, "color", &cur.color), budget_amount_minor: cur.budget_amount_minor, budget_hours: cur.budget_hours,
        repo_url: keep(a, repository_arg(a), &cur.repo_url), test_runs_at_once, team_id: team.as_ref().map(|t| t.id.clone()), ..Default::default()
    };
    Ok((input, team))
}

/// GA-261: what of a project steers its agents: new cards fetch, run and push the code of its repository (its link), its
/// folder and its main branch, and every agent's prompt carries its goal. After a tool from outside Gizai in the chat, a
/// call of `update_project` that changes one of them waits for the user's Approve; GA-319: so does a call of
/// `create_project` that gives one (`new_project_steers`).
pub(crate) const STEERS_PROJECT: [&str; 4] = ["repository", "repo_path", "default_branch", "goal_md"];

/// One thing a call of `update_project` changes: its argument (`repository` for `github` too), its label, and the value now
/// and the new one as the project page says them ("" for none; the goal whole).
pub(crate) struct ProjectChange {
    pub key: &'static str,
    pub label: &'static str,
    pub from: String,
    pub to: String,
}

/// What saving `input` (`project_input`) changes of the project `cur`, in the project form's order. A link in another form
/// of the same repository, or the same folder written another way, changes nothing.
pub(crate) fn project_changes(cx: &Cx, cur: &Project, input: &ProjectInput) -> Vec<ProjectChange> {
    let text = |v: Option<&str>| v.unwrap_or_default().trim().to_string();
    let client = |id: Option<&str>| id.and_then(|id| clients::get(cx.db(), id).ok()).map(|c| c.name).unwrap_or_default();
    // As the link is saved: a GitHub or Bitbucket link in its tidy form.
    let link = |v: Option<&str>| match gizai_core::repo_url::normalize(v.unwrap_or_default()) {
        Ok(l) => l.map(|l| l.url).unwrap_or_default(),
        Err(_) => text(v),
    };
    let runs_at_once = |n: Option<i64>| n.filter(|n| *n > 0).map(|n| n.to_string()).unwrap_or_default();
    let team_now = cur.team_name.clone().unwrap_or_default();
    let team_then = match input.team_id.as_deref().filter(|t| cur.team_id.as_deref() != Some(*t)) {
        Some(id) => team::get(cx.db(), id).map(|t| t.name).unwrap_or_else(|_| id.to_string()),
        None => team_now.clone(),
    };
    let all = [
        ("name", "Name", cur.name.clone(), input.name.trim().to_string()),
        ("client", "Client", client(cur.client_id.as_deref()), client(input.client_id.as_deref())),
        ("status", "Status", cur.status.clone(), text(input.status.as_deref())),
        ("color", "Colour", text(cur.color.as_deref()), text(input.color.as_deref())),
        ("goal_md", "Goal", text(cur.goal_md.as_deref()), text(input.goal_md.as_deref())),
        ("repo_path", "Folder", text(cur.repo_path.as_deref()), text(input.repo_path.as_deref())),
        ("default_branch", "Main branch", cur.default_branch.trim().to_string(), text(input.default_branch.as_deref())),
        ("repository", "Repository", link(cur.repo_url.as_deref()), link(input.repo_url.as_deref())),
        ("test_runs_at_once", "Test runs at once", runs_at_once(cur.test_runs_at_once.map(i64::from)),
         match input.test_runs_at_once { Some(n) => runs_at_once(Some(n)), None => runs_at_once(cur.test_runs_at_once.map(i64::from)) }),
        ("team", "Team", team_now, team_then),
    ];
    all.into_iter().filter(|(_, _, from, to)| from != to).map(|(key, label, from, to)| ProjectChange { key, label, from, to }).collect()
}

pub(crate) async fn update_project(cx: &Cx<'_>, a: &Args) -> Result<Value, String> {
    guard_lead_may_merge(a)?;
    let cur = resolve::project(cx, &a.req("project")?)?;
    let (input, team) = project_input(cx, &cur, a)?;
    let moves = team.as_ref().is_some_and(|t| cur.team_id.as_deref() != Some(t.id.as_str()));
    if moves && cx.check.is_some() {
        return Err(format!("A board check never moves a project to another team: ask the user in a chat (start_chat) instead. {} stays where it is.",
                           cur.name));
    }
    // GA-261: a new repository, folder, main branch or goal steers the project's agents.
    let steers = project_changes(cx, &cur, &input).iter().any(|c| STEERS_PROJECT.contains(&c.key));
    // After a tool from outside Gizai in this chat (asked once, for both): a move, whose cards land where that team's agents
    // start them, is refused (GA-101); a call that steers waits for the user's Approve.
    if (moves || steers) && let Some(tool) = cx.outside("update_project", a).await? {
        return Err(if moves { super::refused_after("update_project", &tool) } else { cx.keep("update_project", a, &tool) });
    }
    let moved = crate::runs::update_project(cx.st, cx.actor, &cur.id, input).map_err(err)?;
    cx.changed("projects");
    if moved {
        // Its cards are in the new team's columns: that team's agents may take those in its Auto columns.
        cx.changed("tasks");
        if tokio::runtime::Handle::try_current().is_ok() {
            let st = cx.st.clone();
            tokio::spawn(async move { crate::runs::pull(&st).await; });
        }
    }
    let p = projects::get(cx.db(), &cur.id).map_err(err)?;
    let label = format!("{} ({})", p.name, p.key);
    let mut v = json!({"ok": true, "project": {"id": p.id, "key": p.key, "name": p.name, "status": p.status, "repo_path": p.repo_path,
              "repository": p.repo_url, "provider": super::provider(p.repo_url.as_deref()), "test_runs_at_once": p.test_runs_at_once,
              "team": p.team_name},
              "link": link("project", &p.id, &label)});
    if moved {
        v["done"] = json!(format!("moved to {} with all its cards", p.team_name.as_deref().unwrap_or("the team")));
    }
    Ok(v)
}

// ---- tasks ----

pub(crate) async fn create_task(cx: &Cx<'_>, a: &Args) -> Result<Value, String> {
    let p = resolve::project(cx, &a.req("project")?)?;
    // GA-319: one line, at most 200 characters, refused before it waits for anything
    let title = tasks::check_title(&a.req("title")?).map_err(err)?;
    let team = resolve::team_of(cx, Some(&p))?;
    let column = match a.opt("column") { Some(c) => Some(resolve::column(&team, &c)?), None => None };
    let label_ids = match a.list("labels") { Some(l) => resolve::labels(&team, &l)?, None => vec![] };
    let assignee_id = match a.opt("assignee") {
        Some(n) if !n.eq_ignore_ascii_case("none") => Some(resolve::assignee(cx, &n)?),
        _ => None,
    };
    // GA-228: the cards it builds on, set in the same write, so no agent takes it before it waits for them.
    let builds_on = builds_on_arg(cx, a)?.unwrap_or_default();
    if builds_on.len() > gizai_core::builds_on::MAX {
        return Err(format!("A card can't build on more than {} cards. Nothing changed.", gizai_core::builds_on::MAX));
    }
    // GA-101: in an Auto column (the first column when none is given) after a tool from outside Gizai, it waits on hold.
    let held_after = match column.as_ref().or(team.states.first()) {
        Some(c) if auto(c) => cx.outside("create_task", a).await?,
        _ => None,
    };
    let reason = held_after.as_deref().map(waiting_for_ok);
    let id = tasks::create_with(cx.db(), cx.actor, TaskInput {
        project_id: p.id.clone(), title, description_md: a.opt("description_md").unwrap_or_default(), acceptance_md: a.opt("acceptance_md"),
        state_id: column.map(|c| c.id), priority: a.int("priority")?.unwrap_or(0), assignee_id, label_ids,
        testing: Some(a.flag("testing").unwrap_or(true)),
    }, reason.as_deref().map(waiting_hold), &builds_on).map_err(err)?;
    task_done(cx, &id, "created", held_after.as_deref())
}

pub(crate) async fn update_task(cx: &Cx<'_>, a: &Args) -> Result<Value, String> {
    let t = resolve::task(cx, &a.req("task")?)?;
    let mut patch = TaskPatch { title: title_arg(a)?, description_md: a.text("description_md"), acceptance_md: a.text("acceptance_md"),
                                priority: a.int("priority")?, testing: a.flag("testing"), ..Default::default() };
    if let Some(n) = a.text("assignee") {
        patch.assignee_id = Some(if n.is_empty() || n.eq_ignore_ascii_case("none") { String::new() } else { resolve::assignee(cx, &n)? });
    }
    let clears = a.flag("clear_hold") == Some(true);
    if clears {
        patch.hold = Some(String::new());
    } else if let Some(h) = a.opt("hold") {
        patch.hold = Some(h);
        patch.hold_reason = a.opt("hold_reason");
    } else if let Some(r) = a.text("hold_reason") {
        patch.hold_reason = Some(r);
    }
    // GA-261: only the user takes GA-101's hold off (on the card, or by approving a call that waited for them): a board check
    // or a chat answer doesn't clear it, put another hold in its place or rewrite its reason.
    if t.waits_for_your_ok() && !cx.approved && (patch.hold.is_some() || patch.hold_reason.is_some()) {
        return Err(super::ONLY_THE_USER.into());
    }
    let labels = a.list("labels");
    // GA-228: the full new list of cards it builds on; an empty list clears it.
    let builds_on = builds_on_arg(cx, a)?;
    let nothing = patch.title.is_none() && patch.description_md.is_none() && patch.acceptance_md.is_none() && patch.priority.is_none()
        && patch.testing.is_none() && patch.assignee_id.is_none() && patch.hold.is_none() && patch.hold_reason.is_none() && labels.is_none()
        && builds_on.is_none();
    if nothing {
        return Err("nothing to change: give a field to update".into());
    }
    // Everything given is checked before anything changes: a bad label (or hold, title, a loop in builds_on, …) changes
    // nothing.
    let label_ids = match labels {
        Some(l) => Some(resolve::labels(&resolve::team_of(cx, t.project_id.as_ref().and_then(|p| projects::get(cx.db(), p).ok()).as_ref())?, &l)?),
        None => None,
    };
    // GA-101: after a tool from outside Gizai in this chat, a card in an Auto column isn't taken off hold, and a change to
    // what its agent is told or to who starts it puts it on hold for the user's OK, in the same write. A run under way goes
    // on. GA-261: elsewhere such a change waits for the user's Approve, the whole call with it. GA-319: so does a new
    // reason for its hold, in any column.
    let steering = steers(cx, &t, &patch, builds_on.as_deref());
    let rewords = rewords_hold(&t, &patch);
    let mut held_after = None;
    if (clears || steering || rewords) && let Some(tool) = cx.outside("update_task", a).await? {
        // (an archived card is read-only: the write below says so, and nothing waits for an Approve that would fail)
        if rewords && t.archived_at.is_none() {
            return Err(cx.keep("update_task", a, &tool));
        }
        if in_auto(cx, &t)? {
            if clears {
                return Err(super::refused_after("update_task", &tool));
            }
            patch.hold = Some("needs_decision".into());
            patch.hold_reason = Some(waiting_for_ok(&tool));
            patch.hold_kind = Some(HOLD_WAITING_FOR_OK.into());
            held_after = Some(tool);
        } else if steering && t.archived_at.is_none() {
            return Err(cx.keep("update_task", a, &tool));
        }
    }
    tasks::update_with(cx.db(), cx.actor, &t.id, patch, label_ids, builds_on).map_err(err)?;
    task_done(cx, &t.id, "updated", held_after.as_deref())
}

pub(crate) async fn move_task(cx: &Cx<'_>, a: &Args) -> Result<Value, String> {
    let t = resolve::task(cx, &a.req("task")?)?;
    let project = t.project_id.as_ref().and_then(|p| projects::get(cx.db(), p).ok());
    let col = resolve::column(&resolve::team_of(cx, project.as_ref())?, &a.req("column")?)?;
    if cx.check.is_some() && matches!(col.category.as_str(), "review" | "deploy" | "done") {
        return Err(format!("A board check never moves a card to {}: ask the user in a chat (start_chat) instead.", col.name));
    }
    // GA-101: into an Auto column after a tool from outside Gizai, it waits on hold.
    let held_after = if auto(&col) { cx.outside("move_task", a).await? } else { None };
    let reason = held_after.as_deref().map(waiting_for_ok);
    tasks::move_to_held(cx.db(), cx.actor, &t.id, &col.id, "", reason.as_deref().map(waiting_hold)).map_err(err)?;
    task_done(cx, &t.id, "moved", held_after.as_deref())
}

pub(crate) fn comment(cx: &Cx, a: &Args) -> Result<Value, String> {
    let t = resolve::task(cx, &a.req("task")?)?;
    let body = a.req("body_md")?;
    comments::add(cx.db(), cx.actor, &t.id, &body, None).map_err(err)?;
    cx.changed("comments");
    Ok(json!({"ok": true, "done": "commented", "link": link("task", &t.id, &format!("{} {}", t.identifier, short(&t.title, 60)))}))
}

// ---- archiving cards in Done (GA-307) ----

/// GA-307: how many cards an answer of archive_tasks or restore_tasks names; `more` counts the rest.
const NAMED: usize = 50;

/// What a call of archive_tasks asks for: the cards `tasks` names, or every card in the team's Done column (`all_done`).
pub(crate) enum Scope {
    Tasks(Vec<String>),
    AllDone,
}

/// The scope of a call of archive_tasks: exactly one of `tasks` (an empty list names no card) and `all_done: true`.
/// Neither or both is refused with why. Needs nothing but the arguments, so a call that can never work is refused before
/// it waits for the user's Approve after a tool from outside Hersir.
pub(crate) fn archive_scope(a: &Args) -> Result<Scope, String> {
    let refs = a.list("tasks").unwrap_or_default();
    match (refs.is_empty(), a.flag("all_done").unwrap_or(false)) {
        (false, false) => Ok(Scope::Tasks(refs)),
        (true, true) => Ok(Scope::AllDone),
        (true, false) => Err("Say which cards: give tasks, their identifiers like [\"GA-12\"], or all_done: true for every card in Done. \
            Nothing was archived.".into()),
        (false, true) => Err("Give tasks or all_done, not both: tasks archives the cards it names, all_done every card in Done. \
            Nothing was archived.".into()),
    }
}

/// The cards a call of archive_tasks is about, as things are now.
pub(crate) struct Targets {
    /// The cards, in order and once each: those `tasks` names (archived ones included), or with `all_done` the cards in the
    /// team's Done column, top to bottom.
    pub cards: Vec<Task>,
    /// What `tasks` names that is no card, with why: it is skipped.
    pub unknown: Vec<Skipped>,
    /// With `all_done`: whose Done column.
    pub all_done: Option<AllDone>,
}

/// The Done column `all_done` empties: its team, its name, and the project when only that project's cards go.
pub(crate) struct AllDone {
    pub team: team::Team,
    pub column: String,
    pub project: Option<Project>,
}

/// GA-307: the cards of a call of archive_tasks (`archive_scope`). With `all_done`: the cards in the Done column of the
/// `team` given (needed with more than one team, as add_column's), or of the `project`'s team with only that project's
/// cards. Reads only, so the preview of a call that waits for the user's Approve uses it too.
pub(crate) fn archive_targets(cx: &Cx, a: &Args) -> Result<Targets, String> {
    if let Scope::Tasks(refs) = archive_scope(a)? {
        let (cards, unknown) = named_cards(cx, &refs)?;
        return Ok(Targets { cards, unknown, all_done: None });
    }
    let project = match a.opt("project") { Some(p) => Some(resolve::project(cx, &p)?), None => None };
    let team = match &project {
        Some(p) => {
            let t = resolve::team_of(cx, Some(p))?;
            if let Some(named) = a.opt("team").map(|r| resolve::team(cx, &r)).transpose()?.filter(|n| n.id != t.id) {
                return Err(format!("{} ({}) is on {}'s board, not {}'s: leave team out, or name {}. Nothing was archived.",
                                   p.name, p.key, t.name, named.name, t.name));
            }
            t
        }
        None => resolve::team_arg(cx, a)?,
    };
    let done: Vec<&team::WorkflowState> = team.states.iter().filter(|s| s.category == "done").collect();
    if done.is_empty() {
        return Err(format!("{}'s board has no Done column, so nothing was archived.", team.name));
    }
    let column = match done.as_slice() {
        [only] => only.name.clone(),
        _ => "Done".to_string(),
    };
    let filter = TaskFilter { project_id: project.as_ref().map(|p| p.id.clone()), open_only: false };
    let cards = tasks::list_lean(cx.db(), &filter).map_err(err)?.into_iter()
        .filter(|t| done.iter().any(|s| s.id == t.state_id)).collect();
    Ok(Targets { cards, unknown: vec![], all_done: Some(AllDone { team, column, project }) })
}

/// The cards `refs` names by identifier (GA-12, case ignored) or id, archived cards included: in the order given, once
/// each. Titles aren't matched: a reference to no card is skipped, with why.
fn named_cards(cx: &Cx, refs: &[String]) -> Result<(Vec<Task>, Vec<Skipped>), String> {
    let mut all = tasks::list_lean(cx.db(), &TaskFilter::default()).map_err(err)?;
    all.extend(tasks::archived(cx.db(), None).map_err(err)?);
    let mut by_ref: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for (i, t) in all.iter().enumerate() {
        by_ref.insert(t.identifier.to_uppercase(), i);
        by_ref.insert(t.id.clone(), i);
    }
    let (mut cards, mut unknown): (Vec<Task>, Vec<Skipped>) = (vec![], vec![]);
    for r in refs {
        let r = r.trim();
        match by_ref.get(r).or_else(|| by_ref.get(&r.to_uppercase())) {
            Some(&i) if !cards.iter().any(|t| t.id == all[i].id) => cards.push(all[i].clone()),
            Some(_) => {}
            None => {
                let name = r.to_uppercase();
                if !unknown.iter().any(|s| s.identifier == name) {
                    unknown.push(Skipped { reason: format!("No task {name}: check the identifier with list_tasks"), identifier: name });
                }
            }
        }
    }
    Ok((cards, unknown))
}

/// The answer of archive_tasks and restore_tasks: how many cards went (`done`: "archived" or "restored"), the first
/// `NAMED` of them by identifier with `more` for the rest, and every card that was skipped, with why.
fn bulk_answer(done: &str, out: BulkResult, cards: &[Task], unknown: Vec<Skipped>) -> Value {
    let names: std::collections::HashMap<&str, &str> = cards.iter().map(|t| (t.id.as_str(), t.identifier.as_str())).collect();
    let gone: Vec<&str> = out.done.iter().map(|id| names.get(id.as_str()).copied().unwrap_or(id.as_str())).collect();
    let skipped: Vec<Value> = out.skipped.iter().chain(&unknown).map(|s| json!({"task": s.identifier, "reason": s.reason})).collect();
    let mut v = json!({"ok": true, "done": done, "tasks": gone.iter().take(NAMED).collect::<Vec<_>>(), "skipped": skipped});
    v[done] = json!(gone.len());
    if gone.len() > NAMED {
        v["more"] = json!(gone.len() - NAMED);
    }
    v
}

/// GA-307: archives cards in Done, as the board's Archive all does: the cards an agent is working on (`runs::working_on`,
/// a run's worktree being made included) are skipped, then `tasks::archive_many` archives the rest in one write, skipping a
/// card outside Done or already archived with why. One `rows-changed`. The cards: `archive_targets`.
pub(crate) fn archive_tasks(cx: &Cx, a: &Args) -> Result<Value, String> {
    let t = archive_targets(cx, a)?;
    let ids: Vec<String> = t.cards.iter().map(|c| c.id.clone()).collect();
    let mut out = BulkResult::default();
    if !ids.is_empty() {
        let working: Vec<String> = ids.iter().filter(|id| crate::runs::working_on(cx.st, id)).cloned().collect();
        out = tasks::archive_many(cx.db(), cx.actor, &ids, &working).map_err(err)?;
        cx.changed("tasks");
    }
    Ok(bulk_answer("archived", out, &t.cards, t.unknown))
}

/// GA-307: puts archived cards back at the bottom of their Done column, in the order given, as they were
/// (`tasks::restore_many`, one write; a card that isn't archived is skipped with why), with one `rows-changed`. It only
/// undoes an archive, so it runs at once after a tool from outside Hersir too.
pub(crate) fn restore_tasks(cx: &Cx, a: &Args) -> Result<Value, String> {
    let refs = a.list("tasks").unwrap_or_default();
    if refs.is_empty() {
        return Err("Say which cards: give tasks, the archived cards' identifiers like [\"GA-12\"]. Nothing was restored.".into());
    }
    let (cards, unknown) = named_cards(cx, &refs)?;
    let ids: Vec<String> = cards.iter().map(|c| c.id.clone()).collect();
    let mut out = BulkResult::default();
    if !ids.is_empty() {
        out = tasks::restore_many(cx.db(), cx.actor, &ids).map_err(err)?;
        cx.changed("tasks");
    }
    Ok(bulk_answer("restored", out, &cards, unknown))
}

// ---- agents ----

fn budget(a: &Args, cur: Option<i64>) -> Result<Option<i64>, String> {
    match a.text("monthly_budget_usd") {
        Some(s) if s.is_empty() => Ok(None),
        Some(_) => Ok(a.num("monthly_budget_usd")?.map(|d| (d * 1_000_000.0).round() as i64)),
        None => Ok(cur),
    }
}

fn agent_result(cx: &Cx, id: &str, what: &str) -> Result<Value, String> {
    cx.changed("actors");
    // A new name, or joining or leaving a shared memory folder (GA-96), moves notes.
    cx.changed("docs");
    let m = team::agent(cx.db(), id).map_err(err)?;
    let columns = gizai_core::columns::of_agent(cx.db(), &m.actor_id).unwrap_or_default();
    Ok(json!({"ok": true, "done": what, "agent": {"id": m.actor_id, "name": m.name, "role": m.role_key, "status": m.status, "columns": columns},
              "link": link("agent", &m.actor_id, &m.name)}))
}

/// The coding CLI from `runs_on` (its name or id), as its id; None when not given. An Other CLI runs with its own
/// permissions (whatever its arguments allow), so only you can pick one, in the agent form. GA-352: Codex and Gemini, which
/// don't keep an agent to its allowed commands either, wait for your Approve instead (`keeps_to_the_list`).
pub(crate) fn runs_on(cx: &Cx<'_>, a: &Args) -> Result<Option<gizai_core::clis::Cli>, String> {
    let Some(want) = a.opt("runs_on") else { return Ok(None) };
    let all = gizai_core::clis::list(cx.db()).map_err(err)?;
    let Some(c) = all.iter().find(|c| c.id == want.trim() || c.name.eq_ignore_ascii_case(want.trim())) else {
        let names: Vec<&str> = all.iter().map(|c| c.name.as_str()).collect();
        return Err(format!("there is no coding CLI called \"{want}\". Use one of: {}.", names.join(", ")));
    };
    if c.kind == "other" {
        return Err(format!("{} runs with its own permissions, so it can't be picked from chat: set it yourself in the agent's settings (Runs on).", c.name));
    }
    Ok(Some(c.clone()))
}

/// Memory (GA-96): `shares_memory_with` as the agent's id; Some("") for its own folder ("none" or empty); None when not
/// given. A name no agent has is refused, so nothing changes.
fn shares_memory_with(cx: &Cx<'_>, a: &Args) -> Result<Option<String>, String> {
    let Some(want) = a.text("shares_memory_with") else { return Ok(None) };
    if want.is_empty() || ["none", "own", "own folder", "its own folder"].contains(&want.to_lowercase().as_str()) {
        return Ok(Some(String::new()));
    }
    Ok(Some(resolve::agent(cx, &want)?.actor_id))
}

pub(crate) async fn create_agent(cx: &Cx<'_>, a: &Args) -> Result<Value, String> {
    guard_agent_powers(a)?;
    // GA-262: no shell or interpreter, unless its role's default list has it.
    guard_commands(a, &gizai_core::seed::role_tools(&team::role_key(&a.opt("role").unwrap_or_default())))?;
    // GA-225: the team it joins; with more than one team it must be named.
    let team_id = resolve::team_arg(cx, a)?.id;
    let shares = shares_memory_with(cx, a)?;
    let cli = runs_on(cx, a)?;
    // Only Claude Code has a model list to check against; Codex and Gemini take their own model names.
    if cli.as_ref().is_none_or(|c| c.kind == "claude_code") {
        check_model(cx, a.opt("model").as_deref(), a.opt("effort").as_deref()).await?;
    }
    let (name, role) = (a.req("name")?, a.req("role")?);
    // No list given: its role's, as the agent form starts it.
    let allowed_tools = a.list("allowed_tools").filter(|l| !l.is_empty()).unwrap_or_else(|| gizai_core::seed::role_tools(&team::role_key(&role)));
    // GA-323, GA-352: what it gives the new agent only with the user's Approve.
    let needs = create_agent_needs_ok(cx, a, cli.as_ref());
    let input = AgentInput {
        name: name.clone(), role_key: role, title: a.opt("title"), adapter: cli.map(|c| c.id).unwrap_or_default(), model: a.opt("model"),
        instructions_md: a.opt("instructions_md"), permission_mode: a.opt("permission_mode").unwrap_or_default(),
        allowed_tools, wakeup: String::new(),
        heartbeat_minutes: None, budget_usd_micros: budget(a, None)?, chat_enabled: None, effort: a.opt("effort"),
        max_runs: a.int("cards_at_once")?, board_check_minutes: a.int("board_check_minutes")?, folders: None, use_memory: None,
        shares_memory_with: shares,
    };
    // GA-323: a command chat doesn't give at once waits for the user's Approve, the whole call with it. GA-352: so does a CLI
    // that doesn't keep the agent to its allowed commands (Codex, Gemini), and a permission mode that isn't stricter than its
    // CLI's usual one (on Claude Code dontAsk and auto). Checked after the rest, so a call that can never work is refused at
    // once instead.
    if !needs.is_empty() && !cx.approved {
        team::check_agent(cx.db(), &input).map_err(err)?;
        return Err(cx.keep_for_ok("create_agent", a, name.trim(), &needs));
    }
    let id = team::add_agent(cx.db(), cx.actor, &team_id, input).map_err(err)?;
    agent_result(cx, &id, "created")
}

pub(crate) async fn update_agent(cx: &Cx<'_>, a: &Args) -> Result<Value, String> {
    guard_agent_powers(a)?;
    let m = resolve::agent(cx, &a.req("agent")?)?;
    // GA-262: no shell or interpreter it doesn't have already (without a list of its own it runs with Hersir's default
    // list), unless its role's default list has it: its role now or the one given.
    let mut kept = if m.allowed_tools.is_empty() { gizai_core::seed::DEFAULT_TOOLS.map(String::from).to_vec() } else { m.allowed_tools.clone() };
    kept.extend(gizai_core::seed::role_tools(&m.role_key));
    kept.extend(a.opt("role").map(|r| gizai_core::seed::role_tools(&team::role_key(&r))).unwrap_or_default());
    guard_commands(a, &kept)?;
    let shares = shares_memory_with(cx, a)?;
    // Moved to another kind of CLI: its model, effort and permission mode don't carry over unless given (the CLI's defaults).
    let kind_now = crate::clis::of_agent(cx.st, m.adapter.as_deref()).map(|c| c.kind).unwrap_or_default();
    let new_cli = runs_on(cx, a)?;
    let moved = new_cli.as_ref().is_some_and(|c| c.kind != kind_now);
    // GA-323, GA-352: what it gives the agent only with the user's Approve.
    let needs = update_agent_needs_ok(cx, &m, a, new_cli.as_ref());
    let model = if moved { a.opt("model") } else { keep(a, "model", &m.model) };
    let effort = if moved { a.opt("effort") } else { keep(a, "effort", &m.effort) };
    let permission_mode = a.opt("permission_mode").or(if moved { None } else { m.permission_mode.clone() }).unwrap_or_default();
    let adapter = match new_cli { Some(c) => Some(c.id), None => m.adapter.clone() };
    // Only Claude Code has a model list to check against; Codex, Gemini and other CLIs take their own model names.
    let on_claude = crate::clis::of_agent(cx.st, adapter.as_deref()).is_ok_and(|c| c.kind == "claude_code");
    if on_claude && (moved || a.text("model").is_some() || a.text("effort").is_some()) {
        check_model(cx, model.as_deref(), effort.as_deref()).await?;
    }
    let input = AgentInput {
        name: a.opt("name").unwrap_or(m.name.clone()), role_key: a.opt("role").unwrap_or(m.role_key.clone()),
        title: keep(a, "title", &m.title), adapter: adapter.unwrap_or_default(), model,
        instructions_md: a.opt("instructions_md"), permission_mode,
        allowed_tools: a.list("allowed_tools").unwrap_or(m.allowed_tools.clone()),
        wakeup: m.wakeup.clone().unwrap_or_default(),
        heartbeat_minutes: m.heartbeat_minutes,
        budget_usd_micros: budget(a, m.budget_usd_micros)?, chat_enabled: None, effort, max_runs: a.int("cards_at_once")?,
        board_check_minutes: a.int("board_check_minutes")?, folders: None, use_memory: None, shares_memory_with: shares,
    };
    // GA-323: a command the agent doesn't run with yet, and that chat doesn't give at once, waits for the user's Approve, the
    // whole call with it (its role's list no longer counts: a narrowed agent gets nothing back from chat by itself). GA-352:
    // so does a move to another CLI that doesn't keep it to its allowed commands (Codex, Gemini), and a permission mode that
    // isn't stricter than the one it runs with now. Checked after the rest, so a call that can never work is refused at once
    // instead.
    if !needs.is_empty() && !cx.approved {
        team::check_agent(cx.db(), &input).map_err(err)?;
        return Err(cx.keep_for_ok("update_agent", a, &m.name, &needs));
    }
    team::update_agent(cx.db(), cx.actor, &m.actor_id, input).map_err(err)?;
    crate::runs::resume_pull(cx.st, &m.actor_id);
    agent_result(cx, &m.actor_id, "updated")
}

pub(crate) fn set_agent_status(cx: &Cx, a: &Args) -> Result<Value, String> {
    let m = resolve::agent(cx, &a.req("agent")?)?;
    team::set_agent_status(cx.db(), cx.actor, &m.actor_id, &a.req("status")?).map_err(err)?;
    crate::runs::resume_pull(cx.st, &m.actor_id);
    agent_result(cx, &m.actor_id, "status changed")
}

/// The setup a tool gives for a column (agents by name, auto, next column by name, a new name, the column it goes after),
/// as a `ColumnInput`. "none" (or empty) as next clears it.
fn column_input(cx: &Cx, t: &team::Team, a: &Args, renaming: bool) -> Result<gizai_core::columns::ColumnInput, String> {
    let agent_ids = match a.list("agents") {
        Some(names) => Some(names.iter().map(|n| resolve::agent(cx, n).map(|m| m.actor_id)).collect::<Result<Vec<_>, String>>()?),
        None => None,
    };
    let next_state_id = match a.text("next") {
        Some(n) if n.is_empty() || n.eq_ignore_ascii_case("none") => Some(String::new()),
        Some(n) => Some(resolve::column(t, &n)?.id),
        None => None,
    };
    let after_id = match a.opt("after") { Some(n) => Some(resolve::column(t, &n)?.id), None => None };
    Ok(gizai_core::columns::ColumnInput {
        name: if renaming { a.opt("new_name") } else { None }, agent_ids, auto: a.flag("auto"), next_state_id, after_id,
    })
}

/// The columns of team `team_id` after a change to them.
fn workflow_done(cx: &Cx, team_id: &str, what: &str) -> Result<Value, String> {
    cx.changed("workflow_states");
    let t = team::get(cx.db(), team_id).map_err(err)?;
    if tokio::runtime::Handle::try_current().is_ok() {
        let st = cx.st.clone();
        tokio::spawn(async move { crate::runs::pull(&st).await; });
    }
    let columns: Vec<String> = t.states.iter().map(|s| format!("{}: {}", s.name, super::read::column_line(s, &t))).collect();
    Ok(json!({"ok": true, "done": what, "team": t.name, "columns": columns, "link": {"page": "team", "id": t.id, "label": "Workflow"}}))
}

pub(crate) fn add_column(cx: &Cx, a: &Args) -> Result<Value, String> {
    // GA-225: the team whose board it goes on; with more than one team it must be named.
    let t = resolve::team_arg(cx, a)?;
    let after = resolve::column(&t, &a.req("after")?)?;
    let kind = a.req("kind").or_else(|_| a.req("category"))?;
    let category = gizai_core::columns::category_of(&kind)
        .ok_or_else(|| format!("a column is waiting, work, testing, review, deploy, done or backlog, not {kind}"))?;
    let setup = column_input(cx, &t, a, false)?;
    let name = a.req("name")?;
    // Checked before the column is added: nothing changes when its setup is refused.
    if setup.agent_ids.as_ref().is_some_and(|l| !l.is_empty()) && !gizai_core::columns::takes_agents(category) {
        return Err(format!("a {kind} column takes no agents: Backlog, Review, Done and Cancelled columns are for people"));
    }
    if setup.auto == Some(true) && setup.next_state_id.as_deref().is_none_or(str::is_empty) {
        return Err("an Auto column needs a next column, so a finished card isn't picked up again: give next".into());
    }
    let id = team::add_state(cx.db(), cx.actor, &t.id, &name, &after.id, category).map_err(err)?;
    let setup = gizai_core::columns::ColumnInput { after_id: None, ..setup };
    if let Err(e) = gizai_core::columns::set_column(cx.db(), cx.actor, &id, setup) {
        cx.changed("workflow_states");
        return Err(format!("{name} was added after {}, but its setup was refused: {}", after.name, err(e)));
    }
    workflow_done(cx, &t.id, "column added")
}

pub(crate) fn set_column(cx: &Cx, a: &Args) -> Result<Value, String> {
    // GA-225: the team whose column it is; with more than one team it must be named.
    let t = resolve::team_arg(cx, a)?;
    let col = resolve::column(&t, &a.req("column")?)?;
    let input = column_input(cx, &t, a, true)?;
    let nothing = input.name.is_none() && input.agent_ids.is_none() && input.auto.is_none() && input.next_state_id.is_none() && input.after_id.is_none();
    if nothing {
        return Err("nothing to change: give agents, auto, next, new_name or after".into());
    }
    gizai_core::columns::set_column(cx.db(), cx.actor, &col.id, input).map_err(err)?;
    workflow_done(cx, &t.id, "column changed")
}

pub(crate) fn save_label(cx: &Cx, a: &Args) -> Result<Value, String> {
    let current = match a.opt("label") {
        Some(n) => Some(gizai_core::labels::find(cx.db(), &n).map_err(err)?.ok_or_else(|| format!("there is no label {n}: leave label out to add a new one"))?),
        None => None,
    };
    let name = a.opt("name").or_else(|| current.as_ref().map(|l| l.name.clone())).ok_or("missing \"name\"")?;
    let id = gizai_core::labels::save(cx.db(), cx.actor, current.as_ref().map(|l| l.id.as_str()), &name, a.opt("color").as_deref()).map_err(err)?;
    cx.changed("labels");
    let labels: Vec<String> = gizai_core::labels::list(cx.db()).map_err(err)?.into_iter().map(|l| l.name).collect();
    Ok(json!({"ok": true, "done": if current.is_some() { "label changed" } else { "label added" }, "label": {"id": id, "name": name},
              "labels": labels, "link": {"page": "team", "id": resolve::team_of(cx, None)?.id, "label": "Labels"}}))
}

/// A board check never starts more runs than the agent's free slots allow ("Runs at once" is checked by every start).
fn check_free_slot(cx: &Cx, agent_id: Option<&str>) -> Result<(), String> {
    let Some(agent_id) = agent_id.filter(|_| cx.check.is_some()) else { return Ok(()) };
    let m = team::agent(cx.db(), agent_id).map_err(err)?;
    let busy = crate::runs::live(cx.st).iter().filter(|r| r.agent_id == m.actor_id).count() as i64;
    if busy >= m.max_runs.max(1) {
        return Err(format!("{} has no free slot ({busy} of {} cards at once): a board check doesn't start more. Leave the card waiting.", m.name, m.max_runs.max(1)));
    }
    Ok(())
}

pub(crate) async fn start_run(cx: &Cx<'_>, a: &Args) -> Result<Value, String> {
    let t = resolve::task(cx, &a.req("task")?)?;
    // GA-261: a card on GA-101's hold waits for the user's OK, also with an agent named: only they get it going again.
    if t.waits_for_your_ok() && !cx.approved {
        return Err(super::ONLY_THE_USER.into());
    }
    let agent = match a.opt("agent") { Some(n) => Some(resolve::agent(cx, &n)?.actor_id), None => None };
    check_free_slot(cx, agent.clone().or_else(|| crate::runs::suggest(cx.st, &t.id)).as_deref())?;
    let (run_id, _done) = crate::runs::start(cx.st, &t.id, agent, None, "manual").await?;
    let run = gizai_core::runs::get(cx.db(), &run_id).map_err(err)?;
    Ok(json!({"ok": true, "done": "started", "run": {"id": run.id, "agent": run.agent_name, "branch": run.branch},
              "link": link("task", &t.id, &format!("{} {}", t.identifier, short(&t.title, 60)))}))
}

/// Continue on the card's latest run, like the Continue button (`runs::continue_run`); a run that ended asking for a
/// decision continues too, with what was written on the card since (`runs::continue_answered`). A `note` goes to the
/// agent with it and on the card as the Chief's comment (`runs::continue_answered_with_note`, GA-31).
pub(crate) async fn continue_run(cx: &Cx<'_>, a: &Args) -> Result<Value, String> {
    let t = resolve::task(cx, &a.req("task")?)?;
    // GA-261: a Continue takes the hold off: not GA-101's, which waits for the user's OK, unless they approved this call.
    if t.waits_for_your_ok() && !cx.approved {
        return Err(super::ONLY_THE_USER.into());
    }
    let last = gizai_core::runs::list_for_task(cx.db(), &t.id).map_err(err)?.into_iter().next()
        .ok_or_else(|| format!("{} has no run to continue: start_agent_run starts one", t.identifier))?;
    check_free_slot(cx, Some(&last.agent_id))?;
    let note = a.opt("note").filter(|n| !n.trim().is_empty());
    let noted = note.is_some();
    let (run_id, _done) = crate::runs::continue_answered_with_note(cx.st, &last.id, cx.actor, note).await?;
    cx.changed("tasks");
    if noted {
        cx.changed("comments");
    }
    let run = gizai_core::runs::get(cx.db(), &run_id).map_err(err)?;
    Ok(json!({"ok": true, "done": "continued", "run": {"id": run.id, "agent": run.agent_name, "branch": run.branch},
              "link": link("task", &t.id, &format!("{} {}", t.identifier, short(&t.title, 60)))}))
}

// ---- chats the Chief starts ----

/// Asks the user in a chat that waits at the top of their Inbox (`chat::start_lead_chat`): one waiting chat per card.
/// GA-319: from a chat that used a tool from outside Gizai, the chat its message goes to is marked with that tool
/// (`chat::start_lead_chat_from`), as the message may carry what that tool returned, also where the user can't see it (an
/// HTML comment): what the Chief does there waits for the user's Approve too.
pub(crate) async fn start_chat(cx: &Cx<'_>, a: &Args) -> Result<Value, String> {
    let title = a.req("title")?;
    let kind = a.req("kind")?.to_lowercase();
    let body = a.req("body_md")?;
    let refs = a.list("tasks").unwrap_or_default();
    if refs.is_empty() {
        return Err("name the cards the chat is about (tasks), like [\"GA-12\"]".into());
    }
    let mut ids = vec![];
    for r in &refs {
        let t = resolve::task(cx, r)?;
        if !ids.contains(&t.id) {
            ids.push(t.id);
        }
    }
    let outside = cx.outside("start_chat", a).await?;
    let (id, new, marked) = gizai_core::chat::start_lead_chat_from(cx.db(), cx.actor, &title, &kind, &ids, &body, cx.check, outside.as_deref())
        .map_err(err)?;
    if let Some(tool) = outside.as_deref().filter(|_| marked) {
        crate::chat::marked_from_chat(cx.st, &id, tool);
    }
    cx.changed("chat_threads");
    cx.changed("chat_messages");
    (cx.st.notify)(crate::runs::Note::ChatChanged);
    let t = gizai_core::chat::get_thread(cx.db(), &id).map_err(err)?;
    let mut v = json!({"ok": true, "done": if new { "chat started" } else { "added to the chat that already waits for this card" },
              "chat": {"id": t.id, "title": t.title, "kind": t.kind, "tasks": t.tasks}, "link": link("chat", &t.id, &t.title)});
    if let Some(tool) = &outside {
        v["waits_for_approve"] = json!(format!("What you do in that chat waits for the user's Approve too: you used {tool}, a tool from \
            outside Hersir, in this chat."));
    }
    Ok(v)
}

pub(crate) fn stop_run(cx: &Cx, a: &Args) -> Result<Value, String> {
    let t = resolve::task(cx, &a.req("task")?)?;
    let live: Vec<_> = crate::runs::live(cx.st).into_iter().filter(|r| r.task_id == t.id).collect();
    if live.is_empty() {
        return Err(format!("no agent is working on {} right now", t.identifier));
    }
    for r in &live {
        crate::runs::stop(cx.st, &r.run_id);
    }
    Ok(json!({"ok": true, "done": "stopping", "link": link("task", &t.id, &format!("{} {}", t.identifier, short(&t.title, 60)))}))
}

// ---- docs, files, people ----

/// record_decision: a numbered decision record on a project's Docs page, after the project's own records. Only for a
/// project with Docs on and its Decisions shown: elsewhere the decision belongs in Memory.
pub(crate) fn record_decision(cx: &Cx, a: &Args) -> Result<Value, String> {
    let project = resolve::project(cx, &a.req("project")?)?;
    if !project.dev_docs || project.docs_off.iter().any(|s| s == "decisions") {
        return Err(format!("{}'s Docs page doesn't keep decisions (Edit project → Docs): save the decision in Memory instead, \
                            in Decisions/ with project: {}.", project.name, project.key));
    }
    let task_id = match a.text("task").filter(|t| !t.trim().is_empty()) {
        Some(t) => Some(resolve::task(cx, &t)?.id),
        None => None,
    };
    let input = gizai_core::devdocs::DecisionInput {
        title: a.req("title")?,
        decision: a.req("decision")?,
        context: a.text("context").unwrap_or_default(),
        consequences: a.text("consequences").unwrap_or_default(),
        alternatives: a.text("alternatives").unwrap_or_default(),
        status: a.text("status"),
        related_files: a.list("related_files").unwrap_or_default(),
        task_id,
        supersedes: a.int("supersedes")?,
    };
    let by = crate::runs::actor_name(cx.st, cx.actor);
    let record = gizai_core::devdocs::record_decision(cx.db(), cx.actor, &by, &project.id, own_records(&project), input).map_err(err)?;
    Ok(json!({ "decision": record, "shows": format!("{}'s Docs page, Decisions {:04}", project.name, record.number) }))
}

/// The highest number among the project's own decision records (docs/decisions and the like, as its Docs page reads
/// them), which Hersir's follow; 0 without any.
fn own_records(project: &Project) -> i64 {
    let Ok(repo) = crate::changes::linked_repo(project) else { return 0 };
    let Ok(files) = gizai_agents::worktree::tracked_files(&repo) else { return 0 };
    let Some(folder) = gizai_docs::content::decisions_folder(&files) else { return 0 };
    files.iter().filter_map(|f| f.strip_prefix(folder)?.strip_prefix('/')).filter(|name| !name.contains('/'))
        .filter_map(|name| name.split(['-', '_']).next()?.parse::<i64>().ok()).max().unwrap_or(0)
}

pub(crate) fn create_doc(cx: &Cx, a: &Args) -> Result<Value, String> {
    let p = resolve::project(cx, &a.req("project")?)?;
    let title = a.req("title")?;
    let id = docs::create(cx.db(), cx.actor, &p.id, &title).map_err(err)?;
    if let Some(body) = a.opt("body_md") {
        docs::save(cx.db(), cx.actor, &id, &body, 1).map_err(err)?;
    }
    cx.changed("docs");
    let d = docs::get(cx.db(), &id).map_err(err)?;
    Ok(json!({"ok": true, "doc": {"id": d.id, "title": d.title, "version": d.current_version}, "link": link("doc", &d.id, &d.title)}))
}

pub(crate) fn write_doc(cx: &Cx, a: &Args) -> Result<Value, String> {
    let project = match a.opt("project") { Some(p) => Some(resolve::project(cx, &p)?), None => None };
    let d = resolve::doc(cx, &a.req("doc")?, project.as_ref())?;
    let body = a.text("body_md").ok_or("missing \"body_md\"")?;
    let v = docs::save(cx.db(), cx.actor, &d.id, &body, d.current_version).map_err(err)?;
    cx.changed("docs");
    Ok(json!({"ok": true, "doc": {"id": d.id, "title": d.title, "version": v}, "link": link("doc", &d.id, &d.title)}))
}

pub(crate) async fn attach_file(cx: &Cx<'_>, a: &Args) -> Result<Value, String> {
    let path = attachable(cx, a)?;
    let (owner_type, owner_id, l) = if let Some(r) = a.opt("task") {
        let t = resolve::task(cx, &r)?;
        ("task", t.id.clone(), link("task", &t.id, &format!("{} {}", t.identifier, short(&t.title, 60))))
    } else if let Some(r) = a.opt("project") {
        let p = resolve::project(cx, &r)?;
        ("project", p.id.clone(), link("project", &p.id, &format!("{} ({})", p.name, p.key)))
    } else if let Some(r) = a.opt("client") {
        let c = resolve::client(cx, &r)?;
        ("client", c.id.clone(), link("client", &c.id, &c.name))
    } else {
        return Err("say where to attach it: a task, project or client".into());
    };
    let (db, actor, dir) = (cx.st.db.clone(), cx.actor.to_string(), cx.st.data_dir.clone());
    let oid = owner_id.clone();
    let f = tokio::task::spawn_blocking(move || files::add_from_path(&db, &actor, &dir, owner_type, &oid, &path))
        .await.map_err(|e| e.to_string())?.map_err(err)?;
    cx.changed("files");
    Ok(json!({"ok": true, "file": {"id": f.id, "name": f.name, "size_bytes": f.size_bytes}, "link": l}))
}

/// The file attach_file names (`path`: absolute, or `~/…` from your home folder) when the Chief may attach it
/// (`allowed_attachment`); else why not, and nothing is attached. GA-351: also asked before a call from the Chief's turn
/// that answers Claude Desktop could be kept for your Approve (`tools::call_scoped`).
pub(crate) fn attachable(cx: &Cx, a: &Args) -> Result<std::path::PathBuf, String> {
    let raw = a.req("path")?;
    // `~/…`, and on Windows `~\…` too
    let path = match raw.strip_prefix("~/").or_else(|| raw.strip_prefix("~\\").filter(|_| cfg!(windows))) {
        Some(rest) => std::path::PathBuf::from(gizai_core::clis::home()).join(rest),
        None => std::path::PathBuf::from(&raw),
    };
    if !path.is_absolute() {
        return Err(format!("{raw} is not an absolute path"));
    }
    allowed_attachment(cx, &raw, &path)?;
    Ok(path)
}

/// Spec §8: the Chief attaches only a file the user named in this chat, one the user added to a message in this chat
/// (its copy in the Chief's folder, `chat::lead_file_path`), or one inside its copies of the projects' code (`code`),
/// never something it found elsewhere on the disk (keys, credentials). The copies hold only tracked files, so a
/// repository's .env isn't among them. GA-341: Claude Desktop has no chat in Hersir to name or add a file in, so only the
/// copies. GA-351: the same for the Chief's turn that answers a message from Claude Desktop (`Desktop::Answering`), and a
/// path in a message from Claude Desktop is never one the user named, in any turn.
fn allowed_attachment(cx: &Cx, raw: &str, path: &std::path::Path) -> Result<(), String> {
    let real = path.canonicalize().map_err(|_| format!("can't read {raw}"))?;
    let in_copy = crate::code::dirs(cx.st).iter()
        .filter_map(|d| d.canonicalize().ok())
        .any(|d| real.starts_with(&d));
    if in_copy {
        return Ok(());
    }
    if cx.desktop.limits() {
        return Err(format!("From Claude Desktop, attach_file takes only a file inside the Chief's copies of the projects' code; {raw} isn't \
                            one. Nothing was attached: the user can attach it themselves in Hersir."));
    }
    let named = cx.thread.is_some_and(|t| {
        gizai_core::chat::messages(cx.db(), t).unwrap_or_default().iter()
            // Saved as the user's, a message from Claude Desktop (ask_chief) still isn't one they typed (GA-351).
            .filter(|m| m.role == "user" && !m.from_desktop())
            .filter_map(|m| m.body_md.as_deref())
            .any(|b| b.contains(raw) || b.contains(&*real.to_string_lossy()) || b.contains(&*path.to_string_lossy()))
    });
    let added = cx.thread.is_some_and(|t| {
        gizai_core::chat::thread_files(cx.db(), t).unwrap_or_default().iter()
            .filter_map(|f| crate::chat::lead_file_path(cx.st, f).canonicalize().ok())
            .any(|p| p == real)
    });
    if named || added {
        Ok(())
    } else {
        Err(format!("I can only attach a file you named in this chat, one you added to a message here, or one inside my copies of the projects' code; {raw} is none of these. Ask the user to give the path."))
    }
}

/// Chat only: starts the update of a project's linked folder to main (`code::start_update`), after the user said yes in
/// the chat; `switch` and `stash` only when they agreed to those too. Only the project's linked folder can be updated;
/// the result comes later as a system message in the chat.
pub(crate) async fn update_checkout(cx: &Cx<'_>, a: &Args) -> Result<Value, String> {
    let Some(thread) = cx.thread else {
        return Err("update_checkout works only in chat, after the user said yes there".into());
    };
    let p = resolve::project(cx, &a.req("project")?)?;
    let linked = p.repo_path.clone().filter(|r| !r.trim().is_empty())
        .ok_or_else(|| format!("{} has no linked folder to update", p.key))?;
    if let Some(f) = a.opt("folder") {
        let real = |s: &str| std::path::Path::new(s).canonicalize().ok();
        if real(&f).is_none() || real(&f) != real(&linked) {
            return Err(format!("update_checkout only updates {}'s linked folder ({linked}); {f} isn't it, so nothing changed", p.key));
        }
    }
    // Not a folder that the Chief's own folders (agent form → Folders) set to read.
    crate::folders::lead_may_update(cx.st, std::path::Path::new(&linked))?;
    let will = crate::code::start_update(cx.st, thread, &p, a.flag("switch").unwrap_or(false), a.flag("stash").unwrap_or(false)).await?;
    Ok(json!({"ok": true, "done": "started", "folder": linked, "will": will,
              "result": "comes as a message in this chat when the update ends",
              "link": link("project", &p.id, &format!("{} ({})", p.name, p.key))}))
}

pub(crate) fn add_person(cx: &Cx, a: &Args) -> Result<Value, String> {
    let id = users::create(cx.db(), cx.actor, &a.req("name")?, a.opt("email").as_deref()).map_err(err)?;
    cx.changed("actors");
    Ok(json!({"ok": true, "person": {"id": id, "name": a.req("name")?}}))
}
