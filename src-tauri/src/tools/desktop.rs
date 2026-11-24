//! GA-342: Claude Desktop asks Hersir's Chief (`ask_chief`) and reads its answer (`chief_reply`). Only Claude Desktop's
//! token is offered them (`desktop_catalog`, `call_desktop`), never the Chief's own chat turns, board checks or questions,
//! so a Chief's turn never starts another. A message from Claude Desktop counts as one you typed in Hersir's chat: it is
//! saved as yours and handed on as the Chat page's Send hands one on (`chat::send_from_desktop`, as `chat::send_message`),
//! starting a turn or waiting in the chat's queue, and it doesn't mark the chat as having used a tool from outside
//! Hersir, so the Chief's changes act at once. GA-351: but it is marked as from Claude Desktop in its meta
//! (`gizai_core::chat::desktop_meta`), which the Chat page shows, and the Chief's turn that answers it gets Claude
//! Desktop's own limits (`tools::Desktop::Answering`): only what Claude Desktop may not do itself doesn't come back
//! through the Chief. ask_chief answers at once, as a turn can outlast Claude's 240-second timeout for a tool; chief_reply
//! waits at most `MAX_WAIT` seconds.
use std::time::Duration;

use gizai_core::chat::{self as core_chat, ChatMessage};
use serde_json::{Value, json};

use super::{Args, ToolDef, err, tool};
use crate::AppState;
use crate::runs::Note;

pub const ASK_CHIEF: &str = "ask_chief";
pub const CHIEF_REPLY: &str = "chief_reply";

/// The statuses chief_reply gives, word for word as the card has them: a turn runs in the chat, or a message waits in its
/// queue to go by itself; the Chief's last answer has calls that wait for your Approve (or your message waits in the
/// queue for Send now); neither.
pub const WORKING: &str = "working";
pub const WAITING_FOR_YOU: &str = "waiting for you";
pub const ANSWERED: &str = "answered";

/// How long chief_reply waits when wait_seconds is left out, and at most.
pub const DEFAULT_WAIT: i64 = 60;
pub const MAX_WAIT: i64 = 120;

/// At most this many messages in one chief_reply: the last ones.
pub const MAX_MESSAGES: usize = 20;

/// At most this many characters in a message's text, and in the draft; a longer one is cut and ends with `CUT`.
pub const MAX_TEXT: usize = 8000;
pub const CUT: &str = "…(cut: read the rest in Hersir)";

/// ask_chief, as Claude Desktop is offered it: the card's description, word for word.
pub(super) fn ask_chief_def() -> ToolDef {
    tool(ASK_CHIEF, "Hands a message to Hersir's Chief, who answers with its memory, its copies of the code and the board. Answers at once; \
                     then call chief_reply for its answer.",
         &[("message", "string", "The message for the Chief (Markdown), as the user would type it in Hersir's chat: it is saved as theirs"),
           ("thread", "string", "A chat with the Chief, by the thread id ask_chief answered with. Leave it out for the chat From Claude Desktop")],
         &["message"])
}

/// chief_reply, as Claude Desktop is offered it: the card's description, word for word. It only reads.
pub(super) fn chief_reply_def() -> ToolDef {
    ToolDef {
        read_only: true,
        ..tool(CHIEF_REPLY, "Hersir's Chief's answer in a thread from ask_chief: its new messages, and whether it is still working. Call it again \
                             while status is working.",
               &[("thread", "string", "The thread id ask_chief answered with"),
                 ("after", "string", "Only the messages after this one: the message id ask_chief answered with, or the id of the last message you \
                                      got. Leave it out for the thread's last messages"),
                 ("wait_seconds", "integer", "How long to wait for the Chief's turn to end or a new message, in seconds: 60 when left out, at most 120")],
               &["thread"])
    }
}

