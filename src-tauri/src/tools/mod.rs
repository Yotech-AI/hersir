//! The Chief's tools: Gizai's clients, projects, tasks, agents, docs, memory, files and inbox, served over MCP.
//! Each tool takes human references (KADE-12, "Kade portal", "Backend Agent"), calls gizai-core as the agent
//! (so activity shows who did it) and tells open screens what changed, exactly like the UI's commands.
pub mod apps;
pub mod desktop;
mod memory;
mod merge;
pub(crate) mod preview;
mod read;
mod resolve;
mod write;

use serde_json::{Map, Value, json};

use crate::AppState;
use crate::runs::Note;
pub use gizai_mcp::ToolDef;

/// The MCP server's view of the tools for one agent (the token's owner), in one chat thread or one board check (the
/// token's scope); for Claude Desktop (GA-338) the owner is you, and it gets the Chief's tools with the Chief's rules
/// (GA-341, `call_desktop`).
pub struct GizaiTools {
    pub st: AppState,
    pub actor: String,
    pub thread: Option<String>,
    /// The board check run the token was minted for.
    pub check: Option<String>,
    /// The run the token was minted for: the chat answer (or board check) whose Claude Code makes the calls.
    pub run: Option<String>,
    /// GA-70: the question (the run that asked) a Chief's run on a question was minted for: only the tools that
    /// read work there (`IN_A_QUESTION`).
    pub question: Option<String>,
    /// GA-338: the app outside Hersir the token was made for (its scope's `client`): `CLAUDE_DESKTOP`, which gets the
    /// Chief's tools with the Chief's rules (GA-341, `desktop`).
    pub client: Option<String>,
}

/// GA-338: the `client` in the scope of Claude Desktop's lasting token (`crate::claude_desktop`).
pub const CLAUDE_DESKTOP: &str = "claude-desktop";

impl GizaiTools {
    /// GA-341: Claude Desktop's token, by its client's name (`CLAUDE_DESKTOP`), for no chat, board check or question: it is
    /// offered the Chief's tools (`desktop_catalog`), and its calls run with the Chief's rules (`call_desktop`). The only
    /// token for no chat, board check or question that may change something; any other only reads (`call_scoped`).
    fn desktop(&self) -> bool {
        self.client.as_deref() == Some(CLAUDE_DESKTOP) && self.thread.is_none() && self.check.is_none() && self.question.is_none()
    }

    /// GA-345: what the view's tools (`apps::reply`) get for Claude Desktop's token: you, in no chat or board check.
    fn desktop_cx(&self) -> Cx<'_> {
        Cx { st: &self.st, actor: &self.actor, thread: None, check: None, from: None, approved: false, desktop: true }
    }
}

impl gizai_mcp::Tools for GizaiTools {
    fn list(&self) -> Vec<ToolDef> {
        if self.desktop() {
            return desktop_catalog();
        }
        // A run on a question only reads: it isn't offered the rest.
        catalog().into_iter().filter(|t| t.read_only || self.question.is_none()).collect()
    }
    async fn call(&self, name: &str, args: Value) -> Result<Value, String> {
        self.call_with_meta(name, args, &Value::Null).await
    }
    async fn call_with_meta(&self, name: &str, args: Value, meta: &Value) -> Result<Value, String> {
        // GA-344, GA-345: the tools of Claude Desktop's view (`apps::tools`: show, view_read, board_changes), which only read,
        // answered before anything else: no other token knows them, so a question's isn't told it can only look things up
        // (`IN_A_QUESTION`), nor another's that it can only read (`ONLY_READS`). As a value: their data, as `reply` puts it in
        // `structuredContent`.
        if apps::is_app_tool(name) {
            if !self.desktop() {
                return Err(format!("unknown tool {name}"));
            }
            let r = apps::reply(&self.desktop_cx(), name, &args_of(args)?)?;
            return Ok(r.structured.unwrap_or(Value::String(r.text)));
        }
        if self.question.is_some() && !reads_only(name) {
            return Err(IN_A_QUESTION.replace("{tool}", name));
        }
        // The answer's stream shows the tool use each call comes from, with the id Claude Code gives the call.
        let from = Stream { run: self.run.as_deref().unwrap_or_default(), tool_use: meta.get("claudecode/toolUseId").and_then(Value::as_str) };
        if self.desktop() {
            return call_desktop(&self.st, &self.actor, from, name, args).await;
        }
        call_scoped(&self.st, &self.actor, self.thread.as_deref(), self.check.as_deref(), Some(from), name, args, false, false).await
    }
    /// GA-343, GA-345: Claude Desktop also gets its view's tools (`apps::tools`: show, then view_read and board_changes for
    /// the view alone) when it shows MCP Apps' views. Every other token, and Claude Desktop without the extension, gets
    /// `list` as before (GA-345: without board_changes, which only the view calls).
    fn list_in(&self, session: &gizai_mcp::Session) -> Vec<ToolDef> {
        let mut list = self.list();
        if session.apps && self.desktop() {
            list.extend(apps::tools());
        }
        list
    }
    /// GA-343, GA-345: show, view_read and board_changes answer Claude Desktop with the view's data too
    /// (`structuredContent`, `apps::reply`); any other call, and theirs from another token (an unknown tool there), as
    /// `call_with_meta`.
    async fn reply(&self, name: &str, args: Value, meta: &Value, _session: &gizai_mcp::Session) -> Result<gizai_mcp::Reply, String> {
        if self.desktop() && apps::is_app_tool(name) {
            return apps::reply(&self.desktop_cx(), name, &args_of(args)?);
        }
        self.call_with_meta(name, args, meta).await.map(|v| gizai_mcp::Reply::of(&v))
    }
    /// GA-343: the board view, for any client that declared MCP Apps (its HTML holds no data: the tools bring that).
    fn apps(&self) -> Vec<gizai_mcp::AppView> {
        vec![apps::view()]
    }
}

/// Why a Chief's run on a question (GA-70) can't use a tool that changes something: Gizai does what its result line
/// says (answers on the card and continues the agent, or asks the user).
const IN_A_QUESTION: &str = "{tool} can't be used while you look at an agent's question: you only look things up (memory, the card, \
    docs, your copies of the code). End with your GIZAI_RESULT line: Hersir answers on the card and continues the agent, or asks the user.";

/// GA-344: a tool only Claude Desktop is offered, not the Chief's own turns: what changed on the board since a change
/// counter (`gizai_core::feed`), for a view in its chat that asks again and again, as Claude doesn't tell it. Answered
/// before anything else (`GizaiTools::call_with_meta`). GA-345: one of the view's tools (`apps::tools`), offered with
/// `_meta.ui.visibility` `["app"]` to a Claude Desktop that shows MCP Apps' views, so that only the view calls it.
pub const BOARD_CHANGES: &str = "board_changes";

/// board_changes's definition. It only reads, though its name doesn't say so (`tool` marks the gets, lists and reads).
fn board_changes_tool() -> ToolDef {
    let mut t = tool(BOARD_CHANGES, "What changed on the Hersir board since `since`, the seq an earlier view_read or board_changes returned: seq, \
        the counter now, to call with next time, and cards, the identifiers of the cards that changed since (their column, fields, labels \
        or hold, a comment, a run or a file). board: true when columns, labels, agents, people, projects or teams changed too. reload: \
        true when too much changed, or since is from before a restore: read the view again. Without since: only the counter, to start from.",
        &[("since", "integer", "The seq an earlier view_read or board_changes returned; leave it out on the first call")], &[]);
    t.read_only = true;
    t
}

/// GA-338: why a call over MCP whose token is for no chat, board check or question can't use a tool that changes
/// something (`call_scoped`): none of the rules that make such a call wait for the user's Approve would hold. GA-341:
/// Claude Desktop's token is the one exception (`call_desktop`).
pub const ONLY_READS: &str = "{tool} isn't available with this token: it is for no chat, board check or question, so it can only read \
    your Hersir board.";

