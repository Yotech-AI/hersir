// The Chat page's terminal panel (GA-278): the chat's terminals, the one shown and whether the panel shows, kept while
// Hersir runs (the Chat page unmounts when you leave it), and the panel's width, kept on this computer.
// - The chat's terminals are the ones opened in its panel (Open terminal, +, Ctrl+Shift+`, openChatTerminal) that are still
//   open, oldest first: a terminal that leaves the list (closed anywhere, or its shell exited) leaves them. Each has TF0's
//   one view (src/lib/terminals.ts), which the panel shows in the place { kind: "chat" }.
// - Closing the one shown shows the next tab; closing the last one hides the panel.
// - Its width is a share of the space right of the chat list, 45% the first time, the same in every chat
//   (gizai.chat.panel): at least 360px of panel and 400px of conversation. When the window has no room for the chat list
//   besides, the list hides while the panel shows.
// localStorage is read inside try/catch: the tests run in Node, which has none.
import { useSyncExternalStore } from "react";
import { terminalOpen } from "../api";
import { nextAfterClose, noteOpened, sentence, terminalsNow, watchTerminals } from "./terminals";

// ---------- the width (pure parts) ----------

export const CHAT_PANEL_KEY = "gizai.chat.panel";
/** The first width, and the one a double-click on the handle gives back: 45% of the space right of the chat list. */
export const CHAT_FIRST_SHARE = 45;
/** ← and → on the handle move it this much. */
export const CHAT_SHARE_STEP = 5;
/** The least width of the panel, and of the conversation while the panel shows (app.css .chat.has-term). */
export const CHAT_PANEL_MIN = 360;
export const CONVERSATION_MIN = 400;

function readKey(key: string): string | null { try { return localStorage.getItem(key); } catch { return null; } }
function writeKey(key: string, value: string) { try { localStorage.setItem(key, value); } catch { /* private mode */ } }

/** The share kept (0 to 100); nothing kept, or something unreadable, is the first 45%. */
export function chatShareOf(kept: string | null): number {
  try {
    const n: unknown = JSON.parse(kept ?? "null");
    return typeof n === "number" && Number.isFinite(n) ? Math.min(100, Math.max(0, n)) : CHAT_FIRST_SHARE;
  } catch { return CHAT_FIRST_SHARE; }
}
export const loadChatShare = () => chatShareOf(readKey(CHAT_PANEL_KEY));
export const keepChatShare = (share: number) => writeKey(CHAT_PANEL_KEY, JSON.stringify(Math.round(share * 10) / 10));

/** The panel's width for a wanted width in pixels, within its limits: at least CHAT_PANEL_MIN of panel and CONVERSATION_MIN
 *  of conversation. `panel` and `conversation` are their widths now; the conversation gives what the panel takes. When
 *  there is no room for both, the panel keeps its least width. */
export function clampChatPanel(want: number, panel: number, conversation: number): number {
  const most = panel + conversation - CONVERSATION_MIN;
  return Math.max(CHAT_PANEL_MIN, Math.min(want, most));
}
/** The share of the space right of the chat list (0 to 100) a panel width in pixels is. */
export function chatShareFor(width: number, space: number): number {
  if (!(space > 0)) return CHAT_FIRST_SHARE;
  return Math.min(100, Math.max(0, (width / space) * 100));
}

/** The chat list's width as app.css draws it (.chat's --chat-list): 260px, 220px in a window up to 1100px wide, and 6px
 *  more for each pixel the text grows in Settings → Appearance (--ui-grow). */
export const chatListWidth = (narrow: boolean, uiGrow: number) => (narrow ? 220 : 260) + 6 * uiGrow;
/** Whether the chat list has room beside the panel in a chat `chat` pixels wide: the space right of the list holds the
 *  panel's and the conversation's least widths. */
export const listFits = (chat: number, list: number) => chat - list >= CHAT_PANEL_MIN + CONVERSATION_MIN;

// ---------- the chat's terminals, while Hersir runs ----------

