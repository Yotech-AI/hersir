// The Terminal page (GA-135, TF0; GA-136, TF1; design/terminal §3). #/terminal is an empty page with New terminal (the
// Editor's folder picker, titled "New terminal in"), and the terminals open now. #/terminal/<id> is one terminal:
// - the top bar: Terminal / its name (a double-click or F2 renames it in place), the folder in mono, the shell and its size,
//   Show in Editor, and ⋯ (Rename, Find, Clear, Copy folder path, Close);
// - its folder's terminals as tabs, with + for another one there (also Ctrl+Shift+`); Ctrl+PgDn and Ctrl+PgUp switch tabs;
// - a bar under the tabs when the shell ended with another code than 0, with Start again (a new shell in the same folder
//   with the same name) and Close, or when a shell couldn't start, with Try again and Close;
// - the terminal, or TF0's placeholder with Show here while the Editor's panel shows it.
// A terminal that closes while its page shows (× , exit, the sidebar) gives way to the tab next to it.
// GA-348: Layout, right of +, shows the folder's terminals side by side (2, 3 or 4 in a row, or 2 × 2), each pane with a
// caption, its own bar and its terminal. The focused pane's terminal is the page's: its tab is selected, the address is
// its id and the top bar is about it. The rules (which terminals fill the panes, a terminal that closes, stepping down)
// and the panes of each folder are in src/lib/terminalPanes.ts.
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import {
  CircleAlert, Columns2, Columns3, Columns4, Ellipsis, FolderCode, Grid2x2, Info, Plus, RotateCcw, Square, Terminal as TerminalIcon,
  type LucideIcon,
} from "lucide-react";
import { editorFolders } from "../api";
import type { EditorFolder, TerminalSession } from "../types";
import { go, href } from "../router";
import { modKey } from "../lib/keys";
import { onWindowFocus } from "../lib/windowFocus";
import {
  asksToClose, clearTerminal, closeQuestion, endedSay, focusTerminal, folderNameOf, sentence, showsCardId, stepTerminal, tabTitle,
  useTerminalSize, useTerminals, useTerminalsLoaded, type TerminalPlace,
} from "../lib/terminals";
import {
  addPane, choose, focused, keepStarted, layoutLabel, LAYOUTS, missingFor, pagePanes, paneCount, panesOf, swapIn, updatePanes,
  useFolderPanes, type Layout,
} from "../lib/terminalPanes";
import { ContextMenu, type MenuItem } from "../components/editor/Menus";
import { TerminalView } from "../components/terminal/TerminalView";
import { TerminalTabs } from "../components/terminal/TerminalTabs";
import { closeTerminal, NameInput, NewTerminalPicker, showInEditor, startTerminal, TabCloseAsk } from "../components/terminal/TerminalParts";

export function TerminalPage({ id, youId }: { id?: string; youId: string }) {
  return id ? <OneTerminal id={id} youId={youId} /> : <NoTerminal youId={youId} />;
}

/** Where a click asked for the folder picker: under the button. */
const under = (e: React.MouseEvent<HTMLElement>) => { const r = e.currentTarget.getBoundingClientRect(); return { x: r.left, y: r.bottom + 6 }; };

/** The page's place: every pane shows its terminal here (the store keeps where each terminal shows). */
const PAGE: TerminalPlace = { kind: "page" };

/** Each layout's icon: on the Layout button (the current one) and on its menu item. */
const LAYOUT_ICONS: Record<Layout, LucideIcon> = { one: Square, two: Columns2, three: Columns3, four: Columns4, grid: Grid2x2 };

