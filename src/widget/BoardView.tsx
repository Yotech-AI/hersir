// GA-343, GA-345, GA-349: the board in Claude Desktop's chat (App.tsx). Inline, its summary: each column with its count,
// the cards that need you (gold) and the cards an agent works on now (teal). After Open board (full screen when the host
// gives it), the whole board drawn with the app's own Board: a card dragged to another column moves there (move_task),
// and a click on a card opens it in this view.
import { useMemo } from "react";
import { Board } from "../components/Board";
import { StatusIcon } from "../components/StatusIcon";
import { holdName } from "../lib/holds";
import { statesOf, tasksOf, workingOf } from "./board";
import type { BoardCard, BoardData } from "./views";

/** The model's tool a dragged card moves with (to the bottom of its new column). */
export const MOVE_TASK = "move_task";

/** At most this many cards in each list of the summary; the rest are counted. */
const LIST_MAX = 8;

/** A column's kind → the app's category, as board.ts gives the Board. */
const CATEGORY: Record<string, string> = { waiting: "ready", work: "in_progress" };

/** Why a card needs you: its hold, or the column that waits for you (Review, Deploy). */
function whyOf(card: BoardCard, column: string): string {
  return card.hold ? `On hold: ${holdName(card.hold).toLowerCase()}` : `In ${column}`;
}

/** A list of the summary: the cards with what they say, each a button that opens it. */
function CardList({ cards, kind, empty, onOpen }: {
  cards: { card: BoardCard; column: string }[]; kind: "needs" | "live"; empty: string; onOpen: (identifier: string) => void;
}) {
  if (cards.length === 0) return <p className="mcp-empty faint">{empty}</p>;
  return (
    <ul className={`mcp-list ${kind}`}>
      {cards.slice(0, LIST_MAX).map(({ card, column }) => (
        <li key={card.identifier}>
          <button className="mcp-row" onClick={() => onOpen(card.identifier)} title={`Open ${card.identifier}`}>
            {kind === "live" ? <span className="pulse" aria-hidden /> : <span className="mcp-dot" aria-hidden />}
            <span className="id">{card.identifier}</span>
            <span className="mcp-row-title">{card.title}</span>
            <span className="mcp-row-why">{kind === "needs" ? whyOf(card, column) : `${column}${card.assignee ? ` · ${card.assignee}` : ""}`}</span>
          </button>
        </li>
      ))}
      {cards.length > LIST_MAX && <li className="faint mcp-more">And {cards.length - LIST_MAX} more: Open board shows them all.</li>}
    </ul>
  );
}

/** The board's summary, inline in the chat. */
function Summary({ board, onOpen }: { board: BoardData; onOpen: (identifier: string) => void }) {
  const all = board.columns.flatMap((c) => c.cards.map((card) => ({ card, column: c.name })));
  const needs = all.filter((x) => x.card.needs_you);
  const working = all.filter((x) => x.card.running);
  return (
    <div className="mcp-summary">
      <ul className="mcp-columns" aria-label="Columns">
        {board.columns.map((c) => (
          <li key={c.name} className="mcp-column">
            <StatusIcon category={CATEGORY[c.kind] ?? c.kind} title={c.name} />
            <span className="name">{c.name}</span>
            <span className="n">{c.total}</span>
          </li>
        ))}
      </ul>
      <section className="mcp-section">
        <h3 className="needs">Needs you <span className="n">{needs.length}</span></h3>
        <CardList cards={needs} kind="needs" empty="Nothing needs you now." onOpen={onOpen} />
      </section>
      <section className="mcp-section">
        <h3 className="live">Agents working <span className="n">{working.length}</span></h3>
        <CardList cards={working} kind="live" empty="No agent is working now." onOpen={onOpen} />
      </section>
    </div>
  );
}

export function BoardView({ board, whole, error, onOpenCard, onMove }: {
  board: BoardData;
  /** The whole board (Open board, full screen) instead of the summary. */
  whole: boolean;
  /** Why the last drag didn't move its card. */
  error: string | null;
  onOpenCard: (identifier: string) => void;
  /** A card dropped in another column, by the column's name. */
  onMove: (identifier: string, column: string) => void;
}) {
  const tasks = useMemo(() => tasksOf(board), [board]);
  const states = useMemo(() => statesOf(board), [board]);
  const working = useMemo(() => workingOf(board), [board]);
  if (!whole) return <Summary board={board} onOpen={onOpenCard} />;
  const drop = (identifier: string, stateId: string) => {
    const to = states.find((s) => s.id === stateId);
    const from = tasks.find((t) => t.id === identifier);
    // move_task puts a card at the bottom of another column: an order within a column isn't kept
    if (to && from && from.stateId !== to.id) onMove(identifier, to.name);
  };
  return (
    <div className="mcp-whole">
      {error && <div className="mcp-error" role="alert">{error}</div>}
      {states.length > 0 && <Board tasks={tasks} states={states} working={working} onMove={drop} onOpen={onOpenCard} />}
    </div>
  );
}