/// GA-341: why Claude Desktop can't start a chat in Hersir: it asks the user in its own chat. Word for word as the card has it.
pub const START_CHAT_IN_CLAUDE_DESKTOP: &str = "start_chat isn't available in Claude Desktop: ask the user here, in this chat.";

/// GA-341: why Claude Desktop can't update a project's linked folder: update_checkout needs a chat in Hersir, where its
/// result comes as a message (`write::update_checkout`). Word for word as the card has it.
pub const UPDATE_CHECKOUT_IN_CLAUDE_DESKTOP: &str = "update_checkout isn't available in Claude Desktop: it works only in Hersir's chat, \
    where its result comes as a message.";

/// GA-341: the tools of the Chief's chat turns that Claude Desktop isn't offered, each with its answer when it is called anyway.
const NOT_IN_CLAUDE_DESKTOP: [(&str, &str); 2] = [("start_chat", START_CHAT_IN_CLAUDE_DESKTOP), ("update_checkout", UPDATE_CHECKOUT_IN_CLAUDE_DESKTOP)];

/// GA-341: a tool only Claude Desktop is offered (`desktop_catalog`), not the Chief's own turns: what the Chief works by
/// (`chief_brief`). Its name starts with get_, so `ToolDef::read_only` marks it.
pub const CHIEF_BRIEF: &str = "get_chief_brief";

/// GA-341: what get_chief_brief starts with, for the user named `you`, word for word as the card has it; the Chief's
/// instructions follow (`chief_brief`).
pub fn chief_brief_line(you: &str) -> String {
    format!("You are working as Hersir's Chief for {you}, from Claude Desktop. Hersir's tools act at once, like the Chief's do in Hersir's \
             chat. Leave out what only fits a run on a card, like the GIZAI_RESULT line. Look things up in Hersir's memory (memory_search) \
             before you ask the user.")
}

/// GA-341: get_chief_brief's answer, as plain text: `chief_brief_line` for the user, a blank line, then the Chief's
/// instructions as the Chief's chat gets them (`chat::system_prompt`): the instructions in the agent form of the agent with
/// Chat on, as they are, else (none written, or no agent has Chat on) the Chief's standard ones, its role's template.
fn chief_brief(st: &AppState) -> Result<Value, String> {
    let you = gizai_core::users::list(&st.db).map_err(err)?.into_iter().find(|p| p.id == st.you_id).map(|p| p.name)
        .unwrap_or_else(|| "the user".into());
    let instructions = gizai_core::team::chat_agent(&st.db).map_err(err)?.and_then(|chief| chief.instructions_md)
        .filter(|i| !i.trim().is_empty()).unwrap_or_else(|| gizai_core::seed::role_template("lead"));
    Ok(Value::String(format!("{}\n\n{instructions}", chief_brief_line(&you))))
}

/// GA-341: a call from Claude Desktop (`GizaiTools::desktop`). It runs as a call from a Chief's chat turn in a chat that
/// used no tool from outside Hersir, as the token's actor (you), so its activity and comments show your name. Hersir can't
/// see what else Claude used in its chat, so the rules about tools from outside Hersir (GA-101, GA-261, GA-319) don't
/// apply; every other rule does, whoever the actor is: the hold that waits for your OK (`ONLY_THE_USER`), new agent
/// commands (GA-323: with no chat to keep the call in, it isn't run, `Cx::keep_commands`), attach_file (only the Chief's
/// copies of the code, `write::allowed_attachment`) and merge_pull_request's rules. It approves no kept call: those stay
/// yours, in Hersir. start_chat and update_checkout answer why they aren't offered, and get_chief_brief is its own; so are
/// ask_chief and chief_reply (GA-342, `desktop`), which hand a message to the Chief in Hersir's chat and read its answer
/// (as is board_changes, GA-344, answered before this in `GizaiTools::call_with_meta`).
async fn call_desktop(st: &AppState, actor: &str, from: Stream<'_>, name: &str, args: Value) -> Result<Value, String> {
    if let Some((_, why)) = NOT_IN_CLAUDE_DESKTOP.iter().find(|(tool, _)| *tool == name) {
        return Err(why.to_string());
    }
    match name {
        CHIEF_BRIEF => chief_brief(st),
        desktop::ASK_CHIEF => desktop::ask_chief(st, &args_of(args)?).await,
        desktop::CHIEF_REPLY => desktop::chief_reply(st, &args_of(args)?).await,
        _ => call_scoped(st, actor, None, None, Some(from), name, args, false, true).await,
    }
}

/// A tool that only reads (`ToolDef::read_only`): the gets, lists and reads, check_board and the memory reads.
fn reads_only(name: &str) -> bool {
    catalog().iter().any(|t| t.name == name && t.read_only)
}

/// The chat answer a call over MCP comes from: its run, and the tool use the client named for the call, if it did.
#[derive(Clone, Copy)]
struct Stream<'a> {
    run: &'a str,
    tool_use: Option<&'a str>,
}

/// Runs one tool as `actor`, outside any chat thread. Errors are plain sentences for the model to act on.
pub async fn call(st: &AppState, actor: &str, name: &str, args: Value) -> Result<Value, String> {
    call_in(st, actor, None, name, args).await
}

/// Runs one tool as `actor` for a chat thread (what the user said there widens `attach_file`). Called directly, not by
/// an answer's Claude Code, so no stream shows it: only a thread already marked refuses the tools that act.
pub async fn call_in(st: &AppState, actor: &str, thread: Option<&str>, name: &str, args: Value) -> Result<Value, String> {
    call_scoped(st, actor, thread, None, None, name, args, false, false).await
}

/// Runs one tool as `actor` in the board check `run_id`: nobody named a file, and the check's rules hold.
pub async fn call_check(st: &AppState, actor: &str, run_id: &str, name: &str, args: Value) -> Result<Value, String> {
    call_scoped(st, actor, None, Some(run_id), None, name, args, false, false).await
}

/// Runs a call you approved (GA-102, `pending::approve`): exactly the stored call, as `actor` (the Chief) in its chat
/// `thread`, with every rule in force but the ones about a tool from outside Gizai in that chat: it isn't kept again, a card
/// it changes doesn't wait on hold for your OK (GA-101), and it may take that hold off (GA-261).
pub(crate) async fn call_approved(st: &AppState, actor: &str, thread: &str, name: &str, args: Value) -> Result<Value, String> {
    call_scoped(st, actor, Some(thread), None, None, name, args, true, false).await
}

/// What a board check may not do, whatever it is asked: attach a file nobody named, change an agent's settings, set up
/// the board's columns, or archive or restore cards (GA-307) (the check asks you instead).
const NOT_IN_A_CHECK: [&str; 8] = ["attach_file", "create_agent", "update_agent", "set_agent_status", "add_column", "set_column",
    "archive_tasks", "restore_tasks"];

/// What a chat may no longer do once it used a tool from outside Gizai (an MCP server of its own, the web, the browser)
/// (`chat::used_outside`; GA-261: for the rest of the chat, as its session holds what that tool returned): such a call isn't
/// run but kept under the answer for the user to approve with one click or discard (GA-102, `pending`). Memory's writes
/// too: a note goes into every answer, board check and run after it. A board check uses no tool from outside Gizai
/// (`chat::check_once`), so it never makes a pending action. Merging a pull request too (GA-86); a board check may merge.
/// Archiving cards too (GA-307): restore_tasks, which only undoes an archive, runs at once.
/// Comments too (GA-261): an agent hears them, also in a Continue you approve. A card such a chat puts where an agent would
/// start it, or changes there, waits on hold for the user's OK instead (GA-101, `Cx::outside`); some calls of
/// `KEPT_SOMETIMES` are kept too. GA-319: start_chat runs, but the chat its message goes to is marked with the same tool,
/// so what the Chief does there waits for the user's Approve too (`write::start_chat`).
pub const NOT_AFTER_OUTSIDE: [&str; 15] = ["start_agent_run", "continue_agent_run", "create_agent", "update_agent", "set_agent_status", "add_column",
    "set_column", "attach_file", "update_checkout", "memory_write", "memory_append", "memory_move", "merge_pull_request", "archive_tasks",
    "comment_on_task"];

