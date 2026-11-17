// The Editor kind of the Chat's side island (GA-298; design/islands-chat → Editor, Figma frame 4): one file at a line,
// beside the chat. Its tab (file-code and the file's name; two with the same name get their folder, as the Editor's tabs
// do; the tooltip its path), and under the island's header:
// - a row: the path from its folder in mono, and Save (disabled until there are changes; Ctrl+S, Cmd+S on macOS, with the
//   focus in the tab);
// - the code in the Editor's CodeMirror view and colours, with the line it was opened at marked (a band of accent-soft and
//   a 2px accent bar, its number in text) until another line is opened there: the view scrolls it to a third from the top
//   and puts the caret at its start;
// - the status bar: Ln 4, Col 15, the language, UTF-8, the branch.
// What is open stays while Hersir runs, also while another tab shows or another page does: each file's text, its changes,
// its undo history and its cursor (kept here by the tab's id, chatSide.editorTabId). As in the Editor, a file that changed
// on disk reloads when it has no changes here, and offers Reload or Save over it when it has; closing a tab with changes
// that aren't saved asks first (Save and close, Close without saving, Cancel): its ×, middle click, Ctrl+W, Own page and
// the strip's Close all.
import { useEffect, useLayoutEffect, useRef, useState, useSyncExternalStore } from "react";
import { createPortal } from "react-dom";
import { FileCode, Lock, TriangleAlert, X } from "lucide-react";
import { StateEffect, type EditorState, type Text } from "@codemirror/state";
import { EditorView, type ViewUpdate } from "@codemirror/view";
import { editorGit, editorRead, editorVersions, editorWrite } from "../../api";
import type { OpenedFile } from "../../types";
import { go } from "../../router";
import { codeState, goToMarkedLine, markLine, markedLine, replaceText, setLanguage, type CodeHandlers } from "../editor/code";
import { cursorLabel, editorPageAt, nameOf, stepTab } from "../../lib/editor";
import { languageName } from "../../lib/codeLanguages";
import { modKey } from "../../lib/keys";
import { closeSideTabs, editorTabId, editorTabOf, openSideTab, showSideTab, tabKey, tabOpenAnywhere } from "../../lib/chatSide";
import { FOCUS_GAP, onWindowFocus } from "../../lib/windowFocus";
import { useEditorFolders } from "./OpenInTerminal";

// ---------- the open files, kept while Hersir runs ----------

type EdFile = {
  folder: string;
  path: string;
  status: "loading" | "ready" | "error";
  error: string | null;
  file: OpenedFile | null;
  /** The version read or saved, which a save checks. */
  version: string;
  dirty: boolean;
  saving: boolean;
  saveError: string | null;
  /** Changed on disk while it has changes here, or gone from disk. */
  disk: null | "changed" | "gone";
  /** The marked line (from 1), and how many times a line was asked: the view goes there again each time. */
  line: number | null;
  asked: number;
  /** Opened read-only: a file in a card's worktree, or in a folder an agent works in (Open a file…). */
  lock: boolean;
};

const files = new Map<string, EdFile>();
const states = new Map<string, EditorState>();
const saved = new Map<string, Text>();
/** What the code tells the tab showing it: its cursor. */
const cursorFns = new Map<string, (s: EditorState) => void>();
/** The view that shows the file, while its tab shows. */
const views = new Map<string, EditorView>();
/** The last line request each file's view went to. */
const wentTo = new Map<string, number>();
const subs = new Set<() => void>();
const publish = () => subs.forEach((f) => f());
const subscribe = (f: () => void) => { subs.add(f); return () => { subs.delete(f); }; };
const why = (e: unknown) => String(e).replace(/^Error:\s*/, "");

function patch(id: string, p: Partial<EdFile>) {
  const f = files.get(id);
  if (!f) return;
  files.set(id, { ...f, ...p });
  publish();
}
/** The kept file of an Editor tab (undefined before it opened). */
export const useEdFile = (id: string) => useSyncExternalStore(subscribe, () => files.get(id), () => files.get(id));
/** Whether an Editor tab has changes that aren't saved. */
export const editorDirty = (id: string) => !!files.get(id)?.dirty;

function onUpdate(id: string, u: ViewUpdate) {
  states.set(id, u.state);
  if (u.docChanged) {
    const base = saved.get(id);
    const dirty = !!base && !u.state.doc.eq(base);
    if (files.get(id)?.dirty !== dirty) patch(id, { dirty });
  }
  if (u.docChanged || u.selectionSet) cursorFns.get(id)?.(u.state);
}

