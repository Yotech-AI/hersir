// GA-343, GA-345: the board as show and view_read give it (views.ts, src-tauri/src/tools/apps.rs), and what the app's
// Board (src/components/Board.tsx) draws from it. A column is known by its place, a card by its identifier.
import { generateNKeysBetween } from "fractional-indexing";
import type { Task, WorkflowState } from "../types";
import type { BoardData } from "./views";

/** A column's kind (get_workflow's) → the app's category, which the Board folds and colours by. */
const CATEGORY: Record<string, string> = { waiting: "ready", work: "in_progress" };

/** What the Board writes before "is working" on a card an agent works on now: the data doesn't name the agent. */
export const WORKING = "An agent";

/** A column's id: its place on the board. */
const columnId = (i: number) => `column-${i}`;

/** The Board's columns: board order is their order here. */
export function statesOf(board: BoardData): WorkflowState[] {
  return board.columns.map((c, i) => ({
    id: columnId(i), name: c.name, category: CATEGORY[c.kind] ?? c.kind, wipLimit: null, auto: false, sortKey: String(i).padStart(4, "0"),
  }));
}

/** The Board's cards: what the board shows of each, the rest empty. Their order keys are real ones (fractional-indexing's,
 *  in board order), as the Board makes a dropped card's key from its neighbours' (GA-349: a card is dragged there). */
export function tasksOf(board: BoardData): Task[] {
  return board.columns.flatMap((c, i) => {
    const keys = generateNKeysBetween(null, null, c.cards.length);
    return c.cards.map((card, at) => ({
      id: card.identifier, identifier: card.identifier, title: card.title, descriptionMd: "", projectName: card.project,
      stateId: columnId(i), stateName: c.name, stateCategory: CATEGORY[c.kind] ?? c.kind,
      priority: card.priority ?? 0, assigneeName: card.assignee ?? null, assigneeKind: null,
      labels: (card.labels ?? []).map((l) => ({ id: l.name, name: l.name, color: l.color ?? null })),
      hold: card.hold ?? null, withLead: false, byHand: false, waitsFor: [],
      bounceCount: 0, failCount: 0, sortKey: keys[at], testing: false, createdAt: 0, updatedAt: 0, screenshots: null,
    }));
  });
}

/** Card id → who works on it now, for the Board's "… is working". */
export function workingOf(board: BoardData): Map<string, string> {
  return new Map(board.columns.flatMap((c) => c.cards.filter((card) => card.running).map((card) => [card.identifier, WORKING] as const)));
}

/** The board's name for its head: the project's, else the team's. */
export function titleOf(board: BoardData): string {
  return board.project ? `${board.project.name} (${board.project.key})` : board.team || "Board";
}

/** The board in a view's data; null when it isn't one. */
export function boardOf(data: unknown): BoardData | null {
  const b = data as Partial<BoardData> | null | undefined;
  if (!b || !Array.isArray(b.columns)) return null;
  return { team: String(b.team ?? ""), project: b.project ?? null, columns: b.columns };
}
