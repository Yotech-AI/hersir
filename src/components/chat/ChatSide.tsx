// The Chief Chat's side island and its tool strip (GA-296; design/islands-chat/README.md), in place of GA-278's terminal
// panel and its rail. What is open beside a chat is src/lib/chatSide.ts's, kept per chat while Hersir runs.
// - The side island, right of the conversation: a header with the chat's tabs (each kind's icon and title; Terminal,
//   Card (GA-297, ChatCard.tsx), Board and Editor (GA-298, ChatBoard.tsx and ChatEditor.tsx)), + (a menu: New terminal,
//   Open a file…, Board), Own page and Hide; then the shown tab.
//   Its left edge, the gap to the conversation, is the drag handle; its width is kept on this computer (gizai.chat.panel).
// - The tool strip, on the background along the window's right edge, while the chat has anything open: an icon per kind
//   open, with its count, the shown kind's selected. A click shows that kind's last tab, or hides the island while that
//   kind is on show; a right click (or the context menu key) opens a menu: Show or Hide, and Close all.
// - The top bar's Open, before New chat: a menu with New terminal, Open a file… (Go to file over the chat project's folder,
//   else the last Editor tab's, else the folder picker first) and Board, the same as the header's +.
// - Keys on the Chat page: Ctrl+` and Alt+1 show or hide Terminal, Alt+2, Alt+3 and Alt+4 Editor, Board and Card (Option on
//   macOS); with none of that kind open Alt+2 opens Open a file…, Alt+3 the Board tab and Alt+4 nothing; Ctrl+Shift+` starts
//   a terminal in the shown terminal's folder (the folder picker with none); in the island, Ctrl+PgDn and Ctrl+PgUp switch
//   tabs, and Ctrl+W (Cmd+W) closes the shown tab but a terminal (an Editor tab with changes asks first). In a terminal only
//   Ctrl+` and Ctrl+Shift+` are Hersir's: Alt with a digit reaches the shell.
// The page itself only follows whether the island or the strip shows (useChatSideLayout), so the conversation isn't drawn
// again when a terminal changes or the island is dragged.
import { useEffect, useLayoutEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import { createPortal } from "react-dom";
import {
  ChevronDown, ChevronRight, CircleAlert, Columns3, FileCode, Maximize2, PanelRight, Plus, SquareKanban, Terminal as TerminalIcon, X,
  type LucideIcon,
} from "lucide-react";
import { terminalClose } from "../../api";
import type { TerminalSession } from "../../types";
import { go } from "../../router";
import { openItem } from "../../lib/openItem";
import { stepTab } from "../../lib/editor";
import { asksToClose, focusTerminal, folderNameOf, showIn, useTerminals } from "../../lib/terminals";
import { panelKey } from "../../lib/terminalPanel";
import {
  CHAT_FIRST_SHARE, CHAT_SHARE_STEP, chatListWidth, chatShareFor, clampChatPanel, keepChatShare, listFits, loadChatShare, openChatTerminal,
} from "../../lib/chatTerminal";
import {
  closeAllQuestion, closeSideTabs, editorTabOf, idOf, kindKey, kindOf, kindsOpen, lastOfKind, openBoardTab, shownKind, showChat, showSide, showSideTab,
  sideKindKey, stripLabel, stripMenuHead, stripTitle, tabKey, toggleSideKind, hideSide, useChatSide, useSideShows, type ChatSide, type SideKind,
} from "../../lib/chatSide";
import { nameOf, tabNames } from "../../lib/editor";
import type { EditorFolder } from "../../types";
import { ContextMenu, type MenuItem } from "../editor/Menus";
import { Handle, TermTab, type HandleEdge } from "../editor/TerminalPanel";
import { FolderPicker } from "../editor/FolderPicker";
import { GoToFile } from "../editor/GoToFile";
import { ChatTerminalBody, OpenTerminalPicker, useStartOff } from "./ChatTerminal";
import { CardTab, ChatCardBody } from "./ChatCard";
import { BoardTab, boardName, boardOwnPage, ChatBoardBody } from "./ChatBoard";
import { ChatEditorBody, closeEditorTabs, EditorCloseAsk, EditorTab, editorOwnPage, openEditorTab } from "./ChatEditor";
import { useEditorFolders } from "./OpenInTerminal";
import { useChatProject } from "./useChatProject";

const KIND_ICON: Record<SideKind, LucideIcon> = { terminal: TerminalIcon, editor: FileCode, board: Columns3, card: SquareKanban };
const why = (e: unknown) => String(e).replace(/^Error:\s*/, "");
/** Under a button, for what opens from it (a menu, a picker). */
const under = (el: Element | null | undefined) => { const r = el?.getBoundingClientRect(); return r ? { x: r.left, y: r.bottom + 6 } : null; };

/** The island's left edge: its width is a share of the room right of the chat list (the conversation and the island), at
 *  least 360px of island and 400px of conversation. */
const SIDE_EDGE: HandleEdge = {
  side: "left", label: "Side island's width", clamp: clampChatPanel, share: chatShareFor, first: CHAT_FIRST_SHARE, step: CHAT_SHARE_STEP,
  keep: keepChatShare,
  measure: (el) => {
    const talk = el.parentElement?.querySelector<HTMLElement>(":scope > .chat-main");
    if (!talk) return null;
    const size = el.getBoundingClientRect().width;
    const other = talk.getBoundingClientRect().width;
    return { size, other, whole: size + other };
  },
};

// ---------- the page's layout ----------

/** .chat's ref and class for the chat `key` (chatSideKey; `on`: the Chief is set up, so the chat shows): the side island is
 *  a third column with the strip after it while it shows, and the chat list hides then when the window has no room for it
 *  besides; the strip alone is one while the island is hidden with tabs open. */
export function useChatSideLayout(on: boolean, key: string) {
  useLayoutEffect(() => { showChat(key); }, [key]);
  const shows = useSideShows();
  const [chat, chatRef] = useState<HTMLDivElement | null>();
  const fits = useListFits(chat ?? null, on && !!shows, shows === "island");
  const className = !on || !shows ? "chat" : `chat${shows === "island" ? " has-side" : ""} has-strip${fits ? "" : " no-list"}`;
  return { chatRef, className };
}

/** Whether the chat list has room beside the conversation, the strip and (`island`) the side island: measured while the strip
 *  shows, and again when the chat changes size. */
function useListFits(chat: HTMLDivElement | null, on: boolean, island: boolean): boolean {
  const [fits, setFits] = useState(true);
  useLayoutEffect(() => {
    if (!on || !chat) return;
    const check = () => {
      const grow = parseFloat(getComputedStyle(document.documentElement).getPropertyValue("--ui-grow")) || 0;
      setFits(listFits(chat.getBoundingClientRect().width, chatListWidth(window.matchMedia("(max-width: 1100px)").matches, grow), island));
    };
    check();
    const ro = new ResizeObserver(check);
    ro.observe(chat);
    return () => ro.disconnect();
  }, [chat, on, island]);
  return fits;
}

// ---------- a new terminal (+, Open, Ctrl+Shift+`) ----------

/** Why the last New terminal couldn't start, shown in the island until another tab shows. */
let failed: string | undefined;
const failSubs = new Set<() => void>();
const setFailed = (m: string | undefined) => { if (m === failed) return; failed = m; failSubs.forEach((f) => f()); };
const subFailed = (f: () => void) => { failSubs.add(f); return () => { failSubs.delete(f); }; };
const useFailed = () => useSyncExternalStore(subFailed, () => failed, () => failed);
let starting = false;
/** Opens the folder picker under the top bar's Open (New terminal with no terminal in the chat, Ctrl+Shift+`). */
let pickUnderOpen: (() => void) | null = null;

/** The terminal whose folder New terminal starts in: the chat's last shown terminal (the shown one while the island shows
 *  a terminal), or null with none; and why a terminal can't start there now. */
function useNewTerminal(side: ChatSide): { s: TerminalSession | null; off: string | null } {
  const all = useTerminals();
  const key = lastOfKind(side, "terminal");
  const s = (key && all.find((x) => x.id === idOf(key))) || null;
  return { s, off: useStartOff(s?.folder ?? null) };
}

/** New terminal: in that terminal's folder, else the folder picker; while none can start there, the island with the
 *  terminals it has. */
function newTerminal(s: TerminalSession | null, off: string | null) {
  if (!s) { pickUnderOpen?.(); return; }
  if (off) { showSide(); return; }
  if (starting) return;
  starting = true;
  setFailed(undefined);
  openChatTerminal(s.folder, false).then((w) => {
    starting = false;
    if (w) { setFailed(w); showSide(); }
  });
}

/** The items of Open's menu and of +'s: New terminal, and New terminal in… (the picker) while it goes to a folder by itself;
 *  Open a file… and Board (GA-298). `under`: where the folder picker of Open a file… shows when it needs one. */
function openItems(s: TerminalSession | null, off: string | null, pick: () => void, under: () => { x: number; y: number } | null): MenuItem[] {
  const name = s ? folderNameOf(s) : null;
  const items: MenuItem[] = [{ label: "New terminal", kbd: "Ctrl+Shift+`", disabled: !!off, title: off ?? (name ? `New terminal in ${name}` : "New terminal in a folder you pick"),
    run: () => newTerminal(s, off) }];
  if (s) items.push({ label: "New terminal in…", title: "New terminal in a folder you pick", run: pick });
  items.push(
    { label: "Open a file…", kbd: kindKey("editor"), title: "Go to a file in the chat's project and open it beside the chat", run: () => openAFile(under()) },
    { label: "Board", kbd: kindKey("board"), title: "The board of the chat's project beside the chat", run: openBoardTab },
  );
  return items;
}

// ---------- Open a file… (+, Open, Alt+2 with no Editor tab) ----------

/** Open a file… asked (`at`: where the folder picker shows when it needs one); `n` counts the asks. */
let fileAsk: { n: number; at: { x: number; y: number } | null } | null = null;
const fileSubs = new Set<() => void>();
const subFile = (f: () => void) => { fileSubs.add(f); return () => { fileSubs.delete(f); }; };
function openAFile(at: { x: number; y: number } | null) {
  fileAsk = { n: (fileAsk?.n ?? 0) + 1, at };
  fileSubs.forEach((f) => f());
}

/** Open a file…: the Editor's Go to file box over the chat project's folder, else over the last shown Editor tab's folder,
 *  else the folder picker first ("Open a file in"). The file opens in an Editor tab; one in a card's worktree or in a folder
 *  an agent works in opens read-only. `you`: for the picker. */
function OpenAFile({ side, you }: { side: ChatSide; you: string }) {
  const asked = useSyncExternalStore(subFile, () => fileAsk, () => fileAsk);
  const folders = useEditorFolders(!!asked);
  const { project } = useChatProject();
  const [goto, setGoto] = useState<EditorFolder | null>(null);
  const [picker, setPicker] = useState<{ x: number; y: number } | null>(null);
  // an ask made before this mount (the Chat page drawn again after another page) was handled then: it doesn't open again
  const done = useRef(fileAsk?.n ?? 0);
  useEffect(() => {
    if (!asked || asked.n === done.current || !folders || project === undefined) return;
    done.current = asked.n;
    const last = lastOfKind(side, "editor");
    const lastFolder = last ? editorTabOf(idOf(last)).folder : null;
    const f = (project && folders.find((x) => x.kind === "project" && x.projectId === project.id))
      || (lastFolder ? folders.find((x) => x.id === lastFolder) : undefined) || null;
    if (f) setGoto(f);
    else setPicker(asked.at ?? { x: Math.max(8, window.innerWidth - 420), y: 56 });
  }, [asked, folders, project]); // eslint-disable-line react-hooks/exhaustive-deps
  const lock = (f: EditorFolder) => f.kind === "card" || f.readOnly;
  return (
    <>
      {picker && <FolderPicker at={picker} title="Open a file in" you={you} onClose={() => setPicker(null)}
        onPick={(f: EditorFolder) => { setPicker(null); setGoto(f); }} />}
      {goto && <GoToFile folder={goto.id} name={goto.name} onClose={() => setGoto(null)}
        onOpen={(p) => openEditorTab(goto.id, p, null, lock(goto))} />}
    </>
  );
}

// ---------- the island and the strip ----------

/** What is open beside the chat, as .chat's last columns: the side island while it shows, and the strip while the chat has
 *  anything open; and the page's keys. A tab shown when the page opens doesn't take the focus; one opened, picked or shown
 *  on the page does. `youId`: for the folder picker. */
export function ChatSide({ youId }: { youId: string }) {
  const { chat, side, focus } = useChatSide();
  const all = useTerminals();
  const fresh = useNewTerminal(side);
  const islandRef = useRef<HTMLDivElement>(null);
  const shown = side.open && !!side.current;
  // what was asked before the page or the chat showed doesn't move the focus: another chat's terminal coming in doesn't take it
  const since = useRef({ chat, focus });
  if (since.current.chat !== chat) since.current = { chat, focus };

  /** Hides the island: with the focus in it, the focus goes to the text box (the Archive's search). */
  const hide = () => {
    const inside = !!islandRef.current?.contains(document.activeElement);
    hideSide();
    if (inside) requestAnimationFrame(() => document.querySelector<HTMLElement>(".chat-main .composer-editor .cm-content, .chat-main .archive-search")?.focus());
  };
  /** A click on a kind's icon, or its key. */
  const toggle = (kind: SideKind) => {
    if (shownKind(side) === kind) hide();
    else toggleSideKind(kind);
  };

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.defaultPrevented) return;
      const k = panelKey(e);
      if (k === "toggle") {
        if (!side.tabs.some((t) => t.kind === "terminal")) return;
        e.preventDefault();
        toggle("terminal");
      } else if (k === "new") {
        e.preventDefault();
        newTerminal(fresh.s, fresh.off);
      } else {
        // Alt+1 to Alt+4; in a terminal they reach the shell. With none of that kind open, Alt+2 opens Open a file… and
        // Alt+3 the Board tab.
        const kind = sideKindKey(e);
        if (!kind || (e.target instanceof Element && e.target.closest(".xterm"))) return;
        e.preventDefault();
        if (side.tabs.some((t) => t.kind === kind)) toggle(kind);
        else if (kind === "editor") openAFile(null);
        else if (kind === "board") openBoardTab();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  // Open a file… and the Editor's question stay in their places with or without tabs (they float: no grid cell of .chat)
  const any = side.tabs.length > 0;
  return (
    <>
      {any && shown && <SideIsland islandRef={islandRef} side={side} all={all} focus={focus} focusNext={focus > since.current.focus} fresh={fresh} you={youId} hide={hide} />}
      {any && <ToolStrip side={side} all={all} toggle={toggle} />}
      <OpenAFile side={side} you={youId} />
      <EditorCloseAsk />
    </>
  );
}

/** The side island: its left edge the drag handle; a header with the chat's tabs, +, Own page and Hide; why a terminal
 *  couldn't start or close; and the shown tab. Its width is .chat's --chat-side-share. */
function SideIsland({ islandRef, side, all, focus, focusNext, fresh, you, hide }: {
  islandRef: React.RefObject<HTMLDivElement | null>; side: ChatSide; all: TerminalSession[]; focus: number; focusNext: boolean;
  fresh: { s: TerminalSession | null; off: string | null }; you: string; hide: () => void;
}) {
  const [share, setShare] = useState(loadChatShare);
  const [asking, setAsking] = useState<{ s: TerminalSession; x: number; y: number } | null>(null);
  const [renaming, setRenaming] = useState<string | null>(null);
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const [picker, setPicker] = useState<{ x: number; y: number } | null>(null);
  const plusRef = useRef<HTMLButtonElement>(null);
  const note = useFailed();
  const current = side.current!;
  const keys = side.tabs.map(tabKey);
  const terms = useMemo(() => side.tabs.filter((t) => t.kind === "terminal").flatMap((t) => all.filter((s) => s.id === t.id)), [all, side.tabs]);
  const termIds = terms.map((s) => s.id);
  const shownTerm = kindOf(current) === "terminal" ? terms.find((s) => s.id === idOf(current)) ?? null : null;
  const shownCard = kindOf(current) === "card" ? idOf(current) : null;
  const shownFile = kindOf(current) === "editor" ? idOf(current) : null;
  const shownBoard = kindOf(current) === "board";
  const { project } = useChatProject();
  // the Editor tabs' names: two with the same name get their folder, as the Editor's tabs do
  const fileNames = useMemo(() => {
    const ids = side.tabs.filter((t) => t.kind === "editor").map((t) => t.id);
    const names = tabNames(ids.map((id) => editorTabOf(id).path));
    return new Map(ids.map((id) => [id, names.get(editorTabOf(id).path) ?? { name: nameOf(editorTabOf(id).path), dir: null }]));
  }, [side.tabs]);

  // the grid's third column (app.css .chat.has-side) is as wide as the share says; set here, so a drag draws the island and
  // the conversation's column again, not the page
  useLayoutEffect(() => {
    const chat = islandRef.current?.parentElement;
    chat?.style.setProperty("--chat-side-share", String(share / 100));
    return () => { chat?.style.removeProperty("--chat-side-share"); };
  }, [share]); // eslint-disable-line react-hooks/exhaustive-deps

  // a terminal opened or picked again while its view is in the island takes the focus now (a view that comes in takes it
  // once it is in: TerminalView's autoFocus)
  const asked = useRef(focus);
  useEffect(() => {
    if (focus === asked.current) return;
    asked.current = focus;
    if (shownTerm) focusTerminal(shownTerm.id);
  }, [focus]); // eslint-disable-line react-hooks/exhaustive-deps

  // why New terminal couldn't start, or a tab couldn't close, goes once another tab shows (not when the island comes in
  // to show it)
  const was = useRef(current);
  useEffect(() => {
    if (was.current === current) return;
    was.current = current;
    setFailed(undefined);
  }, [current]);

  const end = (t: TerminalSession) => { terminalClose(t.id).catch((e) => setFailed(`Couldn't close ${t.name}: ${why(e)}`)); };
  const close = (t: TerminalSession, el: HTMLElement | null) => {
    if (!asksToClose(t)) { end(t); return; }
    const r = el?.getBoundingClientRect();
    setAsking({ s: t, x: r?.left ?? 200, y: (r?.bottom ?? 100) + 4 });
  };
  const name = shownTerm?.name ?? (shownFile ? fileNames.get(shownFile)?.name ?? nameOf(editorTabOf(shownFile).path) : shownBoard ? boardName(project) : idOf(current));
  /** Where a question about closing the shown tab shows: under its tab. */
  const underTab = () => {
    const r = islandRef.current?.querySelector(`[data-tab="${CSS.escape(current)}"]`)?.getBoundingClientRect();
    return r ? { x: r.left, y: r.bottom + 4 } : null;
  };
  /** Own page: the shown tab on its own page (a terminal on the Terminal page, a card on its page, the board on the Tasks
   *  page, a file on the Editor page at its line); its tab closes here. */
  const ownPage = () => {
    if (shownCard) {
      closeSideTabs([current]);
      openItem("task", shownCard).catch(() => {});
      return;
    }
    if (shownBoard) { boardOwnPage(project); return; }
    if (shownFile) { editorOwnPage(shownFile, underTab()); return; }
    if (!shownTerm) return;
    showIn(shownTerm.id, { kind: "page" });
    closeSideTabs([current]);
    go({ page: "terminal", id: shownTerm.id });
  };
  const pickBelow = (el: HTMLElement | null) => { const r = el?.getBoundingClientRect(); setPicker({ x: r?.left ?? 200, y: (r?.bottom ?? 100) + 6 }); };

  return (
    <div ref={islandRef} className="chat-side"
      onKeyDown={(e) => {
        // Ctrl+PgDn and Ctrl+PgUp: the next and previous tab, of any kind (a terminal lets them through)
        if (e.ctrlKey && !e.shiftKey && !e.altKey && !e.metaKey && (e.key === "PageDown" || e.key === "PageUp") && keys.length > 1) {
          e.preventDefault();
          const next = stepTab(keys, current, e.key === "PageDown" ? 1 : -1);
          if (next) showSideTab(next);
        }
        // Ctrl+W (Cmd+W on macOS) closes the shown tab but a terminal (the shell's there): a Card or the Board tab, not from a
        // text box (a comment being written); an Editor tab also from its code, asking first when it has changes not saved
        if ((e.ctrlKey || e.metaKey) && !e.shiftKey && !e.altKey && e.key.toLowerCase() === "w" && !shownTerm
          && !(e.target instanceof Element && e.target.closest(".xterm"))) {
          if (shownFile) { e.preventDefault(); closeEditorTabs([shownFile], underTab()); }
          else if ((shownCard || shownBoard) && !(e.target instanceof Element && e.target.closest("input, textarea, [contenteditable], .cm-editor"))) {
            e.preventDefault();
            closeSideTabs([current]);
          }
        }
      }}>
      <Handle panel={islandRef} share={share} setShare={setShare} edge={SIDE_EDGE} />
      <div className="chat-side-in">
        <div className="island-head">
          <div className="island-tabs" role="tablist" aria-label="Beside the chat">
            {side.tabs.map((t) => {
              const key = tabKey(t);
              if (t.kind === "terminal") {
                const s = terms.find((x) => x.id === t.id);
                return s ? (
                  <TermTab key={key} s={s} on={key === current} ids={termIds} renaming={renaming === s.id}
                    pick={(id) => showSideTab(tabKey({ kind: "terminal", id }))} close={close}
                    rename={(on) => setRenaming(on ? s.id : null)} fail={setFailed} />
                ) : null;
              }
              if (t.kind === "card") return <CardTab key={key} id={t.id} on={key === current} keys={keys} close={() => closeSideTabs([key])} />;
              if (t.kind === "board") return <BoardTab key={key} on={key === current} keys={keys} close={() => closeSideTabs([key])} />;
              const n = fileNames.get(t.id);
              return <EditorTab key={key} id={t.id} on={key === current} keys={keys} name={n?.name ?? nameOf(editorTabOf(t.id).path)} dir={n?.dir ?? null} />;
            })}
          </div>
          <span className="acts">
            <button ref={plusRef} type="button" className="btn ghost sm icon-only" aria-label="Open beside the chat" title="Open beside the chat"
              aria-haspopup="menu" aria-expanded={!!menu} onMouseDown={(e) => { if (menu) e.stopPropagation(); }}
              onClick={(e) => { const r = e.currentTarget.getBoundingClientRect(); setMenu(menu ? null : { x: r.left, y: r.bottom + 4 }); }}>
              <Plus className="icon" />
            </button>
            <button type="button" className="btn ghost sm icon-only" aria-label="Own page" title={`Open ${name} on its own page`} disabled={!shownTerm && !shownCard && !shownFile && !shownBoard} onClick={ownPage}>
              <Maximize2 className="icon" />
            </button>
            <button type="button" className="btn ghost sm icon-only" aria-label="Hide" title="Hide (Ctrl+`): terminals keep running" onClick={hide}>
              <ChevronRight className="icon" />
            </button>
          </span>
        </div>
        {note && (
          <div className="ed-bar file failed" role="alert">
            <CircleAlert className="icon" />
            <span className="say">{note}</span>
            <button type="button" className="btn ghost sm icon-only" aria-label="Close" title="Close" onClick={() => setFailed(undefined)}><X className="icon" /></button>
          </div>
        )}
        {shownCard ? <ChatCardBody key={shownCard} identifier={shownCard} />
          : shownFile ? <ChatEditorBody key={shownFile} id={shownFile} focus={focus} focusNext={focusNext} />
          : shownBoard ? <ChatBoardBody />
          : shownTerm ? <ChatTerminalBody s={shownTerm} focusNext={focusNext} /> : <div className="chat-side-body" />}
      </div>
      {menu && <ContextMenu at={menu} label="Open beside the chat" onClose={() => setMenu(null)}
        items={openItems(fresh.s, fresh.off, () => pickBelow(plusRef.current), () => under(plusRef.current))} />}
      {picker && <OpenTerminalPicker at={picker} you={you} onClose={() => setPicker(null)} />}
      {asking && createPortal(
        <AskClose at={asking} label={`Close ${asking.s.name}`} text={`${asking.s.busy} is still running in ${asking.s.name}. Close it and end ${asking.s.busy}?`}
          cancel={() => setAsking(null)} close={() => { const t = asking.s; setAsking(null); end(t); }} />,
        document.body,
      )}
    </div>
  );
}

/** The tool strip: one button per kind open in the chat, in the order Terminal, Editor, Board, Card, with its count; the
 *  shown kind's selected (aria-pressed). ↑ and ↓ move between them. */
function ToolStrip({ side, all, toggle }: { side: ChatSide; all: TerminalSession[]; toggle: (kind: SideKind) => void }) {
  const kinds = kindsOpen(side);
  const on = shownKind(side);
  const [menu, setMenu] = useState<{ kind: SideKind; count: number; at: { x: number; y: number }; under: { x: number; y: number } } | null>(null);
  const [asking, setAsking] = useState<{ text: string; at: { x: number; y: number }; run: () => void } | null>(null);
  const [failed, setFail] = useState<string>();

  const openMenu = (kind: SideKind, count: number, btn: HTMLElement, at: { x: number; y: number } | null) => {
    const r = btn.getBoundingClientRect();
    setMenu({ kind, count, at: at ?? { x: r.left - 222, y: r.top }, under: { x: r.left, y: r.bottom + 4 } });
  };
  /** Close all of a kind: for terminals, today's question first while one runs a program; for the Editor, its question
   *  first while a file has changes not saved. */
  const closeAll = (kind: SideKind, under: { x: number; y: number }) => {
    const keys = side.tabs.filter((t) => t.kind === kind).map(tabKey);
    if (kind === "editor") { closeEditorTabs(keys.map(idOf), under); return; }
    if (kind !== "terminal") { closeSideTabs(keys); return; }
    const mine = side.tabs.filter((t) => t.kind === "terminal").flatMap((t) => all.filter((s) => s.id === t.id));
    const endAll = () => {
      setFail(undefined);
      for (const s of mine) terminalClose(s.id).catch((e) => setFail(`Couldn't close ${s.name}: ${why(e)}`));
    };
    const busy = mine.filter(asksToClose);
    if (!busy.length) { endAll(); return; }
    setAsking({ text: closeAllQuestion(busy.map((s) => ({ name: s.name, busy: s.busy ?? "" }))), at: under, run: endAll });
  };
  const items = (m: NonNullable<typeof menu>): MenuItem[] => [
    { head: stripMenuHead(m.kind, m.count) },
    on === m.kind ? { label: "Hide", run: () => toggle(m.kind) } : { label: "Show", kbd: kindKey(m.kind), run: () => toggle(m.kind) },
    { sep: true },
    { label: "Close all", danger: true, run: () => closeAll(m.kind, m.under) },
  ];

  return (
    <div className="chat-strip" role="toolbar" aria-label="Beside the chat" aria-orientation="vertical"
      onKeyDown={(e) => {
        if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
        const btns = [...e.currentTarget.querySelectorAll<HTMLButtonElement>(".strip-btn")];
        const i = btns.indexOf(document.activeElement as HTMLButtonElement);
        if (i < 0) return;
        e.preventDefault();
        btns[(i + (e.key === "ArrowDown" ? 1 : -1) + btns.length) % btns.length]?.focus();
      }}>
      {kinds.map(({ kind, count }) => {
        const Icon = KIND_ICON[kind];
        return (
          <button key={kind} type="button" className="strip-btn" data-kind={kind} aria-pressed={on === kind} aria-label={stripLabel(kind, count)}
            title={stripTitle(kind, count)} onClick={() => toggle(kind)}
            onContextMenu={(e) => { e.preventDefault(); openMenu(kind, count, e.currentTarget, e.clientX || e.clientY ? { x: e.clientX, y: e.clientY } : null); }}
            onKeyDown={(e) => {
              if (e.key === "ContextMenu" || (e.shiftKey && e.key === "F10")) { e.preventDefault(); openMenu(kind, count, e.currentTarget, null); }
            }}>
            <Icon className="icon" />
            <span className="n">{count}</span>
          </button>
        );
      })}
      {menu && <ContextMenu at={menu.at} label={stripMenuHead(menu.kind, menu.count)} items={items(menu)} onClose={() => setMenu(null)} />}
      {asking && createPortal(
        <AskClose at={asking.at} label="Close all terminals" text={asking.text} cancel={() => setAsking(null)} close={() => { const run = asking.run; setAsking(null); run(); }} />,
        document.body,
      )}
      {failed && <div className="toast" role="alert"><CircleAlert className="icon" /><span>{failed}</span>
        <button type="button" className="btn ghost sm icon-only" aria-label="Close" title="Close" onClick={() => setFail(undefined)}><X className="icon" /></button></div>}
    </div>
  );
}

/** Closing while a program runs (the Editor panel's CloseAsk, with its words): Cancel, Close; Close has the focus. */
function AskClose({ at, label, text, close, cancel }: { at: { x: number; y: number }; label: string; text: string; close: () => void; cancel: () => void }) {
  const ref = useRef<HTMLDivElement>(null);
  const cancelRef = useRef(cancel);
  cancelRef.current = cancel;
  useEffect(() => {
    (ref.current?.querySelector(".btn.danger") as HTMLButtonElement | null)?.focus();
    const down = (e: MouseEvent) => { if (!ref.current?.contains(e.target as Node)) cancelRef.current(); };
    window.addEventListener("mousedown", down);
    return () => window.removeEventListener("mousedown", down);
  }, []);
  return (
    <div ref={ref} className="pop ed-ask" role="alertdialog" aria-label={label}
      style={{ position: "fixed", left: Math.max(8, Math.min(at.x, window.innerWidth - 392)), top: at.y }}
      onKeyDown={(e) => {
        if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); cancel(); }
        else if (e.key === "Enter") { e.preventDefault(); e.stopPropagation(); close(); }
      }}>
      <p>{text}</p>
      <div className="acts">
        <button type="button" className="btn ghost sm" onClick={cancel}>Cancel</button>
        <button type="button" className="btn danger sm" onClick={close}>Close</button>
      </div>
    </div>
  );
}