/** #/terminal: New terminal, and the terminals open now. */
function NoTerminal({ youId }: { youId: string }) {
  const list = useTerminals();
  const [at, setAt] = useState<{ x: number; y: number } | null>(null);
  return (
    <div className="term-page">
      <div className="topbar"><div className="crumbs"><b>Terminal</b></div></div>
      <div className="ed-empty">
        <div className="empty">
          <TerminalIcon className="icon" />
          <b>{list.length ? "Terminals" : "No terminal open"}</b>
          <span>Start a shell in a project's folder, a folder in ~/Herd or a card's worktree.</span>
          <button className="btn primary term-new" onClick={(e) => setAt(under(e))}><Plus className="icon" />New terminal</button>
          {list.length > 0 && (
            <div className="term-open">
              {list.map((s) => (
                <a key={s.id} href={href({ page: "terminal", id: s.id })} title={`${s.folderPath} · ${s.shell}`}>
                  <TerminalIcon className="icon" /><span>{s.name}</span>
                  <span className="where">{s.ended ? "ended" : s.busy ?? s.folderPath}</span>
                </a>
              ))}
            </div>
          )}
        </div>
      </div>
      {at && <NewTerminalPicker at={at} you={youId} onClose={() => setAt(null)} />}
    </div>
  );
}

/** A shell that couldn't start (from +, Start again, Try again or a layout): the pane it shows in (its terminal), why,
 *  and what Try again and Close do. */
type Failed = { at: string; why: string; retry: () => void; close: () => void };

