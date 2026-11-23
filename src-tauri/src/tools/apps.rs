//! GA-343, GA-345: Hersir's MCP App (`gizai_mcp::APPS_EXTENSION`): the board, a card or the Chief's thread as a view in
//! Claude Desktop's chat.
//!
//! - One view, [`APP_URI`]: the single HTML file `npm run build` makes from src/widget (`vite build --mode mcp-app`, into
//!   src-tauri/mcp-app), embedded by build.rs. Without that build (cargo alone) it is a page that says so.
//! - Three tools for Claude Desktop's token (`GizaiTools::desktop`), offered only when it declared the extension
//!   ([`tools`]): [`SHOW`] for the model, whose result shows the view, and for the view alone (`visibility: ["app"]`)
//!   [`VIEW_READ`], what the view shows as it is now, and board_changes (GA-344, `super::BOARD_CHANGES`), what changed
//!   since: Claude pushes nothing to a view, so it asks again and again.
//! - The view changes things with the model's own tools (move_task, comment_on_task, start_agent_run, stop_agent_run,
//!   ask_chief, chief_reply), through Claude and with GA-341's rules: none of these three writes.
//!
//! The data comes from what the app and the Chief's tools read: the board as the Tasks page lists it (`tasks::list_lean`)
//! with the Inbox's cards (`tasks::needs_you`) and the live runs; a card as get_task reads it (`resolve::task`, its
//! comments and runs), with its live run's log as the Run panel has it (`crate::runs::events_for`); the Chief's thread as
//! chief_reply gives it (`desktop::now`). No result passes [`MAX_RESULT`] characters: Claude turns a longer one into a
//! file, and then the view doesn't load. A text cut for that ends with `desktop::CUT`.
use std::collections::HashSet;

use gizai_agents::stream::RunEvent;
use gizai_core::model::{Comment, Project, Run, Task, TaskFilter};
use gizai_core::{chat as core_chat, comments, projects, runs, tasks, team};
use gizai_mcp::{AppView, Reply, ToolDef, app_meta};
use serde_json::{Value, json};

use super::desktop::{self, CUT, cut_at};
use super::{Args, Cx, err, resolve, schema};
use crate::AppState;

/// The view's resource.
pub const APP_URI: &str = "ui://hersir/app.html";

/// The tool for the model: shows the board, a card or the Chief's thread in the chat.
pub const SHOW: &str = "show";

/// The tool for the view alone: what it shows, as it is now, with the change counter.
pub const VIEW_READ: &str = "view_read";

/// What `show` and `view_read` show (`view`).
pub const VIEWS: [&str; 3] = ["board", "card", "chief"];

/// The view: one HTML file with everything in it (build.rs).
const APP_HTML: &str = include_str!(concat!(env!("OUT_DIR"), "/mcp-app.html"));

/// No result is longer than this many characters: Claude turns a longer one into a file pointer, and then the view
/// doesn't load.
pub const MAX_RESULT: usize = 150_000;

/// A view's data stays under this many bytes of JSON (`fits`), which leaves room under `MAX_RESULT` for the sentence and
/// the rest of the result. A byte counts at least as much as a character.
pub const BUDGET: usize = 120_000;

/// The board: at most this many cards a column, as Board.tsx draws them (its CAP); a column's `total` says how many it
/// has. Fewer in Backlog, Done and Cancelled, which it shows folded, and fewer again when the board would pass `BUDGET`.
const PER_COLUMN: usize = 50;
const PER_RAIL: usize = 20;

/// A card: its last 20 comments, as get_task gives them; its last 5 runs; the last 200 lines of its live run's output.
pub const COMMENTS: usize = 20;
pub const RUNS: usize = 5;
pub const OUTPUT_LINES: usize = 200;

/// The longest a card's texts may be, in characters, before they are cut (`desktop::cut_at`). A card whose data would
/// still pass `BUDGET` gets them smaller, step by step (`STEPS`).
#[derive(Clone, Copy)]
struct Caps {
    description: usize,
    acceptance: usize,
    hold_reason: usize,
    comment: usize,
    summary: usize,
    output: usize,
}

const CAPS: Caps = Caps { description: 40_000, acceptance: 20_000, hold_reason: 2_000, comment: 4_000, summary: 4_000, output: 20_000 };

/// The share of `CAPS` each try keeps, in percent. The last one fits any card.
const STEPS: [usize; 8] = [100, 70, 50, 35, 25, 15, 8, 4];

