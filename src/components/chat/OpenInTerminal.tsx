// Open in terminal under a Chief answer that asks you to run commands (GA-279; src/lib/runCommands.ts finds them): a quiet
// row right under the message, with a button for each folder it names that is one of the Editor's (at most 3), which shows
// a terminal in that folder in the chat's terminal panel (GA-278), or Open in terminal… with the folder picker when it names
// none. It never types or runs anything: the terminal starts in the folder and waits for you.
// The row is there from the message's first draw and its buttons once the Editor's folders are known, so the chat doesn't
// jump. The folders are asked once for all the messages: when the chat shows them, when the window gets the focus, and when
// a message names a path that isn't in one of them (not more than once in FOCUS_GAP).
import { memo, useEffect, useMemo, useState, useSyncExternalStore } from "react";
import { Terminal as TerminalIcon } from "lucide-react";
import { editorFolders } from "../../api";
import type { EditorFolder } from "../../types";
import { openChatTerminal } from "../../lib/chatTerminal";
import { namedPaths, runCommands, terminalFolders } from "../../lib/runCommands";
import { manyNote, useTerminals } from "../../lib/terminals";
import { FOCUS_GAP, onWindowFocus } from "../../lib/windowFocus";
import { FolderPicker } from "../editor/FolderPicker";

// ---------- the Editor's folders, for every message's row ----------

let known: EditorFolder[] | undefined;
let askedAt = -Infinity;
let asking = false;
const subs = new Set<() => void>();
let stopFocus: (() => void) | undefined;

const sameFolders = (a: EditorFolder[], b: EditorFolder[]) =>
  a.length === b.length && a.every((f, i) => f.id === b[i]!.id && f.path === b[i]!.path && f.kind === b[i]!.kind);

/** Asks for the Editor's folders, unless it did FOCUS_GAP or less ago; the rows draw again only when they changed. One that
 *  can't be asked counts as none: the rows offer the picker, which says why. */
function loadFolders() {
  const now = Date.now();
  if (asking || (known && now - askedAt <= FOCUS_GAP && now >= askedAt)) return;
  asking = true;
  askedAt = now;
  const done = (next: EditorFolder[]) => {
    asking = false;
    if (known && sameFolders(known, next)) return;
    known = next;
    subs.forEach((f) => f());
  };
  Promise.resolve().then(editorFolders).then(done, () => done(known ?? []));
}

const subscribe = (f: () => void) => {
  subs.add(f);
  if (subs.size === 1) {
    stopFocus = onWindowFocus(loadFolders);
    loadFolders();
  }
  return () => {
    subs.delete(f);
    if (!subs.size) { stopFocus?.(); stopFocus = undefined; }
  };
};
/** A message that names no path needs no folders. */
const idle = () => () => {};
const knownNow = () => known;

// ---------- the row ----------

/** Under a Chief text message: Open in terminal when it asks you to run at least one command, else nothing. `you`: for the
 *  folder picker, where a card's worktree that isn't pinned to you asks first. */
export const OpenInTerminal = memo(function OpenInTerminal({ md, you }: { md: string; you?: string }) {
  const asks = useMemo(() => runCommands(md).length > 0, [md]);
  return asks ? <OpenInTerminalRow md={md} you={you} /> : null;
});

/** The buttons, one per folder the message names, in the order named; Open in terminal… when it names none of the Editor's
 *  folders. Why a terminal couldn't start shows under them. */
function OpenInTerminalRow({ md, you }: { md: string; you?: string }) {
  const paths = useMemo(() => namedPaths(md).length > 0, [md]);
  const folders = useSyncExternalStore(paths ? subscribe : idle, knownNow, knownNow);
  const named = useMemo(() => (!paths ? [] : folders ? terminalFolders(md, folders) : undefined), [md, paths, folders]);
  const [why, setWhy] = useState<string>();
  const [starting, setStarting] = useState<string>();
  const [at, setAt] = useState<{ x: number; y: number }>();

  // a path in none of the folders may be in one that is new since they were asked
  useEffect(() => {
    if (paths && named?.length === 0) loadFolders();
  }, [paths, named]);

  /** The terminal in that folder that waits for a command shows, or a new one starts there; it takes the focus. */
  const open = (f: EditorFolder) => {
    if (starting) return;
    setStarting(f.id);
    setWhy(undefined);
    openChatTerminal(f.id, true).then((w) => { setStarting(undefined); setWhy(w ?? undefined); });
  };

  return (
    <div className="msg-term">
      <div className="msg-term-row">
        {!named ? (
          // until the folders are known: a button's room, so the row doesn't move the chat when they come
          <span className="btn ghost sm msg-term-wait" aria-hidden><TerminalIcon className="icon" />Open in terminal</span>
        ) : named.length > 0 ? named.map((f) => (
          <button key={f.id} type="button" className="btn ghost sm" title={`Open a terminal in ${f.path} beside the chat`}
            aria-busy={starting === f.id || undefined} onClick={() => open(f)}>
            <TerminalIcon className="icon" />Open in terminal<span className="mono">{f.path}</span>
          </button>
        )) : (
          <button type="button" className="btn ghost sm" aria-haspopup="dialog" aria-expanded={!!at} title="Open a terminal beside the chat, in a folder you pick"
            onClick={(e) => { const r = e.currentTarget.getBoundingClientRect(); setWhy(undefined); setAt({ x: r.left, y: r.bottom + 6 }); }}>
            <TerminalIcon className="icon" />Open in terminal…
          </button>
        )}
      </div>
      {why && <div className="msg-term-why" role="alert">{why}</div>}
      {at && <PickFolder at={at} you={you} onClose={() => setAt(undefined)} />}
    </div>
  );
}

/** The folder picker titled "Open terminal in", as Open terminal's next to New chat, with the note from 20 open terminals.
 *  A pick shows the terminal in that folder that waits for a command, or starts one; one that can't start says why under
 *  the folder's row, with the picker still open. */
function PickFolder({ at, you, onClose }: { at: { x: number; y: number }; you?: string; onClose: () => void }) {
  const open = useTerminals().length;
  return (
    <FolderPicker at={at} title="Open terminal in" you={you} note={manyNote(open)} onClose={onClose}
      onPick={(f: EditorFolder) => openChatTerminal(f.id, true)} />
  );
}