/// GA-261: the tools that keep some of their calls for the user's Approve after a tool from outside Gizai in the chat, as
/// `NOT_AFTER_OUTSIDE` keeps all of theirs (`Cx::keep`): `update_project` one that changes a project's repository, folder,
/// main branch or goal (new cards fetch and run that code, and every agent's prompt carries the goal), `update_task` one
/// that changes the description, acceptance criteria, assignee or builds_on of a card outside an Auto column (in an Auto
/// column the card waits on hold instead, GA-101). GA-319: `update_task` one that changes a card's title the same way, and
/// one that changes its hold's reason in any column (a board check reads it); `create_project` one that gives a repository,
/// folder, main branch or goal (`write::STEERS_PROJECT`).
pub const KEPT_SOMETIMES: [&str; 3] = ["update_project", "update_task", "create_project"];

/// GA-261: why a call that would take GA-101's hold off a card (`Task::waits_for_your_ok`) is refused, from a board check or
/// a chat answer: `update_task` with `clear_hold` (or another hold), `continue_agent_run`, `start_agent_run`. Only the
/// user takes it off: on the card, or by approving a call that waited for them.
pub const ONLY_THE_USER: &str = "Only the user takes this hold off, on the card: it waits for their OK after a tool from outside Hersir.";

/// How long a call of a tool in `NOT_AFTER_OUTSIDE` in a chat answer waits for the answer's stream to show it
/// (`chat::wait_shown`). Not shown by then, it is refused, and the model can call it again.
pub const SHOWN_WAIT: std::time::Duration = std::time::Duration::from_secs(5);

/// Why a call in `NOT_AFTER_OUTSIDE` wasn't run after `tool`, a tool from outside Gizai in this chat: it waits for the user's
/// click (`kept`), or, when Gizai couldn't keep it (`Err`), the user does it themselves (GA-261: a new message in the chat
/// would be kept again).
fn not_run(name: &str, tool: &str, kept: Result<(), String>) -> String {
    match kept {
        Ok(()) => format!("{name} wasn't run: this chat used {tool}, a tool from outside Hersir. Hersir keeps the call for the user to approve \
            with one click. Tell them in a sentence what it does."),
        Err(e) => format!("{name} wasn't run: this chat used {tool}, a tool from outside Hersir, and Hersir couldn't keep the call for the user \
            to approve ({e}). Tell the user what you would do, so they can do it themselves in Hersir."),
    }
}

/// GA-323: why create_agent or update_agent (`name`) wasn't run: it gives `who` the commands `entries`, which chat doesn't
/// give an agent at once (`write::new_commands`), naming them. It waits for the user's click (`kept`), or, when Gizai couldn't
/// keep it (`Err`), nothing changed and the user adds them themselves.
fn not_run_for_commands(name: &str, who: &str, entries: &[String], kept: Result<(), String>) -> String {
    let list = preview::listed(entries, entries.len());
    let (is, them) = if entries.len() == 1 { ("is", "it") } else { ("are", "them") };
    let why = format!("{name} wasn't run: {list} {is} new for {who}, and from chat an agent gets a command it doesn't have at once only \
                       when it is one of Hersir's default list that runs no other command and writes no file.");
    match kept {
        Ok(()) => format!("{why} Hersir keeps the call for the user to approve with one click. Tell them in a sentence what it does, and \
                           name {list}."),
        Err(e) => format!("{why} Hersir couldn't keep the call for the user to approve ({e}), so nothing changed. Tell the user, so they can \
                           add {them} themselves in the agent form (Permissions → Allowed commands)."),
    }
}

/// Why a tool that acts is refused after a tool from outside Gizai in the chat (GA-261: for the rest of the chat).
pub(crate) fn refused_after(name: &str, tool: &str) -> String {
    format!("{name} is refused in this chat: it used {tool}, a tool from outside Hersir, and what that returned could be trying to \
        steer you. Tell the user what you would do, so they can do it themselves in Hersir.")
}

/// A call can come in before Gizai has read the chat answer's stream up to it, so before an outside tool in the same
/// message is marked: it waits until the stream shows the call itself (`chat::wait_shown`). Not shown in time, it is
/// refused and nothing changes. A call no stream shows (made directly), or in a chat already marked, doesn't wait.
async fn shown(st: &AppState, thread: &str, from: Option<Stream<'_>>, name: &str, a: &Args) -> Result<(), String> {
    if let Some(s) = from.filter(|_| crate::chat::used_outside(st, thread).is_none())
        && crate::chat::wait_shown(st, s.run, s.tool_use, name, &a.0, SHOWN_WAIT).await != crate::chat::Shown::Yes {
        return Err(format!("{name} is refused this time: Hersir couldn't see this call in your answer in time, so it can't tell \
            whether a tool from outside Hersir came before it. Nothing changed: call it again."));
    }
    Ok(())
}