/// No text is cut shorter than this, `CUT` included.
const MIN_CAP: usize = 100;

impl Caps {
    fn percent(self, p: usize) -> Caps {
        let f = |n: usize| (n * p / 100).max(MIN_CAP);
        Caps {
            description: f(self.description), acceptance: f(self.acceptance), hold_reason: f(self.hold_reason), comment: f(self.comment),
            summary: f(self.summary), output: f(self.output),
        }
    }
}

/// The tools for Claude Desktop when it shows MCP Apps' views: show, view_read and board_changes, in that order.
pub fn tools() -> Vec<ToolDef> {
    let view = ("view", "enum:board|card|chief", "board: Hersir's board; card: one card; chief: a chat with Hersir's Chief");
    let id = ("id", "string", "board: a project's key, like GA (leave it out for every project on the board); card: the card's identifier, \
                               like GA-12 (needed); chief: a chat's thread id (leave it out for the chat From Claude Desktop)");
    let mut changes = super::board_changes_tool();
    changes.meta = Some(app_meta(None, &["app"]));
    vec![
        ToolDef {
            name: SHOW.into(),
            description: "Shows the user Hersir's board, a card or the Chief's thread inside this chat. Use it when the user wants to see the \
                          board or a card, and before you answer about a card, so they see exactly what it says."
                .into(),
            input_schema: schema(&[view, id], &["view"]),
            read_only: true,
            meta: Some(app_meta(Some(APP_URI), &["model", "app"])),
        },
        ToolDef {
            name: VIEW_READ.into(),
            description: "The board, a card or the Chief's thread as Hersir has it now, for Hersir's view in this chat, with seq: the change \
                          counter to ask board_changes with."
                .into(),
            input_schema: schema(&[view, id], &["view"]),
            read_only: true,
            meta: Some(app_meta(None, &["app"])),
        },
        changes,
    ]
}

/// Whether `name` is one of the view's tools (`tools`).
pub fn is_app_tool(name: &str) -> bool {
    name == SHOW || name == VIEW_READ || name == super::BOARD_CHANGES
}

/// The view, for `resources/list` and `resources/read`. No `csp`: it reaches nothing outside itself.
pub fn view() -> AppView {
    AppView {
        uri: APP_URI.into(),
        name: "Hersir".into(),
        description: "Hersir's board, a card or the Chief's thread, kept up to date.".into(),
        html: APP_HTML.into(),
        ui: json!({"prefersBorder": true}),
    }
}

/// show, view_read or board_changes, as Claude Desktop calls them (`GizaiTools::reply`). All three only read, so an
/// error (an unknown view, card, project or thread) changes nothing.
/// - show: one sentence for the model in `content`; `{"view", "id", "data"}` in `structuredContent`; the view, under
///   both keys, in the result's `_meta`, as on the tool (`app_meta`).
/// - view_read: the same sentence; `{"view", "id", "seq", "data"}`, with board_changes' counter now.
/// - board_changes: its answer (`read::board_changes`) as JSON text and in `structuredContent`.
pub(crate) fn reply(cx: &Cx, name: &str, a: &Args) -> Result<Reply, String> {
    if name == super::BOARD_CHANGES {
        let changes = super::read::board_changes(cx.st, a)?;
        return Ok(Reply { text: changes.to_string(), structured: Some(changes), meta: None });
    }
    let view = view_name(a)?;
    // The counter before the data: a change the data misses has a later number, so the next board_changes names it.
    let seq = match name {
        VIEW_READ => Some(gizai_core::feed::since(cx.db(), None).map_err(err)?.counter),
        _ => None,
    };
    let id = a.opt("id");
    let shown = match view {
        "board" => board(cx, match id { Some(p) => Some(resolve::project(cx, &p)?), None => None }.as_ref())?,
        "card" => card(cx, &resolve::task(cx, &id.ok_or("missing \"id\": the card's identifier, like GA-12")?)?)?,
        _ => chief(cx.st, id.as_deref())?,
    };
    let mut out = json!({"view": view, "id": shown.id, "data": shown.data});
    if let Some(seq) = seq {
        out["seq"] = json!(seq);
    }
    Ok(Reply {
        text: shown.sentence,
        structured: Some(out),
        meta: (name == SHOW).then(|| json!({"ui": {"resourceUri": APP_URI}, "ui/resourceUri": APP_URI})),
    })
}