function read(id: string) {
  const { folder, path } = editorTabOf(id);
  editorRead(folder, path).then((file) => {
    if (!files.has(id)) return;
    if (file.text !== null) {
      const on: { current: CodeHandlers } = { current: { save: () => { void saveFile(id); }, update: (u) => onUpdate(id, u) } };
      const s = codeState(file.text, file.readOnly || !!files.get(id)?.lock, on);
      const withMark = s.update({ effects: StateEffect.appendConfig.of(markedLine) }).state;
      states.set(id, withMark);
      saved.set(id, withMark.doc);
    }
    patch(id, { status: "ready", file, version: file.version, error: null, dirty: false, disk: null });
  }).catch((e) => patch(id, { status: "error", error: why(e) }));
}

/** Opens the file (`path` in the Editor folder `folder`) in an Editor tab of the chat on show, or shows its tab; with a line,
 *  that line is marked and the view goes to it. `lock`: read-only (a card's worktree). */
export function openEditorTab(folder: string, path: string, line: number | null, lock = false) {
  const id = editorTabId(folder, path);
  const f = files.get(id);
  if (!f) {
    files.set(id, { folder, path, status: "loading", error: null, file: null, version: "", dirty: false, saving: false, saveError: null, disk: null,
      line, asked: line ? 1 : 0, lock });
    read(id);
    publish();
  } else if (line) patch(id, { line, asked: f.asked + 1 });
  openSideTab({ kind: "editor", id });
}

