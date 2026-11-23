// GA-343, GA-345, GA-349: Hersir's view in Claude Desktop's chat (MCP Apps): the board, a card or the Chief's thread, as
// show's input and result say ({view, id, data}). A click on a card opens it here, and Back returns to the board.
//
// - Claude pushes nothing to a view, and every tool call mounts a new one, so the state lives in Hersir. While the view is
//   shown it asks board_changes every POLL_MS what changed since the counter of its last read (seq), and reads its view
//   again with view_read only when that touches it (`touches`). A card an agent works on now is read every time, for its
//   live output. The Chief's thread isn't on the board: its view asks chief_reply while the Chief works (its reply comes in
//   as it writes) and view_read otherwise. One call at a time, slower after a failure.
// - It writes with the model's own tools (move_task, comment_on_task, start_agent_run, stop_agent_run, ask_chief): an
//   answer that is an error shows its text by its button; after a write the view is read again at once.
// - Full screen only after a click (Open board, Open full screen): hosts allow it only then, and the view never asks on
//   load. It draws in whatever mode the host returns. The theme follows the host's.
import { useCallback, useEffect, useRef, useState } from "react";
import { ArrowLeft, Maximize2, Minimize2, SquareKanban } from "lucide-react";
import type { McpUiDisplayMode, McpUiHostContext } from "@modelcontextprotocol/ext-apps";
import { Bridge, reportSize, type ToolResult } from "./bridge";
import { BoardView, MOVE_TASK } from "./BoardView";
import { CardView } from "./CardView";
import { ChiefView } from "./ChiefView";
import { boardOf, titleOf } from "./board";
import {
  cardOf, changesOf, chiefOf, jsonOf, keyOf, sentOf, touches, viewOf, type BoardData, type CardData, type ChiefData, type ViewName,
} from "./views";

/** How often the view asks board_changes while it is shown (the card: every 2 to 5 s). */
export const POLL_MS = 3000;

/** While the Chief works: how often its view asks chief_reply, so its reply comes in as it is written. */
export const CHIEF_POLL_MS = 1500;

/** After a failed read: the next one waits longer, up to this. */
const POLL_MAX_MS = 15_000;

/** The view's own tools (visibility app), and the Chief's two (the model's, as Claude Desktop has them). */
export const VIEW_READ = "view_read";
export const BOARD_CHANGES = "board_changes";
export const ASK_CHIEF = "ask_chief";
export const CHIEF_REPLY = "chief_reply";

/** Where the view is: the board, a card or the Chief's thread, by its id. A new object for every move, so an answer that
 *  comes for an earlier place is dropped. `back`: the board a card was opened from (Back returns there). */
export type Place = { view: ViewName; id: string | null; back?: Place };

/** The text of a tool's result, for a message. */
export function textOf(r: ToolResult): string {
  return (r.content ?? []).map((c) => c.text ?? "").join(" ").trim();
}

/** An error's message, for a note. */
const said = (e: unknown) => (e instanceof Error ? e.message : String(e));

/** The theme on the document, as ext-apps' applyDocumentTheme sets it: tokens.css has a dark and a light one, and dark
 *  comes first when the host says neither and the system has no light preference. */
function applyTheme(theme: string | undefined) {
  const t = theme === "light" || theme === "dark" ? theme
    : window.matchMedia?.("(prefers-color-scheme: light)").matches ? "light" : "dark";
  document.documentElement.setAttribute("data-theme", t);
  document.documentElement.style.colorScheme = t;
}

/** view_read's (and show's) arguments for a place. */
const argsOf = (p: Place) => (p.id ? { view: p.view, id: p.id } : { view: p.view });

/** What a place shows, for "Loading …" and "Hersir didn't send …". */
const WHAT: Record<ViewName, string> = { board: "the board", card: "the card", chief: "the Chief's thread" };

const noop = () => {};