/// ask_chief: `message` goes to the Chief in the chat `thread`, else in the chat From Claude Desktop (made the first time,
/// the same one after that, `core_chat::desktop_thread`). It starts a Chief turn, or waits in the chat's queue while one
/// runs, as a message you send in Hersir does, marked as from Claude Desktop (GA-351). Answers at once, without waiting
/// for the turn.
pub(super) async fn ask_chief(st: &AppState, a: &Args) -> Result<Value, String> {
    let message = a.req("message")?;
    let thread = match a.opt("thread") {
        Some(id) => {
            known(st, &id)?;
            id
        }
        None => {
            // The Chief can answer before the chat is made, so a refusal leaves no empty chat behind.
            let chief = crate::chat::chief_ready(st)?;
            let (id, new) = core_chat::desktop_thread(&st.db, &st.you_id, &chief).map_err(err)?;
            if new {
                (st.notify)(Note::RowsChanged("chat_threads"));
            }
            id
        }
    };
    let sent = crate::chat::send_from_desktop(st, thread, message).await?;
    Ok(json!({"thread": sent.thread_id, "message": sent.message_id, "status": WORKING}))
}

/// chief_reply: the chat's status, its messages after `after` and, while the Chief works, its draft. While it works and
/// nothing new came since `after`, it first waits, at most wait_seconds, until the turn ends or a new message comes.
pub(super) async fn chief_reply(st: &AppState, a: &Args) -> Result<Value, String> {
    let thread = a.req("thread")?;
    known(st, &thread)?;
    let after = a.opt("after");
    let wait = a.int("wait_seconds")?.unwrap_or(DEFAULT_WAIT).clamp(0, MAX_WAIT);
    let first = reply(st, &thread, after.as_deref())?;
    // Something to say already: the turn is over, or new messages came since `after`.
    if first.status != WORKING || wait == 0 || (after.is_some() && !first.messages.is_empty()) {
        return Ok(first.json(&thread));
    }
    let seen = first.last_id();
    let mut newest = core_chat::newest_said(&st.db, &thread).map_err(err)?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(wait.unsigned_abs());
    crate::chat::wait_until(st, deadline, || {
        if !matches!(status(st, &thread, after.as_deref()), Ok(WORKING)) {
            return true;
        }
        // A message to show came: one from you, the Chief or Hersir that wasn't there before.
        let now = core_chat::newest_said(&st.db, &thread).ok().flatten();
        if now == newest {
            return false;
        }
        newest = now;
        reply(st, &thread, after.as_deref()).map_or(true, |r| r.last_id() != seen)
    }).await;
    Ok(reply(st, &thread, after.as_deref())?.json(&thread))
}

/// GA-345: the chat as chief_reply gives it, at once, without waiting: `{"thread", "status", "messages", "draft"}`, the
/// draft null while the Chief isn't writing. For the Chief's view in Claude Desktop's chat (`apps`, view_read).
pub(super) fn now(st: &AppState, thread: &str) -> Result<Value, String> {
    known(st, thread)?;
    let mut out = reply(st, thread, None)?.json(thread);
    if out.get("draft").is_none() {
        out["draft"] = Value::Null;
    }
    Ok(out)
}

/// GA-345: the chat From Claude Desktop, when ask_chief made it (`core_chat::desktop_thread`) and it wasn't deleted since;
/// None before that. Only reads: it never makes the chat.
pub(super) fn from_claude_desktop(st: &AppState) -> Result<Option<core_chat::ChatThread>, String> {
    let Some(id) = gizai_core::settings::get::<String>(&st.db, core_chat::DESKTOP_CHAT).map_err(err)? else { return Ok(None) };
    match core_chat::get_thread(&st.db, &id) {
        Ok(t) => Ok(Some(t)),
        Err(gizai_core::Error::NotFound(_)) => Ok(None),
        Err(e) => Err(err(e)),
    }
}