/** The folder's terminals on the page: one, or side by side in panes (GA-348), the address's terminal focused. */
function OneTerminal({ id, youId }: { id: string; youId: string }) {
  const list = useTerminals();
  const loaded = useTerminalsLoaded();
  const listNow = useRef(list);
  listNow.current = list;

  // The address's terminal goes in the folder's panes once, when the address changes (the tab rule); after that the panes
  // say which terminal is the page's, and the address follows them. When the address's terminal closes, its folder's
  // panes give the terminal that takes its place.
  const routed = list.find((x) => x.id === id);
  const placed = useRef<{ id: string; folder: string } | null>(null);
  const folder = routed?.folder ?? (placed.current?.id === id ? placed.current.folder : null);
  const stored = useFolderPanes(folder);
  const tabs = folder ? list.filter((x) => x.folder === folder).map((x) => x.id) : [];
  const show = routed && (placed.current?.id !== id || !stored) ? id : null;
  const panes = folder ? pagePanes(stored, tabs, show) : undefined;
  const pageId = focused(panes);
  const s = pageId ? list.find((x) => x.id === pageId) : undefined;
  useLayoutEffect(() => {
    if (!folder) return;
    updatePanes(folder, (cur) => pagePanes(cur, tabs, show));
    if (routed) placed.current = { id, folder };
  });
  // the focused pane moved (a click in a pane, a tab, a terminal that closed): the address follows, with no history entry
  useEffect(() => {
    if (!folder || pageId === id) return;
    const want = href(pageId ? { page: "terminal", id: pageId } : { page: "terminal" });
    if (window.location.hash !== want) window.location.replace(want);
  }, [folder, pageId, id]);

  const size = useTerminalSize(pageId ?? id);
  const [renaming, setRenaming] = useState(false);
  const [more, setMore] = useState<{ x: number; y: number } | null>(null);
  const [layoutAt, setLayoutAt] = useState<{ x: number; y: number } | null>(null);
  const [asking, setAsking] = useState<{ x: number; y: number; question: string } | null>(null);
  /** Per terminal, how often ⋯ → Find was chosen for it: its view opens find each time this counts up. */
  const [finds, setFinds] = useState<Record<string, number>>({});
  const [failed, setFailed] = useState<Failed | null>(null);
  const [starting, setStarting] = useState(false);
  const busy = useRef(false);
  const [picker, setPicker] = useState<{ x: number; y: number } | null>(null);
  const [folderInfo, setFolderInfo] = useState<EditorFolder | null>(null);
  const moreBtn = useRef<HTMLButtonElement>(null);
  const layoutBtn = useRef<HTMLButtonElement>(null);

  // whether an agent works on the folder (a card's worktree) now: then + is off, with why
  const folderId = s?.folder ?? null;
  useEffect(() => {
    if (!folderId) return;
    let alive = true;
    const load = () => { editorFolders().then((fs) => { if (alive) setFolderInfo(fs.find((f) => f.id === folderId) ?? null); }).catch(() => {}); };
    load();
    const stop = onWindowFocus(load);
    return () => { alive = false; stop(); };
  }, [folderId]);
  const newOff = folderInfo?.readOnly ? `${folderInfo.readOnlyWhy ?? "An agent is working on it"}: a terminal there waits until its run ends.` : null;

  useEffect(() => { setRenaming(false); setMore(null); setAsking(null); }, [pageId]);

  /** One start at a time: +, Start again, Try again and a layout's missing terminals. */
  const begin = () => {
    if (busy.current) return false;
    busy.current = true;
    setStarting(true);
    return true;
  };
  const done = () => { busy.current = false; setStarting(false); };

  /** Starts a terminal in the folder (+, Ctrl+Shift+`) and shows it in the focused pane; or another in place of an ended
   *  one (Start again), in that one's pane. */
  const start = (inFolder: string, again: TerminalSession | null) => {
    if (!begin()) return;
    setFailed(null);
    const at = again?.id ?? focused(panesOf(inFolder)) ?? "";
    startTerminal(inFolder, again ? again.name : null)
      .then((n) => {
        done();
        if (again) updatePanes(inFolder, (p) => p && swapIn(p, again.id, n.id));
        go({ page: "terminal", id: n.id });
        if (again) closeTerminal(again.id); // after the new one is there, so the page never loses its place
      })
      .catch((e) => {
        done();
        setFailed({ at, why: sentence(String(e)), retry: () => start(inFolder, again), close: () => (again ? closeTerminal(again.id) : setFailed(null)) });
      });
  };

  /** A layout from the Layout menu: the page's terminal in the first pane, then the folder's next tabs; the terminals it
   *  lacks start one after the other, each in its pane. One that can't start: the layout keeps the panes that got one. */
  const pick = (inFolder: string, l: Layout) => {
    const p = panesOf(inFolder);
    const current = focused(p);
    if (!p || !current) return;
    if (p.layout === l && p.ids.length === paneCount(l)) return;
    const { panes: next, missing } = choose(listNow.current.filter((x) => x.folder === inFolder).map((x) => x.id), current, l);
    if (missing > 0 && !begin()) return;
    setFailed(null);
    updatePanes(inFolder, () => next);
    requestAnimationFrame(() => focusTerminal(current)); // the menu had the keyboard; the page's terminal has it again
    if (missing > 0) startMissing(inFolder, l, missing);
  };
  const startMissing = (inFolder: string, l: Layout, n: number) => {
    startTerminal(inFolder)
      .then((t) => {
        updatePanes(inFolder, (p) => p && addPane(p, t.id));
        if (n > 1) startMissing(inFolder, l, n - 1);
        else done();
      })
      .catch((e) => {
        done();
        updatePanes(inFolder, (p) => p && keepStarted(p));
        setFailed({ at: focused(panesOf(inFolder)) ?? "", why: sentence(String(e)), retry: () => pick(inFolder, l), close: () => setFailed(null) });
      });
  };

  /** A click in a pane or on its caption: that pane is focused (the address follows with replace). */
  const focusPane = (t: string) => {
    if (!folder || t === pageId) return;
    updatePanes(folder, (p) => (p && p.ids.includes(t) ? { ...p, focus: p.ids.indexOf(t) } : p));
  };
  /** A tab, Ctrl+PgDn and Ctrl+PgUp: a terminal in a pane is focused there; another comes into the focused pane. */
  const open = (t: string) => {
    if (panes?.ids.includes(t)) { focusPane(t); focusTerminal(t); }
    else go({ page: "terminal", id: t });
  };

  const close = (s: TerminalSession) => {
    if (!asksToClose(s)) { closeTerminal(s.id); return; }
    const r = document.querySelector(`.term-tabs [data-term="${CSS.escape(s.id)}"]`)?.getBoundingClientRect();
    setAsking({ x: r?.left ?? 240, y: (r?.bottom ?? 80) + 4, question: closeQuestion(s) });
  };

  // the page's keys: Ctrl+PgDn and Ctrl+PgUp (they come through from the terminal), Ctrl+Shift+` and F2
  const keys = useRef<(e: KeyboardEvent) => void>(() => {});
  keys.current = (e) => {
    if (!s || e.defaultPrevented) return;
    const ctrl = e.ctrlKey && !e.metaKey && !e.altKey;
    if (ctrl && !e.shiftKey && (e.key === "PageDown" || e.key === "PageUp")) {
      e.preventDefault();
      const next = stepTerminal(tabs, s.id, e.key === "PageDown" ? 1 : -1);
      if (next && next !== s.id) open(next);
    } else if (ctrl && e.shiftKey && e.code === "Backquote") {
      e.preventDefault();
      if (!newOff) start(s.folder, null);
    } else if (e.key === "F2" && !e.ctrlKey && !e.metaKey && !e.altKey && !e.shiftKey) {
      const t = e.target as HTMLElement | null;
      if (t?.closest?.(".xterm, input, textarea, [contenteditable], .side, [role=tab], [role=dialog], [role=menu]")) return;
      e.preventDefault();
      setRenaming(true);
    }
  };
  useEffect(() => {
    const on = (e: KeyboardEvent) => keys.current(e);
    window.addEventListener("keydown", on);
    return () => window.removeEventListener("keydown", on);
  }, []);

  if (!s || !panes) {
    return (
      <div className="term-page">
        <div className="topbar"><div className="crumbs"><a href={href({ page: "terminal" })}>Terminal</a></div><div className="actions" /></div>
        {loaded && !folder && (
          <div className="ed-empty">
            <div className="empty">
              <TerminalIcon className="icon" />
              <b>This terminal isn't open any more.</b>
              <button className="btn" onClick={(e) => setPicker(under(e))}><Plus className="icon" />New terminal</button>
            </div>
          </div>
        )}
        {picker && <NewTerminalPicker at={picker} you={youId} onClose={() => setPicker(null)} />}
      </div>
    );
  }

  const name = folderNameOf(s);
  const cols = size?.cols ?? s.cols;
  const rows = size?.rows ?? s.rows;
  const isMac = modKey() === "Cmd";
  const items: MenuItem[] = [
    { label: "Rename", kbd: "F2", run: () => setRenaming(true) },
    { label: "Find", kbd: isMac ? "Cmd F" : "Ctrl Shift F", run: () => setFinds((f) => ({ ...f, [s.id]: (f[s.id] ?? 0) + 1 })) },
    { label: "Clear", title: "Empties the scrollback", run: () => clearTerminal(s.id) },
    { label: "Copy folder path", run: () => { navigator.clipboard?.writeText(s.folder).catch(() => {}); } },
    { sep: true },
    { label: "Close", run: () => close(s) },
  ];
  const layout = panes.layout;
  const LayoutIcon = LAYOUT_ICONS[layout];
  const layoutItems: MenuItem[] = LAYOUTS.map((l) => {
    const Icon = LAYOUT_ICONS[l.id];
    const off = !!newOff && missingFor(l.id, tabs) > 0;
    return {
      label: l.label, icon: <Icon className="icon kind-icon" />, checked: l.id === layout, disabled: off, title: off ? newOff ?? undefined : undefined,
      run: () => pick(s.folder, l.id),
    };
  });

  /** A terminal's bar: why it couldn't start (Try again, Close), or that its shell ended (Start again, Close). */
  const bar = (t: TerminalSession) => failed && failed.at === t.id ? (
    <div className="ed-bar file failed" role="alert">
      <CircleAlert className="icon" />
      <span className="say">{failed.why}</span>
      <span className="acts">
        <button className="btn sm" disabled={starting} onClick={failed.retry}><RotateCcw className="icon" />Try again</button>
        <button className="btn sm ghost" onClick={failed.close}>Close</button>
      </span>
    </div>
  ) : t.ended ? (
    <div className="ed-bar file" role="status">
      <Info className="icon" />
      <span className="say">{endedSay(t.ended)}</span>
      <span className="acts">
        <button className="btn sm" disabled={starting} onClick={() => start(t.folder, t)}><RotateCcw className="icon" />Start again</button>
        <button className="btn sm ghost" onClick={() => closeTerminal(t.id)}>Close</button>
      </span>
    </div>
  ) : null;

  const count = paneCount(layout);
  const slots = Array.from({ length: count }, (_, i) => panes.ids[i] ?? null);

  return (
    <div className="term-page">
      <div className="topbar">
        <div className="crumbs ed-crumbs">
          <a href={href({ page: "terminal" })}>Terminal</a><span className="sep">/</span>
          {renaming ? <NameInput session={s} onDone={() => setRenaming(false)} />
            : <b title="Double-click to rename (F2)" onDoubleClick={() => setRenaming(true)}>{s.name}</b>}
        </div>
        <div className="term-where">
          <span className="path" title={s.folderPath}>{s.folderPath}</span>
          <span className="sep">·</span><span>{s.shell}</span>
          <span className="sep">·</span><span className="size" title={`${cols} columns, ${rows} rows`}>{cols} × {rows}</span>
        </div>
        <div className="actions">
          <button className="btn sm" title={`Show ${s.name} in ${name}'s Editor, in its terminal panel`} onClick={() => showInEditor(s)}>
            <FolderCode className="icon" />Show in Editor
          </button>
          <button ref={moreBtn} className="btn ghost sm icon-only" aria-label="More" title="More" aria-haspopup="menu" aria-expanded={!!more}
            onClick={() => { const r = moreBtn.current?.getBoundingClientRect(); setMore(more ? null : { x: (r?.right ?? 0) - 240, y: (r?.bottom ?? 0) + 4 }); }}>
            <Ellipsis className="icon" />
          </button>
        </div>
      </div>
      <TerminalTabs folder={s.folder} active={s.id} onOpen={open} onNew={() => start(s.folder, null)} newOff={newOff}
        paned={count > 1 ? panes.ids : []}
        after={
          <button ref={layoutBtn} className="btn ghost sm icon-only term-layout" aria-label="Layout" title={`Layout: ${layoutLabel(layout)}`}
            aria-haspopup="menu" aria-expanded={!!layoutAt}
            onClick={() => { const r = layoutBtn.current?.getBoundingClientRect(); setLayoutAt(layoutAt ? null : { x: (r?.right ?? 0) - 200, y: (r?.bottom ?? 0) + 4 }); }}>
            <LayoutIcon className="icon" />
          </button>
        } />
      {count === 1 ? (
        <>
          {bar(s)}
          <TerminalView key={s.id} id={s.id} place={PAGE} autoFocus findAsk={finds[s.id] ?? 0} />
        </>
      ) : (
        <div className="term-panes" data-layout={layout}>
          {slots.map((t, i) => {
            const ts = t ? list.find((x) => x.id === t) : undefined;
            if (!t || !ts) {
              return (
                <div key={`wait-${i}`} className="term-pane wait" aria-busy="true">
                  <div className="term-cap" />
                  <div className="term-place"><div className="term-away"><span>Starting a terminal…</span></div></div>
                </div>
              );
            }
            const on = t === s.id;
            return (
              <section key={t} className={`term-pane${on ? " on" : ""}`} aria-label={ts.name}
                onMouseDownCapture={() => focusPane(t)} onFocusCapture={() => focusPane(t)}
                onClick={(e) => { if (!e.currentTarget.contains(document.activeElement)) focusTerminal(t); }}>
                <div className="term-cap" title={tabTitle(ts)} onClick={() => { focusPane(t); focusTerminal(t); }}>
                  <TerminalIcon className="icon" />
                  <span className={`name${ts.taskId && showsCardId(ts.name, folderNameOf(ts)) ? " card-id" : ""}`}>{ts.name}</span>
                  {ts.ended ? <span className="state">ended</span> : ts.busy ? <span className="prog">{ts.busy}</span> : null}
                </div>
                {bar(ts)}
                <TerminalView key={t} id={t} place={PAGE} autoFocus={on} findAsk={finds[t] ?? 0} />
              </section>
            );
          })}
        </div>
      )}
      {more && <ContextMenu at={more} label="More" items={items} onClose={() => setMore(null)} />}
      {layoutAt && <ContextMenu at={layoutAt} label="Layout" items={layoutItems} onClose={() => setLayoutAt(null)} />}
      {asking && !s.ended && <TabCloseAsk name={s.name} question={asking.question} at={asking} onCancel={() => setAsking(null)} onClose={() => { setAsking(null); closeTerminal(s.id); }} />}
    </div>
  );
}
