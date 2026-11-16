// The Chief Chat's side island and its tool strip (GA-296; design/islands-chat/README.md): what each chat has open beside
// it, kept while Hersir runs (the Chat page unmounts when you leave it).
// - A tab has a kind (Terminal, Editor, Board, Card), and an id within its kind (a terminal's id for now; a card's, a
//   file's or a board's come with cards 8 and 9). Its icon and title come from its kind (components/chat/ChatSide.tsx).
// - Tabs belong to the chat they were opened in, in the order they opened (a new one goes at the end). Another chat shows
//   its own; a chat's terminals keep running while another chat is shown. New chat (no chat yet) has its own until the
//   first message makes the chat, which takes them (adoptNewChat).
// - Closing the shown tab shows the one to its right, else its left; closing the last hides the island, and the strip goes.
// - The strip: one icon per kind open in the chat, in the order Terminal, Editor, Board, Card, with the count. A click shows
//   that kind's last shown tab (showing the island), or hides the island when that kind is on show.
// - A terminal that leaves Hersir's list (closed anywhere, or its shell exited) leaves every chat's tabs.
import { useSyncExternalStore } from "react";
import { isMac } from "./keys";
import { nextAfterClose, terminalsNow, watchTerminals } from "./terminals";

// ---------- the kinds (pure parts) ----------

export type SideKind = "terminal" | "editor" | "board" | "card";
/** The strip's order, and Alt+1 to Alt+4's. */
export const SIDE_KINDS: readonly SideKind[] = ["terminal", "editor", "board", "card"];
export const KIND_NAME: Record<SideKind, string> = { terminal: "Terminal", editor: "Editor", board: "Board", card: "Card" };

export type SideTab = { kind: SideKind; id: string };
/** A tab's key: its kind and its id ("terminal:7f3a…"). */
export const tabKey = (t: SideTab) => `${t.kind}:${t.id}`;
export const kindOf = (key: string): SideKind => key.slice(0, key.indexOf(":")) as SideKind;
export const idOf = (key: string): string => key.slice(key.indexOf(":") + 1);

/** One chat's side island: its tabs in the order they opened, the one shown, whether the island shows, and each kind's
 *  last shown tab (a click on the kind's icon shows it). */
export type ChatSide = {
  tabs: SideTab[];
  current: string | null;
  open: boolean;
  last: Partial<Record<SideKind, string>>;
};
export const NO_SIDE: ChatSide = { tabs: [], current: null, open: false, last: {} };

/** The kinds open in the chat, in the strip's order, with how many of each. */
export function kindsOpen(side: Pick<ChatSide, "tabs">): { kind: SideKind; count: number }[] {
  return SIDE_KINDS.map((kind) => ({ kind, count: side.tabs.filter((t) => t.kind === kind).length })).filter((k) => k.count > 0);
}
/** The kind on show (its icon selected), or null while the island is hidden. */
export const shownKind = (side: ChatSide): SideKind | null => (side.open && side.current ? kindOf(side.current) : null);
/** The kind's last shown tab, else its newest, else null. */
export function lastOfKind(side: ChatSide, kind: SideKind): string | null {
  const keys = side.tabs.filter((t) => t.kind === kind).map(tabKey);
  const last = side.last[kind];
  return last && keys.includes(last) ? last : keys[keys.length - 1] ?? null;
}

/** The tab shown, the island open, and the tab as its kind's last. */
export function withShown(side: ChatSide, key: string): ChatSide {
  if (!side.tabs.some((t) => tabKey(t) === key)) return side;
  return { ...side, current: key, open: true, last: { ...side.last, [kindOf(key)]: key } };
}
/** A tab opened: at the end (where it was, when it is open already), shown. */
export function withTab(side: ChatSide, tab: SideTab): ChatSide {
  const key = tabKey(tab);
  const tabs = side.tabs.some((t) => tabKey(t) === key) ? side.tabs : [...side.tabs, tab];
  return withShown({ ...side, tabs }, key);
}
/** Tabs closed: the shown one gives way to the one to its right, else its left; with none left the island hides. */
export function withoutTabs(side: ChatSide, gone: Set<string>): ChatSide {
  const was = side.tabs.map(tabKey);
  const tabs = side.tabs.filter((t) => !gone.has(tabKey(t)));
  if (tabs.length === side.tabs.length) return side;
  const now = tabs.map(tabKey);
  const current = side.current && now.includes(side.current) ? side.current
    : side.current ? nextAfterClose(was, side.current, now) : now[now.length - 1] ?? null;
  const last = Object.fromEntries(Object.entries(side.last).filter(([, k]) => !!k && now.includes(k)));
  if (current && side.open) last[kindOf(current)] = current;
  return { tabs, current, open: side.open && tabs.length > 0, last };
}
/** A click on the kind's icon (or its key): hides the island while that kind is on show, else shows its last tab. */
export function toggledKind(side: ChatSide, kind: SideKind): ChatSide {
  if (shownKind(side) === kind) return { ...side, open: false };
  const key = lastOfKind(side, kind);
  return key ? withShown(side, key) : side;
}