/// A chat by its id, or why there is none. Nothing changes either way.
fn known(st: &AppState, thread: &str) -> Result<(), String> {
    match core_chat::get_thread(&st.db, thread) {
        Ok(_) => Ok(()),
        Err(gizai_core::Error::NotFound(_)) => Err(format!("There is no chat {thread} in Hersir: give a thread id ask_chief answered with, or leave \
                                                            thread out for the chat From Claude Desktop.")),
        Err(e) => Err(err(e)),
    }
}

/// What chief_reply answers with: the status, the messages shown and the draft.
struct Reply {
    status: &'static str,
    messages: Vec<Value>,
    draft: Option<String>,
}

impl Reply {
    fn last_id(&self) -> Option<String> {
        self.messages.last().and_then(|m| m["id"].as_str()).map(str::to_string)
    }

    /// GA-345: under `apps::MAX_RESULT` characters, as every result is: when the messages would pass it, the oldest go
    /// first. Measured as chief_reply sends it, as JSON text in a result's `content` (escaped once more).
    fn json(&self, thread: &str) -> Value {
        let mut from = 0;
        loop {
            let mut out = json!({"thread": thread, "status": self.status, "messages": &self.messages[from..]});
            if let Some(d) = &self.draft {
                out["draft"] = Value::String(d.clone());
            }
            if from == self.messages.len() || Value::String(out.to_string()).to_string().len() <= super::apps::BUDGET {
                return out;
            }
            from += 1;
        }
    }
}

/// The chat's status first, then its messages: once a turn has ended, every message of it is saved, so an answer that
/// says `answered` has them all.
fn reply(st: &AppState, thread: &str, after: Option<&str>) -> Result<Reply, String> {
    let status = status(st, thread, after)?;
    let messages = shown(st, thread, after)?;
    let draft = (status == WORKING).then(|| crate::chat::draft(st, thread).map(|d| cut(&plain(&d))).unwrap_or_default());
    Ok(Reply { status, messages, draft })
}

/// `WORKING` while a turn runs in the chat (also while it starts, and between an answer and the one for messages queued
/// meanwhile), or a message waits in its queue to go by itself. Else `WAITING_FOR_YOU` while calls of the Chief's last
/// answer there wait for your Approve, or when the message `after` waits in the queue for Send now (the answer before it
/// was stopped or failed, or Hersir restarted): it goes only when you send it in Hersir. Else `ANSWERED`.
fn status(st: &AppState, thread: &str, after: Option<&str>) -> Result<&'static str, String> {
    if crate::chat::answering(st, thread) || core_chat::queue_ready(&st.db, thread).map_err(err)? {
        return Ok(WORKING);
    }
    if gizai_core::pending::waiting_from_last_answer(&st.db, thread).map_err(err)? > 0 {
        return Ok(WAITING_FOR_YOU);
    }
    if let Some(after) = after && core_chat::queue(&st.db, thread).map_err(err)?.iter().any(|q| q.id == after && q.held) {
        return Ok(WAITING_FOR_YOU);
    }
    Ok(ANSWERED)
}

/// The chat's messages after the message `after` (all of them without it), at most the last `MAX_MESSAGES`: yours, the
/// Chief's and Hersir's notes, not its tool calls. `after` may be a message that still waits in the chat's queue: nothing
/// comes after it until it goes (it keeps its id then). An `after` that is neither in the chat nor in its queue is an error.
fn shown(st: &AppState, thread: &str, after: Option<&str>) -> Result<Vec<Value>, String> {
    // The queue first: a queued message moves into the chat, never back, so it is found in one of the two.
    if let Some(after) = after && core_chat::queue(&st.db, thread).map_err(err)?.iter().any(|q| q.id == after) {
        return Ok(vec![]);
    }
    let all = core_chat::messages(&st.db, thread).map_err(err)?;
    let from = match after {
        Some(after) => all.iter().position(|m| m.id == after).map(|i| i + 1)
            .ok_or_else(|| format!("There is no message {after} in chat {thread}: give a message id from ask_chief or chief_reply, or leave after out."))?,
        None => 0,
    };
    let said: Vec<Value> = all[from..].iter().filter_map(message).collect();
    Ok(said[said.len().saturating_sub(MAX_MESSAGES)..].to_vec())
}