/// The `view` asked for: board, card or chief (case ignored).
fn view_name(a: &Args) -> Result<&'static str, String> {
    let v = a.opt("view").ok_or("missing \"view\": board, card or chief")?;
    VIEWS.iter().find(|x| x.eq_ignore_ascii_case(&v)).copied().ok_or_else(|| format!("\"view\" is board, card or chief, not \"{v}\""))
}

/// Whether a view's data fits in a result: at most `BUDGET` bytes of JSON.
pub(crate) fn fits(data: &Value) -> bool {
    data.to_string().len() <= BUDGET
}

/// What a view shows: its `id` (the project's key, the card's identifier, the chat's thread id; null for every project on
/// the board, or before the chat From Claude Desktop is made), its `data` and the sentence for the model.
struct Shown {
    id: Value,
    data: Value,
    sentence: String,
}

/// The board: `{"team", "project", "columns": [{"name", "kind", "total", "cards": [...]}]}`, the columns of the project's
/// team (none given: of the first team) in board order, each with its `total` of cards and the cards on it in board order,
/// at most `PER_COLUMN` (`PER_RAIL` in a folded one), fewer when the whole would pass `BUDGET` (`board_card`). With a
/// project, its cards; without, the team's: those of its projects, and on the first team's board also those without a
/// project (as the Tasks page has it, GA-225).
fn board(cx: &Cx, project: Option<&Project>) -> Result<Shown, String> {
    let db = cx.db();
    let team = resolve::team_of(cx, project)?;
    let mut cards = tasks::list_lean(db, &TaskFilter { project_id: project.map(|p| p.id.clone()), open_only: false }).map_err(err)?;
    if project.is_none() {
        let first = team::list(db).map_err(err)?.first().map(|t| t.id.clone());
        let all = projects::list(db).map_err(err)?;
        let team_of = |t: &Task| t.project_id.as_ref().and_then(|p| all.iter().find(|x| &x.id == p)).and_then(|p| p.team_id.clone())
            .or_else(|| first.clone());
        cards.retain(|t| team_of(t).as_deref() == Some(team.id.as_str()));
    }
    let needs_you: HashSet<String> = tasks::needs_you(db, &cx.st.you_id).map_err(err)?.into_iter().map(|t| t.id).collect();
    let running: HashSet<String> = crate::runs::live(cx.st).into_iter().map(|r| r.task_id).collect();
    let mut states = team.states.clone();
    states.sort_by(|a, b| a.sort_key.cmp(&b.sort_key));
    let columns: Vec<(&team::WorkflowState, Vec<Value>)> = states.iter().map(|s| {
        let on = cards.iter().filter(|t| t.state_id == s.id).map(|t| board_card(t, running.contains(&t.id), needs_you.contains(&t.id)));
        (s, on.collect())
    }).collect();
    let mut cap = PER_COLUMN;
    let data = loop {
        let data = json!({
            "team": team.name,
            "project": project.map(|p| json!({"key": p.key, "name": p.name})),
            "columns": columns.iter().map(|(s, on)| {
                let rail = matches!(s.category.as_str(), "backlog" | "done" | "cancelled");
                let shown: Vec<Value> = on.iter().take(if rail { cap.min(PER_RAIL) } else { cap }).cloned().collect();
                json!({"name": s.name, "kind": super::read::kind_of(&s.category), "total": on.len(), "cards": shown})
            }).collect::<Vec<_>>(),
        });
        if cap == 0 || fits(&data) {
            break data;
        }
        cap = if cap > 5 { cap - 5 } else { cap / 2 };
    };
    let what = match project {
        Some(p) => format!("the board of {} ({})", p.name, p.key),
        None => format!("{}'s board", team.name),
    };
    let count = match cards.len() {
        0 => "no cards yet".to_string(),
        1 => "1 card".to_string(),
        n => format!("{n} cards"),
    };
    let held = match cards.iter().filter(|t| t.hold.is_some()).count() {
        0 => String::new(),
        n => format!(", {n} on hold"),
    };
    Ok(Shown {
        id: project.map_or(Value::Null, |p| json!(p.key)),
        data,
        sentence: format!("The user sees {what} from Hersir in this chat: {count} in {} columns{held}.", columns.len()),
    })
}

