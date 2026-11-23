// GA-343, GA-345, the stand-in host (development and QA only): a made-up board in the shape show and view_read give it
// (src/widget/views.ts, src-tauri/src/tools/apps.rs), which changes as Hersir's board would: a card moves on, an agent
// starts or stops. It keeps a change counter, as Hersir's change feed does, so it answers board_changes too.
import type { BoardCard, BoardData, Changes } from "../widget/views";

const COLUMNS = [
  { name: "Backlog", kind: "backlog" },
  { name: "To do", kind: "waiting" },
  { name: "In progress", kind: "work" },
  { name: "Testing", kind: "testing" },
  { name: "Review", kind: "review" },
  { name: "Done", kind: "done" },
];

const TITLES = [
  "Export the invoices as CSV", "Dark theme for the settings page", "Login with a magic link", "Retry failed webhooks",
  "Show the project's open cards on the client page", "Faster search in the archive", "Label colours in the board's filter",
  "Weekly summary email", "Keyboard shortcut for a new card", "Drag cards between teams", "Fix the date picker in Safari",
  "Read-only share link for a board", "Import cards from a CSV file", "Undo for archiving a card",
];

const AGENTS = ["Backend Agent", "Frontend Agent", "QA Agent"];

/** The made-up project: the one `show {view: "board", id: "GA"}` asks for. */
export const PROJECT = { key: "GA", name: "Hersir" };

/** A card, with its column (by place), its place in it and the counter of its last change. */
type Card = BoardCard & { column: number; order: number; changed: number };

/** The made-up board, step 0, and every step after it one change more. */
export class FakeBoard {
  private cards: Card[];
  private step = 0;
  /** Where the counter starts: a Hersir that has been used a while is far from 0. */
  private readonly base = 1000;

  constructor() {
    const spread = [0, 0, 1, 1, 1, 2, 2, 3, 4, 4, 5, 5, 5, 1];
    this.cards = TITLES.map((title, i) => ({
      identifier: `GA-${101 + i}`, title, project: PROJECT.name, column: spread[i], order: i, changed: this.base,
      priority: i % 4, assignee: i % 3 === 0 ? null : AGENTS[i % AGENTS.length],
      labels: i % 2 ? [{ name: "frontend", color: "#6f97ff" }] : [{ name: "backend", color: "#3fb8a0" }],
      hold: i === 8 ? "needs_decision" : null, running: spread[i] === 2, needs_you: false,
    }));
  }

  /** How many changes so far. */
  get changes(): number {
    return this.step;
  }

  /** board_changes' counter now. */
  get seq(): number {
    return this.base + this.step;
  }

  /** One change: the next card in To do, In progress, Testing or Review moves one column on (Done wraps back to To do). */
  change() {
    this.step++;
    const movable = this.cards.filter((c) => c.column >= 1);
    const card = movable[this.step % movable.length];
    card.column = card.column >= COLUMNS.length - 1 ? 1 : card.column + 1;
    card.running = card.column === 2 || card.column === 3;
    card.order = 100 + this.step;
    card.changed = this.seq;
  }

  /** The board as view_read gives it: the project's, or (null) every project on the board. */
  data(project: string | null): BoardData {
    return {
      team: "My team",
      project: project === null ? null : { ...PROJECT },
      columns: COLUMNS.map((c, i) => {
        const on = this.cards.filter((x) => x.column === i).sort((a, b) => a.order - b.order);
        return {
          ...c, total: on.length,
          cards: on.map(({ column: _c, order: _o, changed: _x, ...card }) => ({
            ...card, labels: card.labels.map((l) => ({ ...l })),
            needs_you: card.hold !== null || COLUMNS[i].kind === "review",
          })),
        };
      }),
    };
  }

  /** board_changes {since}: the counter, and the cards changed since, the one changed last first. */
  since(since: number | undefined): Changes {
    if (since === undefined) return { seq: this.seq, cards: [], board: false, reload: false };
    if (since > this.seq) return { seq: this.seq, cards: [], board: false, reload: true };
    const cards = this.cards.filter((c) => c.changed > since).sort((a, b) => b.changed - a.changed).map((c) => c.identifier);
    return { seq: this.seq, cards, board: false, reload: false };
  }
}

/** The one sentence show and view_read give in `content`, as Hersir writes it for the board. */
export function sentence(b: BoardData): string {
  const n = b.columns.reduce((s, c) => s + c.total, 0);
  const held = b.columns.reduce((s, c) => s + c.cards.filter((x) => x.hold !== null).length, 0);
  const what = b.project ? `the board of ${b.project.name} (${b.project.key})` : `${b.team}'s board`;
  const cards = n === 0 ? "no cards yet" : n === 1 ? "1 card" : `${n} cards`;
  return `The user sees ${what} from Hersir in this chat: ${cards} in ${b.columns.length} columns${held ? `, ${held} on hold` : ""}.`;
}
