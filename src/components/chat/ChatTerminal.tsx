// The Chat page's terminal panel (GA-278): Open terminal in the top bar, right before New chat; the panel on the right of
// the conversation (and of the Archive) while it shows; a rail on the chat's right edge while it is hidden with terminals.
// - The panel draws and works as the Editor's (components/editor/TerminalPanel.tsx), with its tabs (TermTab), its close
//   question (CloseAsk) and its drag handle (Handle), here the panel's left edge: the chat's terminals as tabs, + for
//   another one in the shown terminal's folder, Own page and Hide, that folder in mono, and the terminal, in the place
//   { kind: "chat" }.
// - Its state is src/lib/chatTerminal.ts's, kept while Hersir runs: another chat, New chat, the Archive, or another page and
//   back find the panel, its tabs and the shown terminal as they were. Its width is kept on this computer.
// - Keys on the Chat page: Ctrl+` shows or hides the panel while the chat has terminals, Ctrl+Shift+` starts a terminal in
//   the shown terminal's folder (also from inside the terminal, which lets them through); in the panel, Ctrl+PgDn and
//   Ctrl+PgUp switch tabs.
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { ChevronRight, CircleAlert, Maximize2, Plus, Terminal as TerminalIcon, X } from "lucide-react";
import { editorFolders, terminalClose } from "../../api";
import type { EditorFolder, TerminalSession } from "../../types";
import { go } from "../../router";
import { stepTab } from "../../lib/editor";
import { asksToClose, focusTerminal, folderNameOf, manyNote, showIn, useTerminals, type TerminalPlace } from "../../lib/terminals";
import { panelKey } from "../../lib/terminalPanel";
import { onWindowFocus } from "../../lib/windowFocus";
import {
  CHAT_FIRST_SHARE, CHAT_SHARE_STEP, chatListWidth, chatShareFor, clampChatPanel, hideChatPanel, keepChatShare, listFits, loadChatShare,
  openChatTerminal, pickChatTerminal, showChatPanel, useChatTerminals,
} from "../../lib/chatTerminal";
import { FolderPicker } from "../editor/FolderPicker";
import { CloseAsk, Handle, TermTab, type HandleEdge } from "../editor/TerminalPanel";
import { TerminalView } from "../terminal/TerminalView";

const CHAT: TerminalPlace = { kind: "chat" };
const why = (e: unknown) => String(e).replace(/^Error:\s*/, "");

/** The panel's left edge: its width is a share of the space right of the chat list (the conversation and the panel), at
 *  least 360px of panel and 400px of conversation. */
const CHAT_EDGE: HandleEdge = {
  side: "left", label: "Terminal panel's width", clamp: clampChatPanel, share: chatShareFor, first: CHAT_FIRST_SHARE, step: CHAT_SHARE_STEP,
  keep: keepChatShare,
  measure: (el) => {
    const talk = el.parentElement?.querySelector<HTMLElement>(":scope > .chat-main");
    if (!talk) return null;
    const size = el.getBoundingClientRect().width;
    const other = talk.getBoundingClientRect().width;
    return { size, other, whole: size + other };
  },
};

export type ChatTerminalState = ReturnType<typeof useChatTerminal>;

/** The panel's state on the Chat page (`on`: the Chief is set up, so the chat shows): the chat's terminals, the one shown,
 *  whether the panel or the rail shows, the panel's width and its actions, and .chat's class and style for them. A terminal
 *  shown when the page opens doesn't take the focus; one opened, picked or shown on the page does. */