/// `desktop`: the call comes from Claude Desktop (GA-341, `call_desktop`, the only caller that sets it).
#[allow(clippy::too_many_arguments)]
async fn call_scoped(st: &AppState, actor: &str, thread: Option<&str>, check: Option<&str>, from: Option<Stream<'_>>, name: &str, args: Value,
                     approved: bool, desktop: bool) -> Result<Value, String> {
    // GA-338: fail closed. A call over MCP (`from`) with no chat thread and no board check would skip every rule that makes
    // a call wait for the user's Approve (`Cx::outside` is None without a thread), so only a tool that reads runs there,
    // whatever else its token's scope says: a question's (refused before this already, `IN_A_QUESTION`), or any other.
    // GA-341: Claude Desktop's token is the one exception, by its client's name (`GizaiTools::desktop`): its calls run as a
    // Chief's chat turn's in a chat that used no tool from outside Hersir (`call_desktop`). The calls Hersir makes itself
    // (`call`, `call_in`, `call_check`, `call_approved`) have no `from`.
    if from.is_some() && thread.is_none() && check.is_none() && !desktop && !reads_only(name) {
        return Err(ONLY_READS.replace("{tool}", name));
    }
    let a = args_of(args)?;
    if check.is_some() && NOT_IN_A_CHECK.contains(&name) {
        return Err(match name {
            "attach_file" => "In a board check nobody named a file, so nothing can be attached. Ask the user in a chat (start_chat) instead.".into(),
            "archive_tasks" => "A board check never archives cards: ask the user in a chat (start_chat) instead.".into(),
            "restore_tasks" => "A board check never restores cards: ask the user in a chat (start_chat) instead.".into(),
            _ => format!("{name} can't be used in a board check: never change an agent's settings or the columns there. Ask the user in a chat (start_chat) instead."),
        });
    }
    // GA-307: archive_tasks without exactly one of tasks and all_done can never work: refused at once, so it doesn't wait
    // for the user's Approve after a tool from outside Hersir.
    if name == "archive_tasks" {
        write::archive_scope(&a)?;
    }
    let cx = Cx { st, actor, thread, check, from, approved, desktop };
    // A call you approved (GA-102) was kept because of that rule, so it doesn't apply to it; every other rule does.
    if NOT_AFTER_OUTSIDE.contains(&name) && let Some(tool) = cx.outside(name, &a).await? {
        // Not run: kept under the answer for the user to approve with one click, or discard (GA-102).
        return Err(cx.keep(name, &a, &tool));
    }
    let out = match name {
        "get_overview" => read::overview(&cx),
        "read_inbox" => read::inbox(&cx),
        "check_board" => crate::board::check_json(st),
        "start_chat" => write::start_chat(&cx, &a).await,
        "continue_agent_run" => write::continue_run(&cx, &a).await,
        "list_clients" => read::list_clients(&cx, &a),
        "get_client" => read::get_client(&cx, &a).await,
        "list_projects" => read::list_projects(&cx, &a),
        "get_project" => read::get_project(&cx, &a).await,
        "list_tasks" => read::list_tasks(&cx, &a),
        "get_task" => read::get_task(&cx, &a).await,
        "list_agents" => read::list_agents(&cx, &a),
        "get_agent" => read::get_agent(&cx, &a),
        "list_docs" => read::list_docs(&cx, &a),
        "read_doc" => read::read_doc(&cx, &a),
        "list_people" => read::list_people(&cx),
        "get_workflow" => read::workflow(&cx, &a),
        "create_client" => write::create_client(&cx, &a),
        "update_client" => write::update_client(&cx, &a),
        "save_contact" => write::save_contact(&cx, &a),
        "create_project" => write::create_project(&cx, &a).await,
        "update_project" => write::update_project(&cx, &a).await,
        "create_task" => write::create_task(&cx, &a).await,
        "update_task" => write::update_task(&cx, &a).await,
        "move_task" => write::move_task(&cx, &a).await,
        "archive_tasks" => write::archive_tasks(&cx, &a),
        "restore_tasks" => write::restore_tasks(&cx, &a),
        "comment_on_task" => write::comment(&cx, &a),
        "create_agent" => write::create_agent(&cx, &a).await,
        "update_agent" => write::update_agent(&cx, &a).await,
        "set_agent_status" => write::set_agent_status(&cx, &a),
        "add_column" => write::add_column(&cx, &a),
        "set_column" => write::set_column(&cx, &a),
        "save_label" => write::save_label(&cx, &a),
        "start_agent_run" => write::start_run(&cx, &a).await,
        "stop_agent_run" => write::stop_run(&cx, &a),
        "create_doc" => write::create_doc(&cx, &a),
        "write_doc" => write::write_doc(&cx, &a),
        "memory_list" => memory::list(&cx, &a),
        "memory_search" => memory::search(&cx, &a),
        "memory_read" => memory::read(&cx, &a),
        "memory_write" => memory::write(&cx, &a),
        "memory_append" => memory::append(&cx, &a),
        "memory_move" => memory::move_note(&cx, &a),
        "attach_file" => write::attach_file(&cx, &a).await,
        "add_person" => write::add_person(&cx, &a),
        "update_checkout" => write::update_checkout(&cx, &a).await,
        "merge_pull_request" => merge::merge_pull_request(&cx, &a).await,
        other => Err(format!("unknown tool {other}")),
    };
    // GA-102: the same call made and run here (in a later answer, say after the user said yes in a message) no longer
    // waits for the user's OK, so approving the kept one can't make it twice.
    if let Some(t) = thread.filter(|_| out.is_ok() && (NOT_AFTER_OUTSIDE.contains(&name) || KEPT_SOMETIMES.contains(&name))) {
        crate::pending::made(st, t, name, &a.0);
    }
    out
}

/// What every tool gets: the app and the acting agent.
pub(crate) struct Cx<'a> {
    pub st: &'a AppState,
    pub actor: &'a str,
    /// The chat thread the call comes from, if any.
    pub thread: Option<&'a str>,
    /// The board check run the call comes from, if any.
    pub check: Option<&'a str>,
    /// The answer's stream a call over MCP comes from; None for a call made directly.
    from: Option<Stream<'a>>,
    /// A call the user approved (GA-102, `call_approved`): the rules about a tool from outside Gizai don't apply to it.
    pub approved: bool,
    /// GA-341: a call from Claude Desktop (`call_desktop`), as you: the rules of a Chief's chat turn in a chat that used no
    /// tool from outside Hersir, with no chat to keep a call in or to name a file in.
    pub desktop: bool,
}

impl Cx<'_> {
    /// GA-101: the tool from outside Gizai this chat used before the call `name`, if any (`chat::used_outside`; GA-261: in
    /// this answer or an earlier one); None outside a chat, and for a call the user approved. Like a tool in
    /// `NOT_AFTER_OUTSIDE`, a call from an answer's Claude Code first waits until the stream shows it, so an outside tool
    /// earlier in the same message counts too. Asked at most once per call: a call the stream showed is taken.
    pub async fn outside(&self, name: &str, a: &Args) -> Result<Option<String>, String> {
        let Some(t) = self.thread.filter(|_| !self.approved) else { return Ok(None) };
        shown(self.st, t, self.from, name, a).await?;
        Ok(crate::chat::used_outside(self.st, t))
    }

    /// Keeps the call `name` under the answer for the user to approve with one click or discard (GA-102, `pending::keep`),
    /// as this chat used `tool`, a tool from outside Gizai (`outside`): what the call returns, for the Chief.
    pub fn keep(&self, name: &str, a: &Args, tool: &str) -> String {
        let kept = match self.thread {
            Some(t) => crate::pending::keep(self.st, self.actor, t, self.from.map(|s| s.run), name, &a.0, tool).map(|_| ()),
            None => Err("there is no chat to keep it in".into()),
        };
        not_run(name, tool, kept)
    }

    /// GA-323: keeps the call `name` (create_agent or update_agent) for the user's Approve, as it gives the agent `who` the
    /// commands `entries`, which chat doesn't give at once (`write::new_commands`): pending as a call after a tool from
    /// outside Gizai is (`pending::keep`), marked `NEW_COMMANDS` instead of a tool. What the call returns, for the Chief.
    pub fn keep_commands(&self, name: &str, a: &Args, who: &str, entries: &[String]) -> String {
        let kept = match self.thread {
            Some(t) => crate::pending::keep(self.st, self.actor, t, self.from.map(|s| s.run), name, &a.0, gizai_core::pending::NEW_COMMANDS)
                .map(|_| ()),
            None => Err("there is no chat to keep it in".into()),
        };
        not_run_for_commands(name, who, entries, kept)
    }
    pub fn db(&self) -> &gizai_core::db::Db {
        &self.st.db
    }
    pub fn changed(&self, table: &'static str) {
        (self.st.notify)(Note::RowsChanged(table));
    }
    /// After a card changes (it landed in an Auto column, or was assigned), the agents may start on it.
    pub fn wake(&self, task_id: &str) {
        if tokio::runtime::Handle::try_current().is_err() {
            return;
        }
        let (st, id) = (self.st.clone(), task_id.to_string());
        tokio::spawn(async move { crate::runs::dispatch(&st, &id).await; });
    }
}

pub(crate) fn err(e: gizai_core::Error) -> String {
    match e {
        gizai_core::Error::NotFound(what) => format!("{what} was not found"),
        other => other.to_string(),
    }
}

/// The tool's arguments, with the readers every tool uses.
pub(crate) struct Args(Map<String, Value>);

/// A call's arguments: an object, or none.
fn args_of(args: Value) -> Result<Args, String> {
    match args {
        Value::Object(m) => Ok(Args(m)),
        Value::Null => Ok(Args(Map::new())),
        _ => Err("arguments must be an object".into()),
    }
}