/// One card on the board, with what a card on the board shows. `running`: an agent works on it now; `needs_you`: it is in
/// the Inbox (on hold, or waiting for you in Review or Deploy).
fn board_card(t: &Task, running: bool, needs_you: bool) -> Value {
    json!({
        "identifier": t.identifier, "title": cut_at(&t.title, tasks::TITLE_MAX), "project": t.project_name, "assignee": t.assignee_name,
        "labels": labels(t), "priority": t.priority, "hold": t.hold, "running": running, "needs_you": needs_you,
    })
}

/// A card's labels, with their colours as the board shows them.
fn labels(t: &Task) -> Vec<Value> {
    t.labels.iter().map(|l| json!({"name": l.name, "color": l.color})).collect()
}

/// A card as get_task reads it, with what the card's page shows: its fields, its last `COMMENTS` comments (oldest first,
/// as get_task gives them), its last `RUNS` runs (newest first) and its live run with the last `OUTPUT_LINES` lines of its
/// output. Its long texts are cut at `CAPS`, and smaller when the whole would pass `BUDGET`.
fn card(cx: &Cx, t: &Task) -> Result<Shown, String> {
    let db = cx.db();
    let all = comments::list(db, &t.id).map_err(err)?;
    let comments = &all[all.len().saturating_sub(COMMENTS)..];
    let runs: Vec<Run> = runs::list_for_task(db, &t.id).map_err(err)?.into_iter().take(RUNS).collect();
    let live = match crate::runs::live(cx.st).into_iter().find(|r| r.task_id == t.id) {
        Some(r) => {
            let agent = match runs.iter().find(|x| x.id == r.run_id) {
                Some(x) => x.agent_name.clone(),
                None => team::all_agents(db).map_err(err)?.into_iter().find(|(_, m)| m.actor_id == r.agent_id).map(|(_, m)| m.name)
                    .unwrap_or_default(),
            };
            let lines: Vec<String> = crate::runs::events_for(cx.st, &r.run_id).iter().flat_map(|e| lines_of(&e.event)).collect();
            Some((agent, lines))
        }
        None => None,
    };
    let mut data = Value::Null;
    for p in STEPS {
        data = card_data(t, comments, &runs, live.as_ref(), CAPS.percent(p));
        if fits(&data) {
            break;
        }
    }
    let title = cut_at(&t.title, tasks::TITLE_MAX);
    let hold = t.hold.as_deref().map_or(String::new(), |h| format!(", on hold ({})", h.replace('_', " ")));
    Ok(Shown {
        id: json!(t.identifier),
        data,
        sentence: format!("{} – {}, in {}{hold}.", t.identifier, title.trim_end_matches('.'), t.state_name),
    })
}

/// The card's data with its texts cut at `caps`.
fn card_data(t: &Task, comments: &[Comment], runs: &[Run], live: Option<&(String, Vec<String>)>, caps: Caps) -> Value {
    json!({
        "identifier": t.identifier, "title": cut_at(&t.title, tasks::TITLE_MAX), "project": t.project_name, "column": t.state_name,
        "assignee": t.assignee_name, "labels": labels(t), "priority": t.priority, "hold": t.hold,
        "hold_reason": t.hold_reason.as_deref().map(|r| cut_at(r, caps.hold_reason)),
        "description_md": cut_at(&t.description_md, caps.description),
        "acceptance_md": t.acceptance_md.as_deref().map(|a| cut_at(a, caps.acceptance)),
        "pr_url": t.pr_url,
        "comments": comments.iter().map(|c| json!({
            "author": c.author_name, "at": desktop::time(c.created_at), "body_md": cut_at(&c.body_md, caps.comment),
        })).collect::<Vec<_>>(),
        "runs": runs.iter().map(|r| json!({
            "agent": r.agent_name, "status": r.status, "outcome": r.outcome, "started_at": r.started_at.map(desktop::time),
            "ended_at": r.ended_at.map(desktop::time), "summary": r.summary_md.as_deref().map(|s| cut_at(s, caps.summary)),
        })).collect::<Vec<_>>(),
        "live": live.map(|(agent, lines)| json!({"agent": agent, "output": output(lines, caps.output)})),
    })
}

/// A live run's output: its last `OUTPUT_LINES` lines, at most `max` characters of them, kept from the end. When lines
/// or characters were left out, `CUT` follows on a line of its own, so the text ends with it as every cut text does.
pub fn output(lines: &[String], max: usize) -> String {
    let from = lines.len().saturating_sub(OUTPUT_LINES);
    let mut text = lines[from..].join("\n");
    let mut cut = from > 0;
    let n = text.chars().count();
    if n > max {
        let at = text.char_indices().nth(n - max).map_or(text.len(), |(i, _)| i);
        text.drain(..at);
        cut = true;
    }
    if cut { format!("{text}\n{CUT}") } else { text }
}

