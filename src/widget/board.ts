// GA-343: the board as show_board and board_read give it in structuredContent (src-tauri/src/tools/apps.rs), and what
// the app's Board (src/components/Board.tsx) draws from it.
import type { Task, WorkflowState } from "../types";

export type BoardColumn = { id: string; name: string; category: string; wipLimit?: number | null; auto?: boolean; total: number };
export type BoardCard = {
  id: string; identifier: string; title: string; column: string; sortKey: string; priority: number;
  assignee?: string | null; assigneeKind?: string | null; labels: { name: string; color?: string | null }[];
  hold: boolean; withLead?: boolean; byHand?: boolean; waitsFor?: string[];
  /** The agent working on it now, by name. */
  working?: string | null;
};
export type BoardData = {
  view: "board"; team: string; project: { id: string; key: string; name: string } | null;
  columns: BoardColumn[]; cards: BoardCard[];
};

/** The board in a tool's structuredContent; null when it isn't one. */
export function boardOf(structured: unknown): BoardData | null {
  const b = structured as Partial<BoardData> | null | undefined;
  if (!b || b.view !== "board" || !Array.isArray(b.columns) || !Array.isArray(b.cards)) return null;
  return { view: "board", team: String(b.team ?? ""), project: b.project ?? null, columns: b.columns, cards: b.cards };
}

/** The Board's columns: board order is their order here. */
export function statesOf(board: BoardData): WorkflowState[] {
  return board.columns.map((c, i) => ({
    id: c.id, name: c.name, category: c.category, wipLimit: c.wipLimit ?? null, auto: c.auto ?? false,
    sortKey: String(i).padStart(4, "0"),
  }));
}

/** The Board's cards: what the board shows of each, the rest empty. */
export function tasksOf(board: BoardData): Task[] {
  const columns = new Map(board.columns.map((c) => [c.id, c]));
  return board.cards.map((c) => ({
    id: c.id, identifier: c.identifier, title: c.title, descriptionMd: "",
    stateId: c.column, stateName: columns.get(c.column)?.name ?? "", stateCategory: columns.get(c.column)?.category ?? "",
    priority: c.priority ?? 0, assigneeName: c.assignee ?? null, assigneeKind: c.assigneeKind ?? null,
    labels: (c.labels ?? []).map((l) => ({ id: l.name, name: l.name, color: l.color ?? null })),
    hold: c.hold ? "hold" : null, withLead: !!c.withLead, byHand: !!c.byHand, waitsFor: c.waitsFor ?? [],
    bounceCount: 0, failCount: 0, sortKey: c.sortKey, testing: false, createdAt: 0, updatedAt: 0, screenshots: null,
  }));
}

/** Card id → the agent working on it now. */
export function workingOf(board: BoardData): Map<string, string> {
  return new Map(board.cards.filter((c) => c.working).map((c) => [c.id, c.working as string]));
}

/** The board's name for its head: the project's, else the team's. */
export function titleOf(board: BoardData): string {
  return board.project ? `${board.project.name} (${board.project.key})` : board.team || "Board";
}