/** Saves the file: true when nothing is left unsaved. One that changed on disk meanwhile isn't saved over (its bar asks). */
export function saveFile(id: string, over = false): Promise<boolean> {
  const f = files.get(id);
  const s = states.get(id);
  if (!f || !s || f.saving || f.file?.readOnly || f.lock) return Promise.resolve(!f?.dirty);
  if (!f.dirty && !over && f.disk !== "gone") return Promise.resolve(true);
  if (f.disk === "changed" && !over) return Promise.resolve(false);
  const doc = s.doc;
  patch(id, { saving: true, saveError: null });
  const version = over ? editorVersions(f.folder, [f.path]).then(([v]) => v ?? "") : Promise.resolve(f.disk === "gone" ? "" : f.version);
  return version.then((v) => editorWrite(f.folder, f.path, doc.toString(), v)).then((v) => {
    saved.set(id, doc);
    const now = states.get(id)?.doc;
    patch(id, { saving: false, version: v, disk: null, saveError: null, dirty: !!now && !now.eq(doc) });
    return true;
  }, (e) => {
    const said = why(e);
    if (/changed on disk/.test(said)) patch(id, { saving: false, disk: /isn't there any more/.test(said) ? "gone" : "changed" });
    else patch(id, { saving: false, saveError: said });
    return false;
  });
}

/** Reads the file again over the text here (Reload, or by itself when it has no changes here); its marked line stays. */
function reload(id: string) {
  const f = files.get(id);
  if (!f) return;
  editorRead(f.folder, f.path).then((file) => {
    const s = states.get(id);
    if (file.text === null || !s) { patch(id, { file, version: file.version, disk: null, dirty: false }); return; }
    const tr = replaceText(s, file.text);
    const mark = f.line && f.line <= tr.state.doc.lines ? [markLine.of(tr.state.doc.line(f.line).from)] : [];
    const v = views.get(id);
    let next: EditorState;
    if (v && v.state === s) { v.dispatch(tr); v.dispatch({ effects: mark }); next = v.state; } else next = tr.state.update({ effects: mark }).state;
    states.set(id, next);
    saved.set(id, next.doc);
    patch(id, { file, version: file.version, disk: null, dirty: false, saveError: null });
  }).catch((e) => patch(id, { saveError: why(e) }));
}

/** Outside changes, while its tab shows: an untouched file reloads, one with changes here asks; one gone says so. */
function checkDisk(id: string) {
  const f = files.get(id);
  if (!f || f.status !== "ready" || f.saving || !f.file) return;
  editorVersions(f.folder, [f.path]).then(([onDisk]) => {
    const now = files.get(id);
    if (!now || now.saving || now.version !== f.version || onDisk === undefined) return;
    if (onDisk === "") { if (now.disk !== "gone") patch(id, { disk: "gone" }); return; }
    if (onDisk === now.version) { if (now.disk === "gone") patch(id, { disk: null }); return; }
    if (!now.dirty) reload(id);
    else if (now.disk !== "changed") patch(id, { disk: "changed" });
  }).catch(() => {});
}

/** Forgets a file whose tab closed (in every chat). */
function forget(id: string) {
  if (tabOpenAnywhere(tabKey({ kind: "editor", id }))) return;
  files.delete(id);
  states.delete(id);
  saved.delete(id);
  wentTo.delete(id);
  publish();
}

// ---------- closing, with the question when changes aren't saved ----------

type Ask = { ids: string[]; at: { x: number; y: number }; then?: () => void };
let ask: Ask | null = null;
const setAsk = (a: Ask | null) => { ask = a; publish(); };
const useAsk = () => useSyncExternalStore(subscribe, () => ask, () => ask);

function closeNow(ids: string[], then?: () => void) {
  closeSideTabs(ids.map((id) => tabKey({ kind: "editor", id })));
  ids.forEach(forget);
  then?.();
}
/** Closes Editor tabs of the chat on show; with changes that aren't saved it asks first, under `at`. `then`: after they
 *  closed (Own page goes to the Editor page). */
export function closeEditorTabs(ids: string[], at: { x: number; y: number } | null, then?: () => void) {
  if (!ids.some(editorDirty)) { closeNow(ids, then); return; }
  setAsk({ ids, at: at ?? { x: Math.max(8, window.innerWidth - 420), y: 96 }, then });
}
/** The question's words: one file, or how many. */
export const unsavedQuestion = (names: string[]) =>
  names.length === 1 ? `${names[0]} has changes that aren't saved.` : `${names.length} files have changes that aren't saved.`;

/** Own page: the Editor page at the file and the caret's line; its tab closes here (asking first with changes not saved). */
export function editorOwnPage(id: string, at: { x: number; y: number } | null) {
  const { folder, path } = editorTabOf(id);
  const s = views.get(id)?.state ?? states.get(id);
  const line = s ? s.doc.lineAt(s.selection.main.head).number : files.get(id)?.line ?? null;
  closeEditorTabs([id], at, () => {
    editorPageAt(folder, path, line);
    go({ page: "editor", id: folder });
  });
}

/** The question when closing tabs with changes that aren't saved (the Editor's CloseAsk, with its words): Save and close,
 *  Close without saving, Cancel. Mounted once on the Chat page (ChatSide). */
export function EditorCloseAsk() {
  const a = useAsk();
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!a) return;
    (ref.current?.querySelector("button") as HTMLButtonElement | null)?.focus();
    const down = (e: MouseEvent) => { if (!ref.current?.contains(e.target as Node)) setAsk(null); };
    window.addEventListener("mousedown", down);
    return () => window.removeEventListener("mousedown", down);
  }, [a]);
  if (!a) return null;
  const dirty = a.ids.filter(editorDirty);
  const names = dirty.map((id) => nameOf(editorTabOf(id).path));
  const saveAll = () => {
    setAsk(null);
    Promise.all(dirty.map((id) => saveFile(id))).then((ok) => { if (ok.every(Boolean)) closeNow(a.ids, a.then); });
  };
  return createPortal(
    <div ref={ref} className="pop ed-ask" role="alertdialog" aria-label={names.length === 1 ? `Close ${names[0]}` : "Close the files"}
      style={{ position: "fixed", left: Math.max(8, Math.min(a.at.x, window.innerWidth - 392)), top: a.at.y }}
      onKeyDown={(e) => { if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); setAsk(null); } }}>
      <p>{unsavedQuestion(names)}</p>
      <div className="acts">
        <button type="button" className="btn sm primary" onClick={saveAll}>Save and close</button>
        <button type="button" className="btn sm" onClick={() => { setAsk(null); closeNow(a.ids, a.then); }}>Close without saving</button>
        <button type="button" className="btn sm" onClick={() => setAsk(null)}>Cancel</button>
      </div>
    </div>,
    document.body,
  );
}

// ---------- the tab ----------

/** Where a tab's question shows: under it. */
const under = (el: Element | null | undefined) => { const r = el?.getBoundingClientRect(); return r ? { x: r.left, y: r.bottom + 4 } : null; };

/** The file's tab in the island's header: file-code and its name (`name`, with `dir` when two have the same name); the
 *  tooltip its path. Unsaved, its close shows and its label says so. ← and → go to the tab beside it, middle click and ×
 *  close it (asking first with changes not saved). */