/// The lines an event of a run makes in its log, as the Run panel draws them (RunPanel.tsx's `lineOf`), as plain text:
/// none for one it doesn't show (a tool's result that went well, the limits, Claude Code's other lines).
pub fn lines_of(e: &RunEvent) -> Vec<String> {
    let text = |t: &str| t.trim().lines().map(str::to_string).collect::<Vec<_>>();
    match e {
        RunEvent::Init { model, .. } if model.is_empty() => vec!["Started".into()],
        RunEvent::Init { model, .. } => vec![format!("Started · {model}")],
        // the result line is Hersir's, not the agent's words
        RunEvent::Text { text: t } => text(&t.lines().filter(|l| !l.starts_with("GIZAI_RESULT:")).collect::<Vec<_>>().join("\n")),
        RunEvent::ToolUse { name, summary } => vec![format!("{name} {summary}").trim_end().to_string()],
        RunEvent::ToolResult { is_error: true, preview } => text(preview),
        RunEvent::Result { is_error: true, subtype, num_turns, .. } => vec![format!("Ended with {subtype} · {num_turns} turns")],
        RunEvent::Result { num_turns, .. } => vec![format!("Done · {num_turns} turns")],
        RunEvent::Note { text: t } => text(t),
        RunEvent::Refused { tool, input, reason } if reason.is_empty() => vec![format!("Refused: {tool} {input}")],
        RunEvent::Refused { tool, input, reason } => vec![format!("Refused: {tool} {input} ({reason})")],
        RunEvent::McpServers { servers } => {
            let each: Vec<String> = servers.iter().map(|s| {
                let status = if s.status == "connected" { "connected" } else if s.status.is_empty() { "failed" } else { s.status.as_str() };
                format!("{} ({status})", s.name)
            }).collect();
            vec![format!("MCP servers: {}", each.join(", "))]
        }
        RunEvent::Other { raw_type } if raw_type.starts_with("cap_exceeded") => vec![match raw_type.as_str() {
            t if t.ends_with(":time") => "Stopped at the time limit",
            t if t.ends_with(":tools") => "Stopped at the tool-call limit",
            _ => "Stopped at the time or tool-call limit",
        }.into()],
        _ => vec![],
    }
}

/// A chat with the Chief as chief_reply gives it (`desktop::now`): `{"thread", "status", "messages", "draft"}`, the draft
/// null while the Chief isn't writing. Without `thread`: the chat From Claude Desktop, and before ask_chief made it, an
/// empty one with no thread; view_read never makes it. An unknown thread is an error.
fn chief(st: &AppState, thread: Option<&str>) -> Result<Shown, String> {
    let thread = match thread {
        Some(id) => match core_chat::get_thread(&st.db, id) {
            Ok(t) => Some(t),
            Err(gizai_core::Error::NotFound(_)) => return Err(format!("There is no chat {id} in Hersir: give a thread id ask_chief answered \
                                                                       with, or leave id out for the chat From Claude Desktop.")),
            Err(e) => return Err(err(e)),
        },
        None => desktop::from_claude_desktop(st)?,
    };
    let Some(thread) = thread else {
        return Ok(Shown {
            id: Value::Null,
            data: json!({"thread": null, "status": desktop::ANSWERED, "messages": [], "draft": null}),
            sentence: format!("The user sees the Chief's chat \"{}\" from Hersir in this chat: no messages yet.", core_chat::FROM_CLAUDE_DESKTOP),
        });
    };
    let data = desktop::now(st, &thread.id)?;
    let status = match data["status"].as_str() {
        Some(desktop::WORKING) => "the Chief is working on it",
        Some(desktop::WAITING_FOR_YOU) => "it waits for the user in Hersir",
        _ => "answered",
    };
    let sentence = match data["messages"].as_array().map_or(0, Vec::len) {
        0 => format!("The user sees the Chief's chat \"{}\" from Hersir in this chat: no messages yet.", thread.title),
        1 => format!("The user sees the Chief's chat \"{}\" from Hersir in this chat: 1 message, {status}.", thread.title),
        n => format!("The user sees the Chief's chat \"{}\" from Hersir in this chat: {n} messages, {status}.", thread.title),
    };
    Ok(Shown { id: json!(thread.id), data, sentence })
}
