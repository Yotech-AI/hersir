//! GA-343: Hersir's MCP App (`gizai_mcp::APPS_EXTENSION`), a spike: the board as a view in Claude Desktop's chat.
//!
//! - One view, [`APP_URI`]: the single HTML file `npm run build` makes from src/widget (`vite build --mode mcp-app`, into
//!   src-tauri/mcp-app), embedded by build.rs. Without that build (cargo alone) it is a page that says so.
//! - Two tools for Claude Desktop's token (`GizaiTools::desktop`), offered only when it declared the extension:
//!   [`SHOW_BOARD`] for the model, whose result shows the view, and [`BOARD_READ`] for the view alone
//!   (`visibility: ["app"]`), which calls it every few seconds: Claude doesn't push anything to a view.
//!
//! Both answer with one sentence in `content` and the board in `structuredContent` ([`board`]): the team's columns and
//! the cards on them, each with only what a card on the board shows. Nothing secret goes there (no description, hold
//! reason, branch, pull request or path): some hosts show it to the model too. GA-345 makes the tools general
//! (`show {view, id?}` and `view_read`).
use gizai_core::model::{Project, Task, TaskFilter};
use gizai_core::{projects, tasks, team};
use gizai_mcp::{AppView, Reply, ToolDef, app_meta};
use serde_json::{Value, json};

use super::{Args, Cx, err, resolve, schema};

/// The view's resource.
pub const APP_URI: &str = "ui://hersir/app.html";

/// The tool for the model: shows the board in the chat.
pub const SHOW_BOARD: &str = "show_board";

/// The tool for the view alone: the board again, as it is now.
pub const BOARD_READ: &str = "board_read";

/// The view: one HTML file with everything in it (build.rs).
const APP_HTML: &str = include_str!(concat!(env!("OUT_DIR"), "/mcp-app.html"));

/// Cards per column at most, as the board draws them (Board.tsx's CAP); a column's `total` says how many it has.
const PER_COLUMN: usize = 50;

/// Fewer in Backlog, Done and Cancelled, which the board shows folded.
const PER_RAIL: usize = 20;

/// The board's JSON stays under this many characters: Claude turns a result over about 150,000 into a file, and then
/// the view doesn't load. Over it, fewer cards per column.
const BUDGET: usize = 100_000;

/// A card's title is cut at this many characters.
const TITLE_MAX: usize = 200;

/// The project argument both tools take.
const PROJECT: (&str, &str, &str) = ("project", "string", "The project's key (GA), name or id. Omit for the board of the first team, with all its projects");

/// The tools for Claude Desktop when it shows MCP Apps' views.
pub fn tools() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: SHOW_BOARD.into(),
            description: "Shows the user Hersir's board in this chat, live: its columns and cards, as on Hersir's Tasks page. Use it when the user \
                          asks to see the board or a project's cards. To read cards yourself, use list_tasks or get_task."
                .into(),
            input_schema: schema(&[PROJECT], &[]),
            read_only: true,
            meta: Some(app_meta(Some(APP_URI), &["model", "app"])),
        },
        ToolDef {
            name: BOARD_READ.into(),
            description: "The board's columns and cards, for Hersir's board view in the chat, which calls it every few seconds.".into(),
            input_schema: schema(&[PROJECT], &[]),
            read_only: true,
            meta: Some(app_meta(None, &["app"])),
        },
    ]
}

/// Whether `name` is one of the app's tools.
pub fn is_app_tool(name: &str) -> bool {
    name == SHOW_BOARD || name == BOARD_READ
}

/// The view, for `resources/list` and `resources/read`. No `csp`: it reaches nothing outside itself.
pub fn view() -> AppView {
    AppView {
        uri: APP_URI.into(),
        name: "Hersir board".into(),
        description: "Hersir's board: its columns and cards, kept up to date.".into(),
        html: APP_HTML.into(),
        ui: json!({"prefersBorder": true}),
    }
}

/// `show_board` or `board_read`, as Claude Desktop calls it.
pub(crate) fn reply(cx: &Cx, name: &str, a: &Args) -> Result<Reply, String> {
    let project = match a.opt("project") {
        Some(p) => Some(resolve::project(cx, &p)?),
        None => None,
    };
    let board = board(cx, project.as_ref())?;
    Ok(Reply {
        text: sentence(&board),
        structured: Some(board),
        // The view, under both keys, as on the tool (`app_meta`).
        meta: (name == SHOW_BOARD).then(|| json!({"ui": {"resourceUri": APP_URI}, "ui/resourceUri": APP_URI})),
    })
}