export function App() {
  const [ctx, setCtx] = useState<McpUiHostContext>({});
  const [place, setPlaceState] = useState<Place | null>(null);
  const [board, setBoard] = useState<BoardData | null>(null);
  const [card, setCard] = useState<CardData | null>(null);
  const [chief, setChief] = useState<ChiefData | null>(null);
  const [note, setNote] = useState<string | null>("Loading…");
  const [moveError, setMoveError] = useState<string | null>(null);
  const [updated, setUpdated] = useState<Date | null>(null);
  const [started, setStarted] = useState(false);
  // The board: drawn whole (Open board) instead of its summary.
  const [whole, setWhole] = useState(false);
  // The card's project's columns, for Move to: by the project's key.
  const [columns, setColumns] = useState<{ key: string | null; names: string[] } | null>(null);

  const bridge = useRef<Bridge | null>(null);
  const placeRef = useRef<Place | null>(null);
  // The place whose data is in: until it is the one on show, the poll reads it with view_read.
  const read = useRef<Place | null>(null);
  // board_changes' counter as of the last read (view_read's seq; show's result has none, so the first poll reads).
  const seq = useRef<number | null>(null);
  const boardRef = useRef<BoardData | null>(null);
  const cardRef = useRef<CardData | null>(null);
  const chiefRef = useRef<ChiefData | null>(null);
  boardRef.current = board;
  cardRef.current = card;
  chiefRef.current = chief;
  // Runs the poll now (after a move or a write); set while the poll runs.
  const wake = useRef<() => void>(noop);

  const setPlace = useCallback((p: Place) => {
    placeRef.current = p;
    setPlaceState(p);
  }, []);

  /** A result of show (`asked` null) or view_read (for the place `asked`): its data, or why there is none. False when it
   *  failed; an answer for a place no longer on show is dropped. */
  const take = useCallback((r: ToolResult, asked: Place | null): boolean => {
    if (asked && asked !== placeRef.current) return true;
    const v = viewOf(r.structuredContent);
    const want = asked?.view ?? v?.view ?? placeRef.current?.view ?? "board";
    if (r.isError || !v || v.view !== want) {
      setNote(textOf(r) || `Hersir didn't send ${WHAT[want]}.`);
      return false;
    }
    const data = v.view === "board" ? boardOf(v.data) : v.view === "card" ? cardOf(v.data) : chiefOf(v.data);
    if (!data) {
      setNote(`Hersir didn't send ${WHAT[v.view]}.`);
      return false;
    }
    let p = asked;
    if (!p) {
      // show's result: the place it shows, as Hersir names it (a project's key, a card's identifier, a thread's id)
      p = { view: v.view, id: v.id };
      setPlace(p);
    }
    if (v.view === "board") setBoard(data as BoardData);
    else if (v.view === "card") setCard(data as CardData);
    else setChief(data as ChiefData);
    if (v.seq !== undefined) seq.current = v.seq;
    read.current = p;
    setNote(null);
    setUpdated(new Date());
    return true;
  }, [setPlace]);

  useEffect(() => {
    const b = new Bridge(window.parent, {
      // show's input comes before its result: what is loading
      toolInput: (args) => {
        const view = args.view === "card" || args.view === "chief" ? args.view : "board";
        if (placeRef.current) return;
        setPlace({ view, id: typeof args.id === "string" && args.id ? args.id : null });
        setNote(`Loading ${WHAT[view]}…`);
      },
      toolResult: (r) => {
        take(r, null);
        // also after an error (Hersir wasn't open): the poll reads again until it is
        if (placeRef.current) setStarted(true);
      },
      toolCancelled: () => setNote("Nothing to show: the tool call was cancelled."),
      context: (c) => { setCtx(c); applyTheme(c.theme); },
      teardown: () => setStarted(false),
    });
    bridge.current = b;
    applyTheme(undefined);
    // The size goes to the host once the handshake is done, as ext-apps' App sends it.
    let alive = true, stopSize: (() => void) | undefined;
    b.connect({ name: "Hersir", version: "1" })
      .then(() => { if (alive) stopSize = reportSize(b); })
      .catch((e) => { if (alive) setNote(`Couldn't reach the chat: ${said(e)}`); });
    return () => { alive = false; stopSize?.(); b.close(); bridge.current = null; };
  }, [take, setPlace]);

  /** One poll: what the place on show needs now. The time until the next one, or null after a failure. */
  const step = useCallback(async (b: Bridge): Promise<number | null> => {
    const p = placeRef.current;
    if (!p) return POLL_MS;
    if (p.view === "chief") {
      const c = chiefRef.current;
      if (read.current === p && c?.thread && c.status === "working") {
        const r = await b.callTool(CHIEF_REPLY, { thread: c.thread, wait_seconds: 0 });
        if (p !== placeRef.current) return 0;
        const d = r.isError ? null : chiefOf(jsonOf(textOf(r)));
        if (!d) {
          setNote(textOf(r) || "Hersir didn't send the Chief's reply.");
          return null;
        }
        setChief(d);
        setNote(null);
        setUpdated(new Date());
        return d.status === "working" ? CHIEF_POLL_MS : POLL_MS;
      }
      if (!take(await b.callTool(VIEW_READ, argsOf(p)), p)) return null;
      return chiefRef.current?.status === "working" ? CHIEF_POLL_MS : POLL_MS;
    }
    // A card an agent works on: read every time, for its live output.
    const live = p.view === "card" && !!cardRef.current?.live;
    if (seq.current !== null && read.current === p && !live) {
      const c = changesOf((await b.callTool(BOARD_CHANGES, { since: seq.current })).structuredContent);
      if (p !== placeRef.current) return 0;
      const onBoard = (id: string) => !!boardRef.current?.columns.some((col) => col.cards.some((x) => x.identifier === id));
      if (c && !touches(c, p.view, p.id, onBoard)) {
        seq.current = c.seq;
        setNote(null);
        setUpdated(new Date());
        return POLL_MS;
      }
    }
    return take(await b.callTool(VIEW_READ, argsOf(p)), p) ? POLL_MS : null;
  }, [take]);

  // The poll, while the view is shown: one call at a time; wake() runs it now (after a move or a write).
  useEffect(() => {
    if (!started) return;
    let alive = true, timer: ReturnType<typeof setTimeout> | undefined, busy = false, again = false, wait = POLL_MS;
    const run = async () => {
      if (!alive) return;
      clearTimeout(timer);
      if (busy) { again = true; return; }
      if (document.visibilityState === "hidden" || !bridge.current) { timer = setTimeout(run, POLL_MS); return; }
      busy = true;
      let next: number;
      try {
        const ms = await step(bridge.current);
        wait = ms === null ? Math.min(wait * 2, POLL_MAX_MS) : POLL_MS;
        next = ms ?? wait;
      } catch (e) {
        if (alive) setNote(`Couldn't read ${WHAT[placeRef.current?.view ?? "board"]}: ${said(e)}`);
        wait = Math.min(wait * 2, POLL_MAX_MS);
        next = wait;
      }
      busy = false;
      if (!alive) return;
      if (again) { again = false; next = 0; }
      timer = setTimeout(run, next);
    };
    wake.current = () => void run();
    timer = setTimeout(run, POLL_MS);
    return () => { alive = false; clearTimeout(timer); wake.current = noop; };
  }, [started, step]);

  /** Goes to a place: its data is read at once (what was shown of it stays meanwhile). */
  const go = useCallback((p: Place) => {
    setPlace(p);
    setMoveError(null);
    if (p.view === "card" && cardRef.current?.identifier.toUpperCase() !== p.id?.toUpperCase()) {
      setCard(null);
      setNote(`Loading ${p.id}…`);
    }
    wake.current();
  }, [setPlace]);

  const openCard = useCallback((identifier: string) => {
    const from = placeRef.current;
    go({ view: "card", id: identifier, back: from?.view === "board" ? from : from?.back });
  }, [go]);

  const goBack = () => {
    const back = placeRef.current?.back;
    if (back) go({ view: back.view, id: back.id });
  };

  /** One of the model's tools that changes something. Its error's text, or null when it went; then the view is read again. */
  const write = useCallback(async (name: string, args: Record<string, unknown>): Promise<string | null> => {
    const b = bridge.current;
    if (!b) return "The chat isn't connected.";
    try {
      const r = await b.callTool(name, args);
      if (r.isError) return textOf(r) || `${name} didn't work.`;
      read.current = null;
      wake.current();
      return null;
    } catch (e) {
      return said(e);
    }
  }, []);

  /** A card dragged to another column on the whole board: it moves there at once (to the bottom, as move_task puts it),
   *  then move_task; an error puts it back with the next read and says why above the board. */
  const move = useCallback(async (identifier: string, column: string) => {
    setMoveError(null);
    setBoard((b) => {
      if (!b) return b;
      const moved = b.columns.flatMap((c) => c.cards).find((x) => x.identifier === identifier);
      if (!moved) return b;
      return {
        ...b,
        columns: b.columns.map((c) => {
          const cards = c.cards.filter((x) => x.identifier !== identifier);
          const total = c.total - (c.cards.length - cards.length);
          return c.name === column ? { ...c, total: total + 1, cards: [...cards, moved] } : { ...c, total, cards };
        }),
      };
    });
    const error = await write(MOVE_TASK, { task: identifier, column });
    if (error) {
      setMoveError(`Couldn't move ${identifier} to ${column}: ${error}`);
      read.current = null;
      wake.current();
    }
  }, [write]);

  /** The Chief: a message with ask_chief, to the thread on show (none: the chat From Claude Desktop). */
  const ask = useCallback(async (message: string): Promise<string | null> => {
    const b = bridge.current;
    if (!b) return "The chat isn't connected.";
    const c = chiefRef.current;
    const thread = c?.thread ?? placeRef.current?.id ?? null;
    try {
      const r = await b.callTool(ASK_CHIEF, thread ? { message, thread } : { message });
      if (r.isError) return textOf(r) || "ask_chief didn't work.";
      const sent = sentOf(jsonOf(textOf(r)));
      setChief((now) => ({
        thread: sent?.thread ?? now?.thread ?? thread, status: "working", draft: now?.draft ?? null,
        messages: [...(now?.messages ?? []), { id: sent?.message ?? `sent-${Date.now()}`, from: "you", time: new Date().toISOString(), text: message }],
      }));
      // its reply as it comes: chief_reply now, then while the Chief works
      wake.current();
      return null;
    } catch (e) {
      return said(e);
    }
  }, []);

  /** Opens a web link in the user's browser through the chat (ui/open-link). */
  const openLink = useCallback((url: string) => {
    bridge.current?.openLink(url).catch((e) => setNote(`Couldn't open ${url}: ${said(e)}`));
  }, []);

  // Move to: the card's project's columns, read once (from the board on show when it is that project's).
  const cardKey = card ? keyOf(card.identifier) : null;
  const boardKey = board?.project?.key ?? null;
  const names = cardKey && boardKey && boardKey.toUpperCase() === cardKey.toUpperCase() ? board!.columns.map((c) => c.name)
    : columns && columns.key === cardKey ? columns.names : null;
  const loadColumns = useCallback(async () => {
    const b = bridge.current;
    if (!b || !cardKey) return;
    try {
      const r = await b.callTool(VIEW_READ, { view: "board", id: cardKey });
      const v = viewOf(r.structuredContent);
      const got = !r.isError && v?.view === "board" ? boardOf(v.data) : null;
      setColumns({ key: cardKey, names: got ? got.columns.map((c) => c.name) : [] });
    } catch {
      setColumns({ key: cardKey, names: [] });
    }
  }, [cardKey]);

  // Display modes: the host's. Full screen only after a click; the view draws in the mode the host returns.
  const mode: McpUiDisplayMode = ctx.displayMode ?? "inline";
  const lastMode = useRef(mode);
  useEffect(() => {
    // back from full screen (Claude's own control, or Back to chat): the board's summary again
    if (lastMode.current === "fullscreen" && mode !== "fullscreen") setWhole(false);
    lastMode.current = mode;
  }, [mode]);
  const askMode = async (want: McpUiDisplayMode) => {
    if (!bridge.current) return;
    try {
      const set = await bridge.current.requestDisplayMode(want);
      setCtx((c) => ({ ...c, displayMode: set }));
    } catch (e) {
      setNote(`Couldn't change the view: ${said(e)}`);
    }
  };
  const openBoard = () => {
    setWhole(true);
    void askMode("fullscreen");
  };

  const view = place?.view ?? "board";
  const showBoard = view === "board" && board;
  const title = view === "board" ? (board ? titleOf(board) : "Hersir")
    : view === "card" ? (card ? card.identifier : place?.id ?? "Card")
    : "Chief";

  return (
    <div className={`mcp-app ${mode} view-${view}`}>
      <div className="mcp-head">
        {place?.back && (
          <button className="btn ghost sm" onClick={goBack} title="Back to the board"><ArrowLeft className="icon" />Back</button>
        )}
        <span className="mcp-title">{title}</span>
        {updated && <span className="mcp-updated faint" title="Hersir keeps it up to date">Updated {updated.toLocaleTimeString()}</span>}
        <span className="grow" />
        {mode === "fullscreen" ? (
          <button className="btn ghost sm" onClick={() => void askMode("inline")}><Minimize2 className="icon" />Back to chat</button>
        ) : view === "board" ? (
          whole ? <button className="btn ghost sm" onClick={() => setWhole(false)}>Show the summary</button>
            : board && <button className="btn sm" onClick={openBoard}><SquareKanban className="icon" />Open board</button>
        ) : (
          <button className="btn ghost sm" onClick={() => void askMode("fullscreen")}><Maximize2 className="icon" />Open full screen</button>
        )}
      </div>
      {note && <div className="mcp-note" role="status">{note}</div>}
      {showBoard && (
        <BoardView board={board} whole={whole || mode === "fullscreen"} error={moveError} onOpenCard={openCard} onMove={move} />
      )}
      {view === "card" && card && (
        <CardView card={card} columns={names} loadColumns={loadColumns} write={write} openLink={openLink} />
      )}
      {view === "chief" && chief && (
        <ChiefView chief={chief} ask={ask} openLink={openLink} />
      )}
    </div>
  );
}