// ---------- the strip's words (design: The tool strip) ----------

/** The kind's key: Ctrl+` for Terminal (on all three systems, as today), Alt+2 to Alt+4 for the others (Option on macOS). */
export const kindKey = (kind: SideKind, mac = isMac()) =>
  kind === "terminal" ? "Ctrl+`" : `${mac ? "Option" : "Alt"}+${SIDE_KINDS.indexOf(kind) + 1}`;
/** "Terminal, 2 open": the icon's label for screen readers, and its tooltip before the key. */
export const stripLabel = (kind: SideKind, count: number) => `${KIND_NAME[kind]}, ${count} open`;
/** "Terminal, 2 open (Ctrl+`)": the icon's title. */
export const stripTitle = (kind: SideKind, count: number, mac = isMac()) => `${stripLabel(kind, count)} (${kindKey(kind, mac)})`;
/** "Card · 3 open": the head of the icon's menu. */
export const stripMenuHead = (kind: SideKind, count: number) => `${KIND_NAME[kind]} · ${count} open`;

/** Close all on the Terminal icon while programs run (today's question, for one or more): "npm is still running in gizai 2.
 *  Close it and end npm?", "npm is still running in gizai 2, and sleep in gizai. Close all and end them?". */
export function closeAllQuestion(busy: { name: string; busy: string }[]): string {
  if (busy.length === 1) { const [s] = busy; return `${s!.busy} is still running in ${s!.name}. Close it and end ${s!.busy}?`; }
  const parts = busy.map((s) => `${s.busy} in ${s.name}`);
  parts[0] = `${busy[0]!.busy} is still running in ${busy[0]!.name}`;
  const list = parts.length === 2 ? `${parts[0]}, and ${parts[1]}` : `${parts.slice(0, -1).join(", ")}, and ${parts[parts.length - 1]}`;
  return `${list}. Close all and end them?`;
}

/** Alt+1 to Alt+4 (Option on macOS), read by where the key is (Digit1…), so the typed character doesn't matter: the kind
 *  whose icon it clicks. Alt+1 is Terminal's second key, after Ctrl+`. */
export function sideKindKey(e: { code: string; ctrlKey: boolean; shiftKey: boolean; altKey: boolean; metaKey: boolean }): SideKind | null {
  if (!e.altKey || e.ctrlKey || e.metaKey || e.shiftKey) return null;
  const m = /^Digit([1-4])$/.exec(e.code);
  return m ? SIDE_KINDS[Number(m[1]) - 1] ?? null : null;
}

// ---------- every chat's side island, while Hersir runs ----------

/** The key of New chat (no chat yet) and of Chat → Archive, which have their own tabs. */
export const NEW_CHAT = "new";
export const ARCHIVE = "archive";
export const chatSideKey = (id: string | undefined, archive: boolean) => id ?? (archive ? ARCHIVE : NEW_CHAT);

const chats = new Map<string, ChatSide>();
let chat = NEW_CHAT;
/** Counts up each time a tab is opened, picked or shown on the page: the tab shown then takes the focus. */
let focus = 0;
export type SideSnapshot = { chat: string; side: ChatSide; focus: number };
let snap: SideSnapshot = { chat, side: NO_SIDE, focus };
const subs = new Set<() => void>();

