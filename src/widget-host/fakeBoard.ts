// GA-343, the stand-in host (development and QA only): a made-up board in the shape show_board and board_read give it
// (src-tauri/src/tools/apps.rs), which changes as Hersir's board would: a card moves on, an agent starts or stops.
import type { BoardCard, BoardData } from "../widget/board";

const COLUMNS = [
  { id: "s-backlog", name: "Backlog", category: "backlog", auto: false },
  { id: "s-todo", name: "To do", category: "todo", auto: true },
  { id: "s-progress", name: "In progress", category: "in_progress", auto: true },
  { id: "s-testing", name: "Testing", category: "testing", auto: true },
  { id: "s-review", name: "Review", category: "review", auto: false },
  { id: "s-done", name: "Done", category: "done", auto: false },
];

const TITLES = [
  "Export the invoices as CSV", "Dark theme for the settings page", "Login with a magic link", "Retry failed webhooks",
  "Show the project's open cards on the client page", "Faster search in the archive", "Label colours in the board's filter",
  "Weekly summary email", "Keyboard shortcut for a new card", "Drag cards between teams", "Fix the date picker in Safari",
  "Read-only share link for a board", "Import cards from a CSV file", "Undo for archiving a card",
];

const AGENTS = ["Backend Agent", "Frontend Agent", "QA Agent"];

/** The made-up board, step 0, and every step after it one change more. */
export class FakeBoard {
  private cards: BoardCard[];
  private step = 0;

  constructor() {
    const spread = ["s-backlog", "s-backlog", "s-todo", "s-todo", "s-todo", "s-progress", "s-progress", "s-testing", "s-review",
      "s-review", "s-done", "s-done", "s-done", "s-todo"];
    this.cards = TITLES.map((title, i) => ({
      id: `t-${i + 1}`, identifier: `GA-${101 + i}`, title, column: spread[i], sortKey: `a${String(i).padStart(2, "0")}`,
      priority: i % 4, assignee: i % 3 === 0 ? null : AGENTS[i % AGENTS.length], assigneeKind: i % 3 === 0 ? null : "agent",
      labels: i % 2 ? [{ name: "frontend", color: "#6f97ff" }] : [{ name: "backend", color: "#3fb8a0" }],
      hold: i === 8, withLead: false, byHand: false, waitsFor: i === 13 ? ["GA-106"] : [],
      working: spread[i] === "s-progress" ? AGENTS[i % AGENTS.length] : null,
    }));
  }

  /** How many changes so far. */
  get changes(): number {
    return this.step;
  }

  /** One change: the next card in To do, In progress or Testing moves one column on (Done wraps back to To do). */
  change() {
    this.step++;
    const order = ["s-todo", "s-progress", "s-testing", "s-review", "s-done"];
    const movable = this.cards.filter((c) => order.includes(c.column));
    const card = movable[this.step % movable.length];
    const next = order[(order.indexOf(card.column) + 1) % order.length];
    card.column = next;
    card.working = next === "s-progress" || next === "s-testing" ? (card.assignee ?? AGENTS[0]) : null;
    card.sortKey = `b${String(this.step).padStart(4, "0")}`;
  }

  /** The board as the tools give it, the project's or (none) the team's. */
  data(project: string | null): BoardData {
    return {
      view: "board", team: "My team",
      project: project === null ? null : { id: "p-ga", key: "GA", name: "Giz AI" },
      columns: COLUMNS.map((c) => ({ ...c, wipLimit: null, total: this.cards.filter((x) => x.column === c.id).length })),
      cards: COLUMNS.flatMap((c) => this.cards.filter((x) => x.column === c.id).sort((a, b) => (a.sortKey < b.sortKey ? -1 : 1)))
        .map((c) => ({ ...c, labels: c.labels.map((l) => ({ ...l })) })),
    };
  }
}

/** The one sentence the tools give in `content`, as Hersir writes it. */
export function sentence(b: BoardData): string {
  const n = b.columns.reduce((s, c) => s + c.total, 0);
  const held = b.cards.filter((c) => c.hold).length;
  const what = b.project ? `the board of ${b.project.name} (${b.project.key})` : `${b.team}'s board`;
  const cards = n === 0 ? "no cards yet" : n === 1 ? "1 card" : `${n} cards`;
  return `The user sees ${what} from Hersir in this chat: ${cards} in ${b.columns.length} columns${held ? `, ${held} on hold` : ""}.`;
}