impl Args {
    /// A string that must be there and not empty.
    pub fn req(&self, k: &str) -> Result<String, String> {
        self.text(k).filter(|s| !s.is_empty()).ok_or_else(|| format!("missing \"{k}\""))
    }
    /// A string, trimmed; Some("") when given empty (an update clears it), None when absent.
    pub fn text(&self, k: &str) -> Option<String> {
        match self.0.get(k)? {
            Value::String(s) => Some(s.trim().to_string()),
            Value::Number(n) => Some(n.to_string()),
            Value::Null => None,
            other => Some(other.to_string()),
        }
    }
    /// A string that is there and not empty.
    pub fn opt(&self, k: &str) -> Option<String> {
        self.text(k).filter(|s| !s.is_empty())
    }
    pub fn int(&self, k: &str) -> Result<Option<i64>, String> {
        match self.0.get(k) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::Number(n)) => n.as_i64().or_else(|| n.as_f64().map(|f| f.round() as i64)).map(Some).ok_or_else(|| format!("\"{k}\" must be a whole number")),
            Some(Value::String(s)) if s.trim().is_empty() => Ok(None),
            Some(Value::String(s)) => s.trim().parse().map(Some).map_err(|_| format!("\"{k}\" must be a whole number")),
            _ => Err(format!("\"{k}\" must be a whole number")),
        }
    }
    pub fn num(&self, k: &str) -> Result<Option<f64>, String> {
        match self.0.get(k) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::Number(n)) => Ok(n.as_f64()),
            Some(Value::String(s)) if s.trim().is_empty() => Ok(None),
            Some(Value::String(s)) => s.trim().replace(',', ".").parse().map(Some).map_err(|_| format!("\"{k}\" must be a number")),
            _ => Err(format!("\"{k}\" must be a number")),
        }
    }
    pub fn flag(&self, k: &str) -> Option<bool> {
        match self.0.get(k)? {
            Value::Bool(b) => Some(*b),
            Value::String(s) => Some(matches!(s.trim().to_lowercase().as_str(), "true" | "yes" | "1")),
            _ => None,
        }
    }
    /// A list of strings: a JSON array, or one comma-separated string.
    pub fn list(&self, k: &str) -> Option<Vec<String>> {
        let items: Vec<String> = match self.0.get(k)? {
            Value::Array(v) => v.iter().filter_map(|x| x.as_str().map(|s| s.trim().to_string())).collect(),
            Value::String(s) => s.split(',').map(|x| x.trim().to_string()).collect(),
            _ => return None,
        };
        Some(items.into_iter().filter(|s| !s.is_empty()).collect())
    }
}

/// A JSON Schema for a tool's arguments. Each property: (name, type, description); type is string,
/// integer, number, boolean, string[] or `enum:a|b|c`.
fn schema(props: &[(&str, &str, &str)], required: &[&str]) -> Value {
    let mut p = Map::new();
    for (name, ty, desc) in props {
        let v = match *ty {
            "string[]" => json!({"type": "array", "items": {"type": "string"}, "description": desc}),
            t if t.starts_with("enum:") => json!({"type": "string", "enum": t[5..].split('|').collect::<Vec<_>>(), "description": desc}),
            t => json!({"type": t, "description": desc}),
        };
        p.insert(name.to_string(), v);
    }
    json!({"type": "object", "properties": p, "required": required})
}

fn tool(name: &str, description: &str, props: &[(&str, &str, &str)], required: &[&str]) -> ToolDef {
    let read_only = name.starts_with("get_") || name.starts_with("list_") || name.starts_with("read_") || name == "check_board"
        || matches!(name, "memory_list" | "memory_search" | "memory_read");
    ToolDef { name: name.into(), description: description.into(), input_schema: schema(props, required), read_only, meta: None }
}

const TASK: (&str, &str, &str) = ("task", "string", "The task's identifier, like KADE-12 (or its id)");
/// get_task's task: it finds an archived task too, and what its builds_on and needed_by are. Said here, not in get_task's
/// description, to keep that short (GA-310; GA-311 moved builds_on and needed_by here to make room for the PDF copies).
/// GA-332's agent_time sentence, word for word as its card asks, takes it past 600 characters.
const TASK_OR_ARCHIVED: (&str, &str, &str) = ("task", "string",
    "The task's identifier, like KADE-12 (or its id). Finds an archived task too, and says so (archived: true): it is read-only until the user restores it. Its builds_on lists the cards it builds on, each with its column and merged: true once in Deploy or Done; needed_by the cards that build on it");
const PROJECT: (&str, &str, &str) = ("project", "string", "The project's key (KADE), name or id");
const CLIENT: (&str, &str, &str) = ("client", "string", "The client's name or id");
const AGENT: (&str, &str, &str) = ("agent", "string", "The agent's name or id");
/// GA-225: the team add_column, set_column and create_agent act on.
const TEAM: (&str, &str, &str) = ("team", "string", "The team's name or id. Needed when there is more than one team; with one, it may be left out");
/// GA-228: create_task's builds_on.
const BUILDS_ON: (&str, &str, &str) = ("builds_on", "string[]",
    "Identifiers of the cards it builds on (it needs their code), like [\"GA-226\"]: it waits until each is merged (in Deploy or Done), then starts by itself. At most 20");

const CLIENT_FIELDS: [(&str, &str, &str); 14] = [
    ("legal_name", "string", "Registered name"), ("kind", "enum:company|person", "A company or a private person"),
    ("email", "string", "Email"), ("phone", "string", "Phone"), ("website", "string", "Website"),
    ("street", "string", "Street and number"), ("postal_code", "string", "Postal code"), ("city", "string", "City"),
    ("country", "string", "Country code, like NL"), ("vat_number", "string", "VAT number"), ("coc_number", "string", "Chamber of Commerce number"),
    ("iban", "string", "IBAN"), ("payment_terms_days", "integer", "Payment terms in days"), ("notes_md", "string", "Notes (Markdown)"),
];

const AGENT_FIELDS: [(&str, &str, &str); 11] = [
    ("board_check_minutes", "integer", "Only for the agent with Chat on (the Chief): check the board every this many minutes (5–1440); 0 = off"),
    ("runs_on", "string", "The coding CLI it runs on, by name as Settings → Coding CLIs lists them (Claude Code, Codex, Gemini, a second account, …). Omit: Claude Code for a new agent, unchanged on update"),
    ("model", "string", "On Claude Code, a model it offers: an alias (default, opus, sonnet, haiku, fable) or its full id (claude-opus-5-5). On Codex or Gemini, that CLI's model name. Empty = the CLI's default"),
    ("effort", "string", "How hard the model thinks. Claude Code: low, medium, high, xhigh or max. Codex: minimal, low, medium, high or xhigh. Gemini takes none. Empty = the CLI's default; higher costs more"),
    ("instructions_md", "string", "Instructions sent with every run (Markdown). Omit on create to use the role's template; it must end by asking for the GIZAI_RESULT line"),
    ("cards_at_once", "integer", "How many cards it works on at the same time, each in its own git worktree (1–10, default 1)"),
    ("permission_mode", "string", "Permission mode for task runs, in its CLI's terms. Claude Code: acceptEdits (the usual), dontAsk, auto, plan or manual. Codex: workspace-write (the usual) or read-only. Gemini: auto_edit (the usual), plan or default"),
    ("allowed_tools", "string[]", "Commands it may run without asking, each its own entry as Bash(…), like Bash(npm test:*). Never a shell, an interpreter or a path, like Bash(bash:*), Bash(python3:*) or Bash(/bin/sh:*), unless the agent has it already or its role's list has it: only the user adds those. A command the agent doesn't have yet waits for the user's Approve, the whole call with it, unless it is one of Bash(git status:*), Bash(git add:*), Bash(ls:*), Bash(cat:*), Bash(head:*), Bash(tail:*), Bash(wc:*), Bash(cut:*), Bash(diff:*), Bash(grep:*), Bash(jq:*), Bash(pwd:*), Bash(which:*) and Bash(sleep:*). Omit on create for its role's list (builders: Hersir's default list plus git push, pull and fetch; QA also gh pr; DevOps its release commands), saved at once; an empty list on update = Hersir's default list"),
    ("monthly_budget_usd", "number", "Monthly spending cap in dollars; empty = no cap"),
    ("title", "string", "Job title shown on the team page"),
    ("shares_memory_with", "string", "Memory: the agent whose memory folder it shares, by name, like Backend Agent for Backend Agent 2 (their runs get that folder's notes and their learned lines go there). An agent that shares a folder itself stands for that folder's owner; never the Chief. \"none\" = its own folder. Joining moves its notes into that folder; leaving gives it a fresh Notes. Omit: its own folder for a new agent, unchanged on update"),
];