// ---------- the top bar's Open ----------

/** Open, right before New chat: a menu with New terminal (in the shown terminal's folder, else the folder picker titled
 *  "Open terminal in" under the button) and New terminal in…. `youId`: for the picker. */
export function OpenButton({ youId }: { youId: string }) {
  const { side } = useChatSide();
  const fresh = useNewTerminal(side);
  const ref = useRef<HTMLButtonElement>(null);
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const [picker, setPicker] = useState<{ x: number; y: number } | null>(null);
  const pick = () => { const r = ref.current?.getBoundingClientRect(); setPicker({ x: r?.left ?? 200, y: (r?.bottom ?? 50) + 6 }); };
  useEffect(() => {
    pickUnderOpen = pick;
    return () => { if (pickUnderOpen === pick) pickUnderOpen = null; };
  });
  return (
    <>
      <button ref={ref} type="button" className="btn sm chat-open" aria-haspopup="menu" aria-expanded={!!menu} onMouseDown={(e) => { if (menu) e.stopPropagation(); }}
        title="Open a terminal, a file or the board beside the chat"
        onClick={(e) => { const r = e.currentTarget.getBoundingClientRect(); setMenu(menu ? null : { x: r.left, y: r.bottom + 6 }); }}>
        <PanelRight className="icon" />Open<ChevronDown className="icon" />
      </button>
      {menu && <ContextMenu at={menu} label="Open beside the chat" onClose={() => setMenu(null)} items={openItems(fresh.s, fresh.off, pick, () => under(ref.current))} />}
      {picker && <OpenTerminalPicker at={picker} you={youId} onClose={() => setPicker(null)} />}
    </>
  );
}