export type ChatTerminals = {
  /** The chat's terminals, oldest first. */
  ids: string[];
  /** The one the panel shows, or null with none. */
  current: string | null;
  /** Whether the panel shows; it does only with terminals. */
  open: boolean;
  /** Counts up each time a terminal is opened, picked or shown in the panel: the terminal shown then takes the focus. */
  focus: number;
};

/** Every terminal opened in the panel while Hersir runs, oldest first; the chat's terminals are those still open. */
const opened: string[] = [];
let state: ChatTerminals = { ids: [], current: null, open: false, focus: 0 };
const subs = new Set<() => void>();
const set = (next: Partial<ChatTerminals>) => { state = { ...state, ...next }; subs.forEach((f) => f()); };

/** The list changed: a terminal that left it leaves the chat's. The one shown gives way to the next tab (else the one
 *  before it), and with the last one gone the panel hides. */
function update() {
  const live = new Set(terminalsNow().map((s) => s.id));
  const ids = opened.filter((id) => live.has(id));
  if (ids.length === state.ids.length && ids.every((id, i) => id === state.ids[i])) return;
  const was = state.current;
  const current = was && ids.includes(was) ? was : was ? nextAfterClose(state.ids, was, ids) : ids[ids.length - 1] ?? null;
  set({ ids, current, open: state.open && ids.length > 0 });
}

let watching = false;
/** The chat's terminals follow the list from their first use on. */
function watch() {
  if (watching) return;
  watching = true;
  watchTerminals(update);
}

/** Shows the panel on the terminal, which takes the focus. */
function showTerminal(id: string) {
  set({ current: id, open: true, focus: state.focus + 1 });
}

/** Opens a terminal in the folder (an EditorFolder id) in the Chat page's terminal panel, shows the panel on it and gives it the focus.
 *  reuse: the newest of the panel's terminals in that folder that waits for a command (not busy, not ended) shows instead of a new one.
 *  Resolves with why it couldn't start, as a sentence, or null. */
export function openChatTerminal(folder: string, reuse: boolean): Promise<string | null> {
  watch();
  if (reuse) {
    const list = terminalsNow();
    const waiting = [...state.ids].reverse().map((id) => list.find((s) => s.id === id))
      .find((s) => !!s && s.folder === folder && !s.busy && !s.ended);
    if (waiting) { showTerminal(waiting.id); return Promise.resolve(null); }
  }
  // 0 × 0: the backend starts it at 80 × 24, and the panel fits it to its place at once
  return terminalOpen(folder, 0, 0, null).then(
    (s) => {
      noteOpened(s);
      opened.push(s.id);
      update();
      showTerminal(s.id);
      return null;
    },
    (e) => sentence(String(e).replace(/^Error:\s*/, "")),
  );
}

/** A tab was picked (a click, ← and →, Ctrl+PgDn and Ctrl+PgUp): it shows, with the focus. */
export function pickChatTerminal(id: string) {
  if (state.ids.includes(id)) showTerminal(id);
}

/** Shows the panel on the terminal it had (the rail, Ctrl+`), with the focus; only while the chat has terminals. */
export function showChatPanel() {
  if (state.ids.length) set({ open: true, focus: state.focus + 1 });
}

/** Hides the panel (Hide, Ctrl+`): its shells keep running, and the rail shows how many there are. */
export function hideChatPanel() {
  if (state.open) set({ open: false });
}

const subscribe = (f: () => void) => {
  watch();
  subs.add(f);
  return () => { subs.delete(f); };
};
/** The chat's terminals, the one shown, whether the panel shows (the same snapshot on a server render, as tests do). */
export function useChatTerminals(): ChatTerminals {
  return useSyncExternalStore(subscribe, () => state, () => state);
}

/** What the Chat page has on the right of the conversation: the panel, the rail (the panel hidden, with terminals) or
 *  nothing. A string, so the page only draws again when it changes. */
const showing = (): "panel" | "rail" | null => (!state.ids.length ? null : state.open && state.current ? "panel" : "rail");
export function useChatTerminalShows(): "panel" | "rail" | null {
  return useSyncExternalStore(subscribe, showing, showing);
}