fn with<const N: usize>(head: &[(&'static str, &'static str, &'static str)], rest: [(&'static str, &'static str, &'static str); N]) -> Vec<(&'static str, &'static str, &'static str)> {
    head.iter().copied().chain(rest).collect()
}

/// GA-341: what Claude Desktop is offered (`GizaiTools::desktop`): get_chief_brief first, to read before it changes the
/// board; GA-342: then ask_chief and chief_reply, to ask Hersir's Chief itself (`desktop`); then every tool of the Chief's
/// chat turns (`catalog`) but start_chat and update_checkout (`NOT_IN_CLAUDE_DESKTOP`). The Chief's own turns, board
/// checks and questions get `catalog`, without these three. GA-345: board_changes (GA-344) is no longer here but one of the
/// view's tools (`apps::tools`), which a Claude Desktop that shows MCP Apps' views gets after these (`GizaiTools::list_in`).
pub fn desktop_catalog() -> Vec<ToolDef> {
    let brief = tool(CHIEF_BRIEF, "What Hersir's Chief works by: its instructions. Read it before you change the board, and follow it as the \
                                   Chief does.", &[], &[]);
    [brief, desktop::ask_chief_def(), desktop::chief_reply_def()].into_iter()
        .chain(catalog().into_iter().filter(|t| !NOT_IN_CLAUDE_DESKTOP.iter().any(|(name, _)| *name == t.name)))
        .collect()
}