export function EditorTab({ id, on, keys, name, dir }: { id: string; on: boolean; keys: string[]; name: string; dir: string | null }) {
  const key = tabKey({ kind: "editor", id });
  const f = useEdFile(id);
  const folders = useEditorFolders(true);
  const { folder, path } = editorTabOf(id);
  const where = `${folders?.find((x) => x.id === folder)?.path ?? folder}/${path}`;
  const close = (el: Element | null) => closeEditorTabs([id], under(el));
  return (
    <div className={`ed-tab editor-tab${f?.dirty ? " unsaved" : ""}`} role="tab" data-tab={key} tabIndex={on ? 0 : -1} aria-selected={on}
      aria-label={`${name}${f?.dirty ? ", not saved" : ""}`} title={where}
      onClick={() => showSideTab(key)}
      onAuxClick={(e) => { if (e.button === 1) { e.preventDefault(); close(e.currentTarget); } }}
      onMouseDown={(e) => { if (e.button === 1) e.preventDefault(); }}
      onKeyDown={(e) => {
        if (e.key !== "ArrowRight" && e.key !== "ArrowLeft") return;
        e.preventDefault();
        const next = stepTab(keys, key, e.key === "ArrowRight" ? 1 : -1);
        if (!next) return;
        showSideTab(next);
        (e.currentTarget.parentElement?.querySelector(`[data-tab="${CSS.escape(next)}"], [data-term="${CSS.escape(next.slice(next.indexOf(":") + 1))}"]`) as HTMLElement | null)?.focus();
      }}
      ref={(el) => { if (el && on) el.scrollIntoView({ block: "nearest", inline: "nearest" }); }}>
      <FileCode className="icon" />
      <span className="name">{name}</span>
      {dir && <span className="dir">{dir}</span>}
      <button className="x" aria-label={`Close ${name}`} title={`Close (${modKey()}+W)`} tabIndex={-1}
        onClick={(e) => { e.stopPropagation(); close(e.currentTarget.parentElement); }}><X className="icon" /></button>
    </div>
  );
}

// ---------- the file ----------

type Cursor = { line: number; col: number; sel: number };
const cursorOf = (s: EditorState): Cursor => {
  const head = s.selection.main.head;
  const line = s.doc.lineAt(head);
  return { line: line.number, col: head - line.from + 1, sel: s.selection.ranges.reduce((n, r) => n + (r.to - r.from), 0) };
};

/** The shown Editor tab's body. `focus` counts up when a tab is opened, picked or shown on the page: the code takes the
 *  focus then (`focusNext`: it came in so). */