const sideOf = (key: string) => chats.get(key) ?? NO_SIDE;
function setSide(key: string, next: ChatSide, focusIt = false) {
  if (next.tabs.length) chats.set(key, next);
  else chats.delete(key);
  if (focusIt) focus += 1;
  publish();
}
function publish() {
  const side = sideOf(chat);
  if (snap.chat !== chat || snap.side !== side || snap.focus !== focus) snap = { chat, side, focus };
  subs.forEach((f) => f());
}

/** The list changed: a terminal that left it leaves every chat's tabs. */
function update() {
  const live = new Set(terminalsNow().map((s) => s.id));
  let changed = false;
  for (const [key, side] of chats) {
    const gone = new Set(side.tabs.filter((t) => t.kind === "terminal" && !live.has(t.id)).map(tabKey));
    if (!gone.size) continue;
    const next = withoutTabs(side, gone);
    if (next.tabs.length) chats.set(key, next);
    else chats.delete(key);
    changed = true;
  }
  if (changed) publish();
}
let watching = false;
function watch() {
  if (watching) return;
  watching = true;
  watchTerminals(update);
}

/** The chat the page shows (its id, NEW_CHAT or ARCHIVE): the island and the strip are its. */
export function showChat(key: string) {
  if (key === chat) return;
  chat = key;
  publish();
}
/** The chat on show now, for what opens a tab later (a terminal that starts): it goes to the chat it was asked in. */
export const chatNow = () => chat;
/** The chat's side island now, for code outside React. */
export const sideNow = (key = chat): ChatSide => sideOf(key);

/** The first message made New chat's chat: it takes the tabs New chat had. */
export function adoptNewChat(id: string) {
  const had = chats.get(NEW_CHAT);
  if (!had || chats.has(id)) return;
  chats.delete(NEW_CHAT);
  chats.set(id, had);
  publish();
}

/** Opens a tab in the chat (the one on show by default) and shows it, with the focus. */
export function openSideTab(tab: SideTab, key = chat) {
  watch();
  setSide(key, withTab(sideOf(key), tab), key === chat);
}
/** A tab picked (a click, ← and →, Ctrl+PgDn and Ctrl+PgUp): it shows, with the focus. */
export function showSideTab(key: string) {
  const side = sideOf(chat);
  if (side.tabs.some((t) => tabKey(t) === key)) setSide(chat, withShown(side, key), true);
}
/** Tabs leave the chat (closed, or moved to their own page). The caller ends what they hold (a terminal's shell). */
export function closeSideTabs(keys: string[], key = chat) {
  setSide(key, withoutTabs(sideOf(key), new Set(keys)));
}
/** Shows the island on the tab it had, with the focus; only while the chat has tabs. */
export function showSide() {
  const side = sideOf(chat);
  const newest = side.tabs[side.tabs.length - 1];
  const key = side.current ?? (newest ? tabKey(newest) : null);
  if (key) setSide(chat, withShown(side, key), true);
}
/** Hides the island (Hide, a click on the selected icon): terminals keep running, the strip stays. */
export function hideSide() {
  const side = sideOf(chat);
  if (side.open) setSide(chat, { ...side, open: false });
}
/** A click on the kind's icon in the strip, or its key: shows that kind's last tab, or hides the island while it is on show.
 *  Whether anything changed (a kind with nothing open does nothing). */
export function toggleSideKind(kind: SideKind): boolean {
  const side = sideOf(chat);
  const next = toggledKind(side, kind);
  if (next === side) return false;
  setSide(chat, next, next.open);
  return true;
}

const subscribe = (f: () => void) => {
  watch();
  subs.add(f);
  return () => { subs.delete(f); };
};
/** The chat on show, its side island and the focus count (the same snapshot on a server render, as tests do). */
export function useChatSide(): SideSnapshot {
  return useSyncExternalStore(subscribe, () => snap, () => snap);
}

/** What the Chat page has right of the conversation: the island with the strip, the strip alone (the island hidden), or
 *  nothing. A string, so the page only draws again when it changes. */
const showing = (): "island" | "strip" | null => (!snap.side.tabs.length ? null : snap.side.open && snap.side.current ? "island" : "strip");
export function useSideShows(): "island" | "strip" | null {
  return useSyncExternalStore(subscribe, showing, showing);
}