/// Every tool with its description and argument schema: what the Chief's chat turns are offered.
pub fn catalog() -> Vec<ToolDef> {
    vec![
        tool("get_overview", "A summary of the organisation: clients, active projects, tasks per column, how many items wait in the inbox, the agents and who is working now. Start here.", &[], &[]),
        tool("read_inbox", "What needs the user: open tasks on hold (an agent or a gate needs a person), tasks waiting for them in Review or Deploy, and the chats you started that wait for their answer.", &[], &[]),
        tool("check_board", "What on the board needs attention now: answered cards (a person commented after a needs-a-decision hold), held cards, cards no agent will start (and why; builds_on_blocked: a card it builds on is on hold or cancelled), cards whose run stopped part-way, and cards in Review that QA passed in a project that lets you merge (merge_pull_request); then per agent its cards at once, the cards it works on, its free slots and whether its pull is paused (and why), and the free \"Runs at once\".", &[], &[]),
        tool("start_chat", "Asks the user something in a new chat that waits at the top of their Inbox: a question, or an approval you need. One waiting chat per card: when a card already has one, the message is added to that chat. Returns a link to it.",
             &[("title", "string", "A short title, like \"GA-12: pick the export format\""), ("kind", "enum:question|approval", "question, or approval for something you want to do"),
               ("tasks", "string[]", "The identifiers of the cards it is about, like GA-12"), ("body_md", "string", "Your first message (Markdown): what you found, what you recommend and what you need")],
             &["title", "kind", "tasks", "body_md"]),
        tool("continue_agent_run", "Continues the agent's latest run on a task, like the Continue button: it resumes the run's session in its worktree, for a run that stopped part-way (a limit, a failure, stopped) or one that asked for a decision that has been answered on the card since (the agent hears the comments written since). A note goes to the agent with it and is saved on the card as your comment; on a run that asked for a decision it counts as an answer. A hold on the card is cleared, but for one that waits for the user's OK after a tool from outside Hersir (waiting_for_ok in get_task): only the user takes that off.",
             &[TASK, ("note", "string", "Optional: what the agent should know as it continues, like \"use the existing CSV writer\"")], &["task"]),
        tool("list_clients", "All clients with city, main contact and counts of projects and open tasks.", &[("status", "enum:lead|active|inactive", "Only clients with this status")], &[]),
        tool("get_client", "One client with every field, its contacts, projects and files. Each file comes with the path of a copy in your folder, to read with Read (null for a file over 100 MB or one that couldn't be copied: its note or error says why). A Word, Excel, PowerPoint, OpenDocument or RTF file also has readable_path: a copy of its text as Markdown, to read with Read. Pages, Keynote, Numbers, .doc and .ppt files get a PDF copy instead, when LibreOffice is installed.", &[CLIENT], &["client"]),
        tool("list_projects", "Projects with key, client, status, team (whose board its cards are on), linked repository, task counts and Test runs at once (null: no limit of its own).", &[("client", "string", "Only this client's projects"), ("status", "enum:planned|active|paused|done|archived", "Only projects with this status")], &[]),
        tool("get_project", "One project with its goal, team (whose board its cards are on), repository, Test runs at once, tasks per column, docs and files. Each file comes with the path of a copy in your folder, to read with Read (null for a file over 100 MB or one that couldn't be copied: its note or error says why). A Word, Excel, PowerPoint, OpenDocument or RTF file also has readable_path: a copy of its text as Markdown, to read with Read. Pages, Keynote, Numbers, .doc and .ppt files get a PDF copy instead, when LibreOffice is installed. time_tracking: its agents' minutes and tokens over all its cards, and for its cards in Done that had runs, the median and 90th percentile of agent minutes per card: use them to estimate new cards.", &[PROJECT], &["project"]),
        tool("list_tasks", "Tasks, newest columns first. Done and cancelled tasks are left out unless include_done is true. Archived tasks are always left out; archived: true lists only them, the most recently archived first. A task that waits for cards it builds on shows them in waits_for (not merged yet). Each task has agent_minutes and tokens: its agents' time and tokens over all its runs, as in get_task's agent_time.",
             &[("project", "string", "Project key, name or id"), ("column", "string", "Column name, like To do or In progress"),
               ("assignee", "string", "A person or agent name; \"none\" for unassigned; \"me\" for the user"), ("label", "string", "Label name"),
               ("text", "string", "Words in the title"), ("include_done", "boolean", "Include done and cancelled tasks"),
               ("archived", "boolean", "Only the archived tasks (archived from Done; read-only until the user restores them)"), ("limit", "integer", "At most this many (default 50)")], &[]),
        tool("get_task", "One task with its description, acceptance criteria, labels, hold, builds_on, needed_by, latest comments, agent runs (with the tool calls each was refused) and files. Each file comes with the path of a copy in your folder, to read with Read (null over 100 MB or if it couldn't be copied: its note or error says why). A Word, Excel, PowerPoint, OpenDocument or RTF file also has readable_path: a copy of its text as Markdown, to read with Read. Pages, Keynote, Numbers, .doc and .ppt files get a PDF copy instead, when LibreOffice is installed. agent_time: what the agents' runs on it took, added up over all its runs (each start, Continue and nudge is a run): the minutes from each run's start to its end (up to now while one works; a run cut off when Hersir closed counts at most the run time limit) and the tokens (input with cache, and output), as the project's Time tracking tab counts them. Each run has its own minutes and tokens.", &[TASK_OR_ARCHIVED], &["task"]),
        tool("list_agents", "The agents: role, status, the columns they are on, model, whether they work right now, spend this month and budget; with more than one team, each agent's team. An agent works only the columns of its own team.",
             &[("team", "string", "Only this team's agents: its name or id")], &[]),
        tool("get_agent", "One agent's settings, instructions and recent runs, with the card and the tool calls each run was refused.", &[AGENT], &["agent"]),
        tool("list_docs", "A project's docs (title, version, last change).", &[PROJECT], &["project"]),
        tool("read_doc", "A doc's current text (Markdown) and version.", &[("doc", "string", "The doc's title or id"), ("project", "string", "Project key or name, when titles repeat")], &["doc"]),
        tool("list_people", "The people in Hersir (the user and colleagues), with open task counts.", &[], &[]),
        tool("get_workflow", "The board's columns in order, each with its agents, Auto (its agents pick up its cards) or Manual (only Run starts one), its next column and what happens there; and the labels (tags for people: they never route; every team shares them). Each team has its own board, with the cards of its projects; with more than one team and no team given, each team's columns come under its name (teams).",
             &[("team", "string", "Only this team's columns: its name or id")], &[]),
        tool("create_client", "Adds a client.", &with(&[("name", "string", "Client name")], CLIENT_FIELDS), &["name"]),
        tool("update_client", "Changes a client. Only the fields given change; an empty string clears a field.",
             &with(&[CLIENT, ("name", "string", "New name"), ("status", "enum:lead|active|inactive", "Status")], CLIENT_FIELDS), &["client"]),
        tool("save_contact", "Adds a contact person to a client, or changes one (name it in contact).",
             &[CLIENT, ("name", "string", "Contact's name"), ("contact", "string", "An existing contact's name or id, to change it"),
               ("role", "string", "Job title"), ("email", "string", "Email"), ("phone", "string", "Phone"), ("is_primary", "boolean", "The client's main contact")], &["client", "name"]),
        tool("create_project", "Adds a project. The key (2–6 letters, used in task ids like KADE-12) is suggested from the name when omitted.",
             &[("name", "string", "Project name"), ("key", "string", "2–6 letters or digits, starting with a letter"), CLIENT,
               ("goal_md", "string", "The goal (Markdown)"), ("repo_path", "string", "Absolute path of the local git repository agents work in"),
               ("default_branch", "string", "The repository's main branch (default main)"),
               ("repository", "string", "The repository's link on GitHub (https://github.com/owner/name) or Bitbucket (https://bitbucket.org/workspace/repository), or another git URL; new cards then start from its main branch"),
               ("github", "string", "The same as repository (its old name)"),
               ("color", "string", "A colour like #7b9bff"),
               ("status", "enum:planned|active|paused|done|archived", "Status (default active)"),
               ("team", "string", "The team whose board its cards go on, by name or id (default the first team)")], &["name"]),
        tool("update_project", "Changes a project. Only the fields given change; an empty string clears a field. The key can't change.",
             &[PROJECT, ("name", "string", "New name"), CLIENT, ("goal_md", "string", "The goal (Markdown)"),
               ("team", "string", "Move it to this team, by name or id: all its cards (archived ones too) go to that team's first column of the same kind, else its first Backlog column, and agents of its old team are taken off them (people stay). Refused while one of its cards has a run live or starting"),
               ("repo_path", "string", "Absolute path of the local git repository"), ("default_branch", "string", "Main branch"),
               ("repository", "string", "The repository's link on GitHub (https://github.com/owner/name) or Bitbucket (https://bitbucket.org/workspace/repository), or another git URL"),
               ("github", "string", "The same as repository (its old name)"),
               ("color", "string", "A colour like #7b9bff"), ("status", "enum:planned|active|paused|done|archived", "Status"),
               ("test_runs_at_once", "integer", "Test runs at once: how many of its cards may be in a test run (a run on a card in a Testing column) at the same time, 1 to 20; 0 clears it (no limit of its own). A test run builds and runs the whole test suite: keep a heavy project (Rust, a large monorepo) to 1")], &["project"]),
        tool("create_task", "Adds a task to a project. In an Auto column the agents on that column pick it up; assign an agent to have only that agent start it. Labels are tags for people (any existing label; add one with save_label).",
             &[PROJECT, ("title", "string", "Short title: one line, at most 200 characters"), ("description_md", "string", "What to do and why (Markdown)"),
               ("acceptance_md", "string", "Acceptance criteria, as a Markdown checklist"), ("column", "string", "Column name (default the first, Backlog)"),
               ("priority", "integer", "0 none, 1 urgent, 2 high, 3 medium, 4 low"), ("assignee", "string", "A person or agent name; \"me\" for the user"),
               ("labels", "string[]", "Existing label names, like Must have or bug"),
               ("testing", "boolean", "On (default): the QA Agent tests it before Review. Off: straight to Review, for a small UI fix or bug fix"),
               BUILDS_ON], &["project", "title"]),
        tool("update_task", "Changes a task's fields, labels, assignee, hold or builds_on. Only what is given changes.",
             &[TASK, ("title", "string", "New title: one line, at most 200 characters"), ("description_md", "string", "Description (Markdown)"), ("acceptance_md", "string", "Acceptance criteria (Markdown)"),
               ("priority", "integer", "0 none, 1 urgent, 2 high, 3 medium, 4 low"), ("assignee", "string", "A person or agent name; \"none\" to unassign"),
               ("labels", "string[]", "The full new set of existing label names"), ("hold", "enum:needs_decision|blocked|stalled", "Put it on hold"),
               ("hold_reason", "string", "Why it is on hold"),
               ("clear_hold", "boolean", "Take it off hold. Not a card that waits for the user's OK after a tool from outside Hersir (waiting_for_ok in get_task): only the user takes that hold off"),
               ("testing", "boolean", "On: the QA Agent tests it before Review. Off: straight to Review"),
               ("builds_on", "string[]", "The full new list of cards it builds on, by identifier, like [\"GA-226\"]; an empty list clears it. It waits until each is merged (in Deploy or Done), then starts by itself. At most 20, never the card itself or a card that builds on it")], &["task"]),
        tool("move_task", "Moves a task to another column (to the bottom of that column).", &[TASK, ("column", "string", "Column name, like In progress")], &["task", "column"]),
        tool("archive_tasks", "Archives cards in Done: they leave the board and every list, and keep their ID, comments, runs and branch. list_tasks with archived: true lists them, and restore_tasks puts them back. Give tasks, or all_done for every card in Done. A card outside Done, or one an agent is working on, is skipped with why; the others are archived.",
             &[("tasks", "string[]", "The cards' identifiers, like [\"GA-12\"]"), ("all_done", "boolean", "Every card in the team's Done column instead"),
               ("project", "string", "With all_done: only this project's cards, by key, name or id"), TEAM], &[]),
        tool("restore_tasks", "Puts archived cards back at the bottom of their Done column, as they were.",
             &[("tasks", "string[]", "The archived cards' identifiers, like [\"GA-12\"]")], &["tasks"]),
        tool("comment_on_task", "Adds a comment to a task, as you.", &[TASK, ("body_md", "string", "The comment (Markdown)")], &["task", "body_md"]),
        tool("create_agent", "Adds an agent to a team, on Claude Code unless runs_on names another coding CLI. It starts from the role's instructions and allowed commands unless instructions_md or allowed_tools is given.",
             &with(&[("name", "string", "Agent name, like Frontend Agent"), ("role", "string", "Role key: lead, frontend, backend, design, qa, devops or your own"), TEAM], AGENT_FIELDS),
             &["name", "role"]),
        tool("update_agent", "Changes an agent's settings. Only the fields given change. An agent's folders (what its file tools may read or change), its MCP servers and their tools, web search, fetching pages, the browser and its CLI's built-in tools (slash commands and skills among them) are set only by the user, in the agent form.",
             &with(&[AGENT, ("name", "string", "New name"), ("role", "string", "Role key")], AGENT_FIELDS), &["agent"]),
        tool("set_agent_status", "Pauses an agent (no new cards; its running cards finish) or makes it active again.", &[AGENT, ("status", "enum:active|paused", "active or paused")], &["agent", "status"]),
        tool("add_column", "Adds a column to a team's board, right after another one, and optionally sets it up. Backlog, review, done and cancelled columns take no agents; an Auto column needs a next column.",
             &[TEAM, ("name", "string", "Column name, unique in the team"), ("after", "string", "The column it goes after, like Review"),
               ("kind", "enum:waiting|work|testing|review|deploy|done|backlog", "waiting (like To do: starting moves a card on), work (like In progress or Design), testing, review (you review and merge), deploy, done or backlog"),
               ("agents", "string[]", "The agents on it, by name, in order"), ("auto", "boolean", "true: its agents pick up its cards by themselves; false (default): only Run starts one"),
               ("next", "string", "The column its cards go to next")], &["name", "after", "kind"]),
        tool("set_column", "Sets up a column of a team's board: its agents (the full list, of that team), Auto or Manual, its next column, a new name or its place. Only what is given changes. Removing a column is done by the user in the app.",
             &[TEAM, ("column", "string", "The column's name"), ("agents", "string[]", "The full new list of agents on it, by name; empty takes them all off"),
               ("auto", "boolean", "true: its agents pick up its cards by themselves; false: only Run starts one"),
               ("next", "string", "The column its cards go to next; \"none\" for none"), ("new_name", "string", "A new name"),
               ("after", "string", "Move it right after this column")], &["column"]),
        tool("save_label", "Adds a label (a tag for people, like Must have; labels never route or start anything), or renames or recolours one (name it in label). Removing a label is done by the user in the app.",
             &[("name", "string", "The label's (new) name, unique ignoring case"), ("label", "string", "An existing label's name, to rename or recolour it"),
               ("color", "string", "A colour like #7b9bff")], &[]),
        tool("start_agent_run", "Starts an agent on a task now, like Run: the given agent, else the agent it is assigned to, else the first agent on its column. The project needs a linked git repository.",
             &[TASK, ("agent", "string", "Agent name; omit to use the assigned agent or the column's first agent")], &["task"]),
        tool("stop_agent_run", "Stops the agent working on a task right now.", &[TASK], &["task"]),
        tool("create_doc", "Adds a doc to a project, optionally with its first text.", &[PROJECT, ("title", "string", "Doc title"), ("body_md", "string", "Text (Markdown)")], &["project", "title"]),
        tool("write_doc", "Saves new text for a doc as a new version (the whole text, not a diff).",
             &[("doc", "string", "The doc's title or id"), ("body_md", "string", "The complete new text (Markdown)"), ("project", "string", "Project key or name, when titles repeat")], &["doc", "body_md"]),
        tool("memory_list", "The notes in Hersir's Memory, by path (folder and title), with version and size. Shared folders: Clients/, Projects/, Standards/, Workflows/, Deployments/, Dependencies/, Decisions/, Lessons/. Each agent has its own folder (Agents/<name>/), and you have Chief/.",
             &[("folder", "string", "Only the notes in this folder, like Standards or Agents/Backend Agent")], &[]),
        tool("memory_search", "Searches the memory notes: words and \"quoted phrases\" (case ignored), path:Folder to look in one folder (path:\"Chief\"), tag:name for a tag. Best matches first, each with the line that matched. Look here before you ask the user.",
             &[("query", "string", "Words, \"a phrase\", path:Folder, tag:name"), ("limit", "integer", "At most this many notes (default 20)")], &["query"]),
        tool("memory_read", "One memory note's whole text (Markdown with YAML properties), its version (memory_write needs it) and the notes that link to it.",
             &[("note", "string", "The note's path (Standards/Rust style), its title or its id")], &["note"]),
        tool("memory_write", "Saves a whole memory note as a new version, or makes it when the path is new; for a note that exists, give the version you read (changed since: nothing is saved). Keep what the repository and the board can't tell: decisions and reasons, preferences, gotchas. Link [[Folder/Note]], KADE-12, @handle. YAML properties on top: type, tags, client, project, applies_to (roles or all: who gets it in a run), source. A text with a key or token is refused.",
             &[("path", "string", "Folder and title, like Decisions/Memory in the database"), ("body_md", "string", "The complete text (Markdown)"),
               ("version", "integer", "The version you read; leave it out for a new note")], &["path", "body_md"]),
        tool("memory_append", "Adds text to a memory note without rewriting it: at the end of the section under heading (a new section at the end when the note has none), else at the end of the note. Makes the note when the path is new.",
             &[("note", "string", "The note's path, title or id, like Chief/Notes"), ("text", "string", "The text to add (Markdown), like a dated bullet"),
               ("heading", "string", "The section to add it to, like Open threads")], &["note", "text"]),
        tool("memory_move", "Moves a memory note to another path, or to a folder (end with /, the title stays), and rewrites the links that point to it; copy: true copies it instead. Use it to promote a useful note from an agent's folder into a shared folder.",
             &[("note", "string", "The note's path, title or id"), ("to", "string", "The new path (Standards/Rust style) or folder (Standards/)"),
               ("copy", "boolean", "Copy instead of move: the original stays")], &["note", "to"]),
        tool("attach_file", "Copies a local file (absolute path, or ~/…) into Hersir and attaches it to one task, project or client.",
             &[("path", "string", "Absolute path of the file"), ("task", "string", "Task identifier"), ("project", "string", "Project key or name"), ("client", "string", "Client name")], &["path"]),
        tool("add_person", "Adds a person (a colleague or reviewer) to Hersir.", &[("name", "string", "Full name"), ("email", "string", "Email")], &["name"]),
        tool("update_checkout", "Chat only, and only after the user said yes in this chat: updates a project's linked folder (the user's own checkout; new cards copy its vendor/ and node_modules/) to main as last fetched. A fast-forward, then composer install or npm ci where a lock file changed or a folder is missing; never the setup command. Changes nothing, and says why, with uncommitted changes unless stash is true, a merge or rebase in progress, a local main with its own commits, another branch unless switch is true, or a folder set to read in the Chief's folders. Answers at once; the result comes as a chat message.",
             &[PROJECT, ("switch", "boolean", "When the folder is on another branch: switch it to the default branch first (that branch stays as it is). Only when the user agreed to the switch"),
               ("stash", "boolean", "When tracked files have uncommitted changes: put them in git's stash first (they stay there: git stash pop brings them back), then update. Only when the user agreed to the stash in this chat"),
               ("folder", "string", "The folder to update; only the project's linked folder is allowed (the default)")], &["project"]),
        tool("merge_pull_request", "Merges a card's own pull request on GitHub with a merge commit, only when every rule holds: its project's Chief may merge switch is on (only the user sets it, in the app); the card is in Review with testing on; its latest QA verdict is qa_pass on the pull request's latest commit; GitHub can merge it; every check on it succeeded; no release card of the project is in Deploy. Otherwise it refuses with the reason and changes nothing: never work around a refusal. After a merge the card gets your comment and moves on to Deploy. Releases and deploys stay the user's.",
             &[TASK], &["task"]),
    ]
}

/// Where a project's link is: "github", "bitbucket" or "git" (another git URL); None without a link.
pub(crate) fn provider(repo_url: Option<&str>) -> Option<String> {
    gizai_core::repo_url::normalize(repo_url?).ok().flatten().map(|l| l.provider)
}

/// A short label for links: at most `n` characters.
pub(crate) fn short(s: &str, n: usize) -> String {
    match s.char_indices().nth(n) {
        Some((i, _)) => format!("{}…", s[..i].trim_end()),
        None => s.to_string(),
    }
}

/// Unix ms → "2026-10-07" (UTC).
pub fn ymd(ms: i64) -> String {
    let z = ms.div_euclid(86_400_000) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02}")
}