export function ChatEditorBody({ id, focus, focusNext }: { id: string; focus: number; focusNext: boolean }) {
  const f = useEdFile(id);
  const host = useRef<HTMLDivElement>(null);
  const view = useRef<EditorView | null>(null);
  const [cursor, setCursor] = useState<Cursor>({ line: 1, col: 1, sel: 0 });
  const [branch, setBranch] = useState<string | null>(null);
  const folders = useEditorFolders(true);
  const { folder, path } = editorTabOf(id);
  const ready = f?.status === "ready" && states.has(id);

  // the view, once the text is read: at the line asked (the first time, and each time it is asked again), else where it was
  useLayoutEffect(() => {
    if (!ready || !host.current) return;
    const v = new EditorView({ state: states.get(id)!, parent: host.current });
    view.current = v;
    views.set(id, v);
    cursorFns.set(id, (s) => setCursor(cursorOf(s)));
    setCursor(cursorOf(v.state));
    void setLanguage(path, v, () => v.state, (n) => v.setState(n));
    const now = files.get(id);
    if (now?.line && wentTo.get(id) !== now.asked) { wentTo.set(id, now.asked); goToMarkedLine(v, now.line); }
    else v.dispatch({ effects: EditorView.scrollIntoView(v.state.selection.main.head, { y: "center" }) });
    if (focusNext) v.focus();
    return () => {
      if (files.has(id)) states.set(id, v.state);
      views.delete(id);
      cursorFns.delete(id);
      v.destroy();
      view.current = null;
    };
  }, [ready, id]); // eslint-disable-line react-hooks/exhaustive-deps

  // another line asked while it shows
  useEffect(() => {
    const v = view.current;
    if (!v || !f?.line || wentTo.get(id) === f.asked) return;
    wentTo.set(id, f.asked);
    goToMarkedLine(v, f.line);
    v.focus();
  }, [f?.asked]); // eslint-disable-line react-hooks/exhaustive-deps

  // picked again (its tab, its icon in the strip): the code takes the focus
  const asked = useRef(focus);
  useEffect(() => {
    if (focus === asked.current) return;
    asked.current = focus;
    view.current?.focus();
  }, [focus]);

  useEffect(() => {
    let alive = true;
    editorGit(folder).then((g) => { if (alive) setBranch(g.branch); }, () => {});
    return () => { alive = false; };
  }, [folder, f?.version]);

  // outside changes: every 3 seconds while the window is in use, and on its focus, as the Editor
  useEffect(() => onWindowFocus(() => checkDisk(id), FOCUS_GAP, 3000), [id]);

  if (!f) return <div className="chat-side-body" />;
  const name = nameOf(path);
  const where = `${folders?.find((x) => x.id === folder)?.path ?? folder}/${path}`;
  const readOnly = !!f.file?.readOnly || f.lock;
  const text = f.file?.text != null;
  return (
    <div className="chat-ed"
      onKeyDown={(e) => {
        if (e.defaultPrevented || !(e.ctrlKey || e.metaKey) || e.altKey || e.shiftKey || e.key.toLowerCase() !== "s") return;
        e.preventDefault();
        void saveFile(id);
      }}>
      <div className="chat-ed-bar">
        <span className="path" title={where}><bdi>{path}</bdi></span>
        {readOnly && text && <span className="ro" title={f.file?.readOnlyWhy ?? (f.lock ? "A card's worktree: edit it on the Editor page" : undefined)}><Lock className="icon" />Read-only</span>}
        <button type="button" className="btn sm" disabled={!f.dirty || f.saving || readOnly} title={`Save (${modKey()}+S)`} onClick={() => { void saveFile(id); }}>Save</button>
      </div>
      {f.disk === "changed" ? (
        <div className="ed-bar file refused" role="alert">
          <TriangleAlert className="icon" />
          <span className="say"><span className="mono">{path}</span> changed on disk while you were editing it.</span>
          <span className="acts">
            <button type="button" className="btn sm" title="Show the version on disk: your unsaved changes here are lost" onClick={() => reload(id)}>Reload</button>
            <button type="button" className="btn sm" title="Save your version over the one on disk" onClick={() => { void saveFile(id, true); }}>Save over it</button>
          </span>
        </div>
      ) : f.disk === "gone" ? (
        <div className="ed-bar file refused" role="alert">
          <TriangleAlert className="icon" />
          <span className="say"><span className="mono">{path}</span> isn't there any more.</span>
          {f.dirty && <span className="acts"><button type="button" className="btn sm" onClick={() => { void saveFile(id); }}>Save it again</button></span>}
        </div>
      ) : f.saveError ? (
        <div className="ed-bar file failed" role="alert">
          <TriangleAlert className="icon" />
          <span className="say">Couldn't save {name}: {f.saveError.replace(/\.$/, "")}. Your changes are still here.</span>
          <span className="acts"><button type="button" className="btn sm" onClick={() => { void saveFile(id); }}>Try again</button></span>
        </div>
      ) : null}
      <div className="chat-ed-code">
        {ready ? <div ref={host} className="ed-host" />
          : <div className="ed-empty">
            {f.status === "loading" ? <span className="faint">Loading…</span>
              : f.status === "error" ? <div className="empty"><b>{f.error}</b></div>
              : f.file?.tooLarge ? <div className="empty"><FileCode className="icon" /><b>Too big to show here.</b></div>
              : f.file?.binary ? <div className="empty"><FileCode className="icon" /><b>A binary file: Hersir shows text only.</b></div>
              : null}
          </div>}
      </div>
      <div className="ed-status chat-ed-status" role="status" aria-label="Status bar">
        {ready && <span className="item">{cursorLabel(cursor.line, cursor.col, cursor.sel)}</span>}
        {text && <span className="item">{languageName(path)}</span>}
        {text && <span className="item">{f.file?.bom ? "UTF-8 with BOM" : "UTF-8"}</span>}
        {branch && <span className="item">{branch}</span>}
        <span className="grow" />
        {f.saving ? <span className="item">Saving…</span> : f.dirty ? <span className="item unsaved">Not saved</span> : null}
      </div>
    </div>
  );
}
