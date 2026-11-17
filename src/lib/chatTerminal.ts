// The Chat page's terminals (GA-278, GA-296): Terminal tabs in the chat's side island (src/lib/chatSide.ts), and the side
// island's width, kept on this computer.
// - A chat's terminals are the Terminal tabs opened in it (New terminal, Ctrl+Shift+`, Open in terminal under an answer)
//   that are still open, in the order they opened. Each has TF0's one view (src/lib/terminals.ts), which the island shows in
//   the place { kind: "chat" }.
// - The island's width is a share of the room right of the chat list, 45% the first time, the same in every chat and for
//   every kind of tab (gizai.chat.panel): at least 360px of island and 400px of conversation. When the window has no room
//   for the chat list besides, the list hides while the island shows; the tool strip always stays.
// localStorage is read inside try/catch: the tests run in Node, which has none.
import { terminalOpen } from "../api";
import { chatNow, openSideTab, sideNow, tabKey, showSideTab } from "./chatSide";
import { noteOpened, sentence, terminalsNow } from "./terminals";

// ---------- the width (pure parts) ----------

export const CHAT_PANEL_KEY = "gizai.chat.panel";
/** The first width, and the one a double-click on the handle gives back: 45% of the space right of the chat list. */
export const CHAT_FIRST_SHARE = 45;
/** ← and → on the handle move it this much. */
export const CHAT_SHARE_STEP = 5;
/** The least width of the side island, and of the conversation while the island shows (app.css .chat.has-side). */
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
/** The gap between islands (island-gap), and the tool strip's buttons (size-tool) on the window's right edge. */
export const ISLAND_GAP = 6;
export const STRIP_WIDTH = 28;
/** Whether the chat list has room in a chat `chat` pixels wide while the tool strip shows: what is right of the list (less
 *  the gaps and the strip) holds the conversation's least width, and the side island's while it shows (`island`). */
export const listFits = (chat: number, list: number, island = true) =>
  island ? chat - list - 3 * ISLAND_GAP - STRIP_WIDTH >= CHAT_PANEL_MIN + CONVERSATION_MIN : chat - list - 2 * ISLAND_GAP - STRIP_WIDTH >= CONVERSATION_MIN;

// ---------- a chat's terminals ----------

/** The chat's terminals, in the order they opened (the chat on show by default). */
export function chatTerminalIds(key = chatNow()): string[] {
  return sideNow(key).tabs.filter((t) => t.kind === "terminal").map((t) => t.id);
}

/** Opens a terminal in the folder (an EditorFolder id) in the chat's side island, shows it and gives it the focus. It
 *  goes to the chat it was asked in, also when another chat shows by the time it has started.
 *  reuse: the newest of the chat's terminals in that folder that waits for a command (not busy, not ended) shows instead of a new one.
 *  Resolves with why it couldn't start, as a sentence, or null. */
export function openChatTerminal(folder: string, reuse: boolean): Promise<string | null> {
  const chat = chatNow();
  if (reuse) {
    const list = terminalsNow();
    const waiting = [...chatTerminalIds(chat)].reverse().map((id) => list.find((s) => s.id === id))
      .find((s) => !!s && s.folder === folder && !s.busy && !s.ended);
    if (waiting) { showSideTab(tabKey({ kind: "terminal", id: waiting.id })); return Promise.resolve(null); }
  }
  // 0 × 0: the backend starts it at 80 × 24, and the island fits it to its place at once
  return terminalOpen(folder, 0, 0, null).then(
    (s) => {
      noteOpened(s);
      openSideTab({ kind: "terminal", id: s.id }, chat);
      return null;
    },
    (e) => sentence(String(e).replace(/^Error:\s*/, "")),
  );
}