/// One message as chief_reply shows it: its id, who it is from (you, the Chief, or Hersir for Hersir's notes in the chat),
/// its time and its text (`plain`, `cut`). None for a tool call.
fn message(m: &ChatMessage) -> Option<Value> {
    let from = match m.role.as_str() {
        "user" => "you",
        "agent" => "Chief",
        "system" => "Hersir",
        _ => return None,
    };
    Some(json!({"id": m.id, "from": from, "time": time(m.created_at), "text": cut(&plain(m.body_md.as_deref().unwrap_or_default()))}))
}

/// At most `MAX_TEXT` characters: a longer text keeps its start and ends with `CUT`, `MAX_TEXT` characters in all.
pub fn cut(text: &str) -> String {
    cut_at(text, MAX_TEXT)
}

/// GA-345: at most `max` characters: a longer text keeps its start and ends with `CUT`, `max` characters in all (`CUT`
/// alone when `max` is shorter than it).
pub fn cut_at(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let keep = max.saturating_sub(CUT.chars().count());
    let end = text.char_indices().nth(keep).map_or(text.len(), |(i, _)| i);
    format!("{}{CUT}", &text[..end])
}

/// The text with each link to a Hersir item, `[text](gizai:…)`, as just its text (its backslash escapes undone), as Claude
/// Desktop can't open them. A link's text is one line with no unescaped `]`, and its target has no space or parenthesis,
/// as the Chat page reads them (`ITEM_LINK_SOURCE`, src/lib/itemLinks.ts).
pub fn plain(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    while let Some(open) = text[at..].find('[').map(|i| at + i) {
        out.push_str(&text[at..open]);
        match item_link(&text[open..]) {
            Ok((label, len)) => {
                out.push_str(&label);
                at = open + len;
            }
            Err(none) => {
                out.push_str(&text[open..open + none]);
                at = open + none;
            }
        }
    }
    out.push_str(&text[at..]);
    out
}

/// A gizai: link at the start of `s` (which starts with `[`): its text, unescaped, and the link's length in bytes. Else
/// how many bytes of `s` (at least 1) no link starts in: up to the `]` its text would end at (a `[` before it reads to
/// the same `]`), or up to the end of the line. So the text is read once, whatever it holds.
fn item_link(s: &str) -> Result<(String, usize), usize> {
    let mut label = String::new();
    let mut chars = s.char_indices().skip(1);
    let close = loop {
        match chars.next() {
            None => return Err(s.len()),
            Some((i, '\n')) => return Err(i),
            Some((_, '\\')) => match chars.next() {
                None => return Err(s.len()),
                Some((i, '\n')) => return Err(i),
                // Markdown's escapes: a backslash before ASCII punctuation stands for that character.
                Some((_, c)) => {
                    if !c.is_ascii_punctuation() {
                        label.push('\\');
                    }
                    label.push(c);
                }
            },
            Some((i, ']')) => break i,
            Some((_, c)) => label.push(c),
        }
    };
    let link = s[close..].strip_prefix("](gizai:").filter(|_| !label.is_empty()).and_then(|target| {
        let end = target.find(|c: char| c == ')' || c == '(' || c.is_whitespace())?;
        target[end..].starts_with(')').then(|| s.len() - target.len() + end + 1)
    });
    link.map(|len| (label, len)).ok_or(close)
}

/// Unix ms as UTC time: `2026-11-23T14:05:09Z`.
pub fn time(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let day = secs.rem_euclid(86_400);
    format!("{}T{:02}:{:02}:{:02}Z", super::ymd(ms), day / 3600, day % 3600 / 60, day % 60)
}