/// The one sentence in `content`, for the model.
fn sentence(board: &Value) -> String {
    let count = |k: &str| board[k].as_array().map_or(0, Vec::len);
    let cards: u64 = board["columns"].as_array().map_or(0, |c| c.iter().filter_map(|c| c["total"].as_u64()).sum());
    let held = board["cards"].as_array().map_or(0, |c| c.iter().filter(|c| c["hold"] == true).count());
    let what = match board["project"]["name"].as_str() {
        Some(name) => format!("the board of {name} ({})", board["project"]["key"].as_str().unwrap_or_default()),
        None => format!("{}'s board", board["team"].as_str().unwrap_or("the team")),
    };
    let cards = match cards {
        0 => "no cards yet".to_string(),
        1 => "1 card".to_string(),
        n => format!("{n} cards"),
    };
    let held = if held > 0 { format!(", {held} on hold") } else { String::new() };
    format!("The user sees {what} from Hersir in this chat: {cards} in {} columns{held}.", count("columns"))
}

/// The board as the view draws it (`structuredContent`): the columns of the project's team (none given: of the first
/// team) in board order, each with its `total` of cards, and the cards on them in board order, at most `PER_COLUMN` a
/// column (`PER_RAIL` in a folded one), fewer when the whole would pass `BUDGET`. With a project, its cards; without, the
/// team's: those of its projects, and on the first team's board also those without a project (as the Tasks page has it,
/// GA-225).
pub(crate) fn board(cx: &Cx, project: Option<&Project>) -> Result<Value, String> {
    let db = cx.db();
    let team = resolve::team_of(cx, project)?;
    let mut cards = tasks::list(db, &TaskFilter { project_id: project.map(|p| p.id.clone()), open_only: false }).map_err(err)?;
    if project.is_none() {
        let first = team::list(db).map_err(err)?.first().map(|t| t.id.clone());
        let all = projects::list(db).map_err(err)?;
        let team_of = |t: &Task| t.project_id.as_ref().and_then(|p| all.iter().find(|x| &x.id == p)).and_then(|p| p.team_id.clone())
            .or_else(|| first.clone());
        cards.retain(|t| team_of(t).as_deref() == Some(team.id.as_str()));
    }
    let mut states = team.states.clone();
    states.sort_by(|a, b| a.sort_key.cmp(&b.sort_key));
    // Who works on which card now: the agent's name, as the board's "… is working".
    let live = crate::runs::live(cx.st);
    let agents = team::all_agents(db).map_err(err)?;
    let working = |task: &str| live.iter().find(|r| r.task_id == task)
        .and_then(|r| agents.iter().find(|(_, m)| m.actor_id == r.agent_id)).map(|(_, m)| m.name.clone());
    let columns: Vec<(Value, Vec<Value>)> = states.iter().map(|s| {
        let on: Vec<Value> = cards.iter().filter(|t| t.state_id == s.id).map(|t| card(t, working(&t.id))).collect();
        let column = json!({"id": s.id, "name": s.name, "category": s.category, "wipLimit": s.wip_limit, "auto": s.auto, "total": on.len()});
        (column, on)
    }).collect();
    let mut cap = PER_COLUMN;
    loop {
        let shown: Vec<Value> = columns.iter().flat_map(|(c, on)| {
            let rail = matches!(c["category"].as_str(), Some("backlog" | "done" | "cancelled"));
            on.iter().take(if rail { cap.min(PER_RAIL) } else { cap }).cloned()
        }).collect();
        let board = json!({
            "view": "board",
            "team": team.name,
            "project": project.map(|p| json!({"id": p.id, "key": p.key, "name": p.name})),
            "columns": columns.iter().map(|(c, _)| c.clone()).collect::<Vec<_>>(),
            "cards": shown,
        });
        if cap <= 5 || board.to_string().len() <= BUDGET {
            return Ok(board);
        }
        cap -= 5;
    }
}

/// One card, with what a card on the board shows: no description, hold reason, branch or pull request.
fn card(t: &Task, working: Option<String>) -> Value {
    json!({
        "id": t.id, "identifier": t.identifier, "title": super::short(&t.title, TITLE_MAX), "column": t.state_id, "sortKey": t.sort_key,
        "priority": t.priority, "assignee": t.assignee_name, "assigneeKind": t.assignee_kind,
        "labels": t.labels.iter().map(|l| json!({"name": l.name, "color": l.color})).collect::<Vec<_>>(),
        "hold": t.hold.is_some(), "withLead": t.with_lead, "byHand": t.by_hand, "waitsFor": t.waits_for, "working": working,
    })
}
