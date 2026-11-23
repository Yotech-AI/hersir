// GA-343, GA-345: Hersir's view in Claude Desktop's chat (MCP Apps). It draws the board show's result brings with the
// app's own Board, read-only. Claude pushes nothing to a view, and every tool call mounts a new view, so the board's state
// lives in Hersir: every few seconds the view asks board_changes what changed since the counter of its last read (seq),
// and reads the board again with view_read only when something did. The theme and the display mode follow the host; Full
// screen is offered when the host has it (hosts allow it only after a click or a key). The card and the Chief's thread
// (show's other views) come with GA-349: until then the view says so.
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Maximize2, Minimize2 } from "lucide-react";
import type { McpUiDisplayMode, McpUiHostContext } from "@modelcontextprotocol/ext-apps";
import { Board } from "../components/Board";
import { Bridge, reportSize, type ToolResult } from "./bridge";
import { boardOf, statesOf, tasksOf, titleOf, workingOf } from "./board";
import { changesOf, viewOf, type BoardData } from "./views";

/** How often the view asks board_changes while it is shown (the card: every 2 to 5 s). */
export const POLL_MS = 3000;

/** After a failed read: the next one waits longer, up to this. */
const POLL_MAX_MS = 15_000;

/** The tool that reads the view's data again, and the one that says what changed since (both visibility app). */
export const VIEW_READ = "view_read";
export const BOARD_CHANGES = "board_changes";

/** The text of a tool's result, for a message. */
function textOf(r: ToolResult): string {
  return (r.content ?? []).map((c) => c.text ?? "").join(" ").trim();
}

/** The theme on the document, as ext-apps' applyDocumentTheme sets it: tokens.css has a dark and a light one. */
function applyTheme(theme: string | undefined) {
  const t = theme === "light" || theme === "dark" ? theme
    : window.matchMedia?.("(prefers-color-scheme: light)").matches ? "light" : "dark";
  document.documentElement.setAttribute("data-theme", t);
  document.documentElement.style.colorScheme = t;
}

const noop = () => {};

export function BoardView() {
  const [board, setBoard] = useState<BoardData | null>(null);
  const [note, setNote] = useState<string | null>("Loading the board…");
  const [updated, setUpdated] = useState<Date | null>(null);
  const [ctx, setCtx] = useState<McpUiHostContext>({});
  const bridge = useRef<Bridge | null>(null);
  // The project view_read asks for: the one show's result was for (null: every project on the board).
  const project = useRef<string | null>(null);
  // board_changes' counter as of the last read (view_read's seq; show's result has none, so the first poll reads).
  const seq = useRef<number | null>(null);
  const [started, setStarted] = useState(false);

  /** A result of show or view_read: its board, or why there is none. */
  const take = useCallback((r: ToolResult) => {
    const v = viewOf(r.structuredContent);
    const b = v?.view === "board" ? boardOf(v.data) : null;
    if (r.isError || !v || (v.view === "board" && !b)) {
      setNote(textOf(r) || "Hersir didn't send the board.");
      return false;
    }
    if (!b) {
      setNote("This Hersir shows only the board here: the card and the Chief's thread come in a later version.");
      return false;
    }
    project.current = v.id;
    if (v.seq !== undefined) seq.current = v.seq;
    setBoard(b);
    setNote(null);
    setUpdated(new Date());
    return true;
  }, []);

  useEffect(() => {
    const b = new Bridge(window.parent, {
      toolResult: (r) => { if (take(r)) setStarted(true); },
      toolCancelled: () => setNote("The board wasn't loaded: the tool call was cancelled."),
      context: (c) => { setCtx(c); applyTheme(c.theme); },
      teardown: () => setStarted(false),
    });
    bridge.current = b;
    applyTheme(undefined);
    // The size goes to the host once the handshake is done, as ext-apps' App sends it.
    let alive = true, stopSize: (() => void) | undefined;
    b.connect({ name: "Hersir", version: "1" })
      .then(() => { if (alive) stopSize = reportSize(b); })
      .catch((e) => { if (alive) setNote(`Couldn't reach the chat: ${e instanceof Error ? e.message : String(e)}`); });
    return () => { alive = false; stopSize?.(); b.close(); bridge.current = null; };
  }, [take]);

  // Every POLL_MS while the view is shown, one call at a time: board_changes since the last read, and view_read when
  // something changed (or there is no counter yet); slower after a failure.
  useEffect(() => {
    if (!started) return;
    let alive = true, timer: ReturnType<typeof setTimeout> | undefined, wait = POLL_MS;
    const read = async (b: Bridge) => {
      if (seq.current !== null) {
        const c = changesOf((await b.callTool(BOARD_CHANGES, { since: seq.current })).structuredContent);
        if (c && !c.reload && !c.board && c.cards.length === 0) {
          seq.current = c.seq;
          setUpdated(new Date());
          return true;
        }
      }
      return take(await b.callTool(VIEW_READ, project.current ? { view: "board", id: project.current } : { view: "board" }));
    };
    const tick = async () => {
      if (!alive) return;
      if (document.visibilityState === "hidden" || !bridge.current) { timer = setTimeout(tick, POLL_MS); return; }
      try {
        wait = (await read(bridge.current)) && alive ? POLL_MS : Math.min(wait * 2, POLL_MAX_MS);
      } catch (e) {
        if (alive) setNote(`Couldn't read the board: ${e instanceof Error ? e.message : String(e)}`);
        wait = Math.min(wait * 2, POLL_MAX_MS);
      }
      if (alive) timer = setTimeout(tick, wait);
    };
    timer = setTimeout(tick, POLL_MS);
    return () => { alive = false; clearTimeout(timer); };
  }, [started, take]);

  const mode: McpUiDisplayMode = ctx.displayMode ?? "inline";
  const modes = ctx.availableDisplayModes ?? [];
  const next: McpUiDisplayMode | null = mode === "fullscreen" ? (modes.includes("inline") ? "inline" : null) : modes.includes("fullscreen") ? "fullscreen" : null;
  const switchMode = () => {
    if (!next || !bridge.current) return;
    bridge.current.requestDisplayMode(next)
      .then((set) => setCtx((c) => ({ ...c, displayMode: set })))
      .catch((e) => setNote(`Couldn't change the view: ${e instanceof Error ? e.message : String(e)}`));
  };

  const tasks = useMemo(() => (board ? tasksOf(board) : []), [board]);
  const states = useMemo(() => (board ? statesOf(board) : []), [board]);
  const working = useMemo(() => (board ? workingOf(board) : new Map<string, string>()), [board]);

  return (
    <div className={`mcp-board ${mode}`}>
      <div className="mcp-head">
        <span className="mcp-title">{board ? titleOf(board) : "Hersir"}</span>
        {updated && <span className="mcp-updated faint" title="board_changes and view_read keep it up to date">Updated {updated.toLocaleTimeString()}</span>}
        <span className="grow" />
        {next && (
          <button className="btn ghost sm" onClick={switchMode}>
            {next === "fullscreen" ? <><Maximize2 className="icon" />Full screen</> : <><Minimize2 className="icon" />Back to chat</>}
          </button>
        )}
      </div>
      {note && <div className="mcp-note" role="status">{note}</div>}
      {board && states.length > 0 && <Board tasks={tasks} states={states} working={working} onMove={noop} onOpen={noop} readOnly />}
    </div>
  );
}