export function useChatTerminal(on: boolean) {
  const t = useChatTerminals();
  const all = useTerminals();
  const tabs = useMemo(() => t.ids.flatMap((id) => all.filter((s) => s.id === id)), [all, t.ids]);
  const current = tabs.find((s) => s.id === t.current) ?? null;
  const visible = on && t.open && !!current;
  const since = useRef(t.focus);
  const focusNext = t.focus > since.current;
  const [share, setShare] = useState(loadChatShare);
  const [failed, setFailed] = useState<string>();
  const [starting, setStarting] = useState(false);
  const [chat, chatRef] = useState<HTMLDivElement | null>();
  const panelRef = useRef<HTMLDivElement>(null);
  const off = useStartOff(current?.folder ?? null);
  const fits = useListFits(chat ?? null, visible);

  // the terminal opened or picked again while its view is in the panel takes the focus now (a view that comes in takes it
  // once it is in: focusNext)
  const asked = useRef(t.focus);
  useEffect(() => {
    if (t.focus === asked.current) return;
    asked.current = t.focus;
    if (visible && t.current) focusTerminal(t.current);
  }, [t.focus]); // eslint-disable-line react-hooks/exhaustive-deps

  // why + couldn't start one, or a tab couldn't close, goes once another terminal shows
  useEffect(() => { setFailed(undefined); }, [t.current]);

  /** + and Ctrl+Shift+`: a new terminal in the shown terminal's folder; why it couldn't start shows in the panel. */
  const start = () => {
    if (!current || off || starting) return;
    setStarting(true);
    setFailed(undefined);
    openChatTerminal(current.folder, false).then((w) => {
      setStarting(false);
      if (w) { setFailed(w); showChatPanel(); }
    });
  };
  /** Hide and Ctrl+`: the shells keep running. With the focus in the panel, it goes to the text box (the Archive's search). */
  const hide = () => {
    const inside = !!panelRef.current?.contains(document.activeElement);
    hideChatPanel();
    if (inside) requestAnimationFrame(() => document.querySelector<HTMLElement>(".chat-main .composer-editor .cm-content, .chat-main .archive-search")?.focus());
  };
  const fail = (message: string) => setFailed(message);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const k = panelKey(e);
      if (!k || !on || !current || e.defaultPrevented) return;
      e.preventDefault();
      if (k === "toggle") { if (visible) hide(); else showChatPanel(); }
      // while none can start there, the panel with the terminals it has
      else if (off) showChatPanel();
      else start();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  const className = `chat${visible ? ` has-term${fits ? "" : " no-list"}` : on && tabs.length > 0 ? " has-rail" : ""}`;
  const style = visible ? ({ "--chat-term-share": share / 100 } as React.CSSProperties) : undefined;
  return { tabs, current, visible, focusNext, share, setShare, failed, setFailed, starting, off, chatRef, panelRef, className, style, start, hide, fail };
}

/** Why a terminal can't start in the folder now (an agent works on the card's worktree; the backend refuses it too), or
 *  null; asked again when the window gets the focus, as the Terminal page asks. */
function useStartOff(folder: string | null): string | null {
  const [found, setFound] = useState<EditorFolder>();
  useEffect(() => {
    if (!folder) return;
    let alive = true;
    const load = () => { editorFolders().then((fs) => { if (alive) setFound(fs.find((f) => f.id === folder)); }).catch(() => {}); };
    load();
    const stop = onWindowFocus(load);
    return () => { alive = false; stop(); };
  }, [folder]);
  return found && found.id === folder && found.readOnly ? `${found.readOnlyWhy ?? "An agent is working on it"}: a terminal there waits until its run ends.` : null;
}

/** Whether the chat list has room beside the panel: measured while the panel shows, and again when the chat changes size. */
function useListFits(chat: HTMLDivElement | null, on: boolean): boolean {
  const [fits, setFits] = useState(true);
  useLayoutEffect(() => {
    if (!on || !chat) return;
    const check = () => {
      const grow = parseFloat(getComputedStyle(document.documentElement).getPropertyValue("--ui-grow")) || 0;
      setFits(listFits(chat.getBoundingClientRect().width, chatListWidth(window.matchMedia("(max-width: 1100px)").matches, grow)));
    };
    check();
    const ro = new ResizeObserver(check);
    ro.observe(chat);
    return () => ro.disconnect();
  }, [chat, on]);
  return fits;
}

/** Open terminal, right before New chat: the folder picker titled "Open terminal in" under it; a pick starts a terminal in
 *  that folder, shown in the panel, and one that can't start says why under the folder's row, with the picker still open. */
export function OpenTerminalButton({ youId }: { youId: string }) {
  const [at, setAt] = useState<{ x: number; y: number }>();
  return (
    <>
      <button className="btn sm" aria-haspopup="dialog" aria-expanded={!!at}
        onClick={(e) => { const r = e.currentTarget.getBoundingClientRect(); setAt({ x: r.left, y: r.bottom + 6 }); }}>
        <TerminalIcon className="icon" />Open terminal
      </button>
      {at && <OpenTerminalPicker at={at} you={youId} onClose={() => setAt(undefined)} />}
    </>
  );
}

/** The picker, with the note from 20 open terminals as New terminal's (TerminalParts' NewTerminalPicker). */
function OpenTerminalPicker({ at, you, onClose }: { at: { x: number; y: number }; you: string; onClose: () => void }) {
  const open = useTerminals().length;
  return (
    <FolderPicker at={at} title="Open terminal in" you={you} note={manyNote(open)} onClose={onClose}
      onPick={(f: EditorFolder) => openChatTerminal(f.id, false)} />
  );
}

/** The panel on the right of the conversation: its left edge is the drag handle; the chat's terminals as tabs with +, Own
 *  page and Hide; the shown terminal's folder; and the terminal, with the focus when it was just opened or picked. */
export function ChatTerminalPanel({ p }: { p: ChatTerminalState }) {
  const [asking, setAsking] = useState<{ s: TerminalSession; x: number; y: number } | null>(null);
  const [renaming, setRenaming] = useState<string | null>(null);
  const s = p.current;
  const ids = p.tabs.map((x) => x.id);
  const name = s ? folderNameOf(s) : "";
  const end = (t: TerminalSession) => { terminalClose(t.id).catch((e) => p.fail(`Couldn't close ${t.name}: ${why(e)}`)); };
  const close = (t: TerminalSession, el: HTMLElement | null) => {
    if (!asksToClose(t)) { end(t); return; }
    const r = el?.getBoundingClientRect();
    setAsking({ s: t, x: r?.left ?? 200, y: (r?.bottom ?? 100) + 4 });
  };
  const ownPage = () => {
    if (!s) return;
    showIn(s.id, { kind: "page" });
    go({ page: "terminal", id: s.id });
  };
  return (
    <div ref={p.panelRef} className="chat-term" role="region" aria-label="Terminal"
      onKeyDown={(e) => {
        // Ctrl+PgDn and Ctrl+PgUp: the next and previous terminal tab (the terminal lets them through)
        if (e.ctrlKey && !e.shiftKey && !e.altKey && !e.metaKey && (e.key === "PageDown" || e.key === "PageUp") && ids.length > 1) {
          e.preventDefault();
          const next = stepTab(ids, s?.id ?? null, e.key === "PageDown" ? 1 : -1);
          if (next) pickChatTerminal(next);
        }
      }}>
      <Handle panel={p.panelRef} share={p.share} setShare={p.setShare} edge={CHAT_EDGE} />
      <div className="chat-term-in">
        <div className="ed-tabs term-tabs" role="tablist" aria-label="Terminals in the chat">
          {p.tabs.map((t) => (
            <TermTab key={t.id} s={t} on={t.id === s?.id} ids={ids} renaming={renaming === t.id}
              pick={pickChatTerminal} close={close} rename={(on) => setRenaming(on ? t.id : null)} fail={p.fail} />
          ))}
          <button className={`btn ghost sm icon-only${p.off ? " term-btn" : ""}`} aria-label={`New terminal in ${name}`}
            title={p.off ?? `New terminal in ${name} (Ctrl+Shift+\`)`} aria-disabled={!!p.off || p.starting || undefined}
            onClick={() => { if (!p.off) p.start(); }}>
            <Plus className="icon" />
          </button>
          <span className="acts">
            <button className="btn ghost sm" title={`Open ${s?.name ?? name} on its own page`} disabled={!s} onClick={ownPage}><Maximize2 className="icon" />Own page</button>
            <button className="btn ghost sm" title="Hide the terminal (Ctrl+`): its shells keep running" onClick={p.hide}><ChevronRight className="icon" />Hide</button>
          </span>
        </div>
        {s && <div className="chat-term-where" title={s.folderPath}>{s.folderPath}</div>}
        {p.failed && (
          <div className="ed-bar file failed" role="alert">
            <CircleAlert className="icon" />
            <span className="say">{p.failed}</span>
            <button className="btn ghost sm icon-only" aria-label="Close" title="Close" onClick={() => p.setFailed(undefined)}><X className="icon" /></button>
          </div>
        )}
        {s ? <TerminalView key={s.id} id={s.id} place={CHAT} autoFocus={p.focusNext} /> : <div className="term-place" />}
      </div>
      {asking && createPortal(
        <CloseAsk at={asking} s={asking.s} cancel={() => setAsking(null)} close={() => { const t = asking.s; setAsking(null); end(t); }} />,
        document.body,
      )}
    </div>
  );
}

/** The panel hidden while the chat has terminals: a narrow rail on the chat's right edge, the terminal icon and how many
 *  there are; a click shows the panel. */
export function ChatTerminalRail({ p }: { p: ChatTerminalState }) {
  const n = p.tabs.length;
  return (
    <button className="chat-term-rail" title="Show the terminal (Ctrl+`)" aria-label={`Show the terminal (${n === 1 ? "1 terminal" : `${n} terminals`})`}
      onClick={showChatPanel}>
      <TerminalIcon className="icon" />
      <span className="n">{n}</span>
    </button>
  );
}
