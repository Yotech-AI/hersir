// GA-349, the stand-in host (development and QA only): made-up cards in the shape view_read gives a card
// (src/widget/views.ts's CardData, src-tauri/src/tools/apps.rs), for the cards of the made-up board (fakeBoard.ts). Each
// has a description and acceptance criteria in Markdown, comments and runs; a card an agent works on has a live output
// that grows every second, cut to its last 200 lines as Hersir cuts it. It answers the card's writes as Hersir would:
// comment_on_task, start_agent_run and stop_agent_run (move_task is the board's).
import { CUT, type CardComment, type CardData, type CardRun } from "../widget/views";
import type { FakeBoard } from "./fakeBoard";

/** The last this many comments, runs and output lines, as Hersir gives them. */
const COMMENTS = 20;
const RUNS = 5;
const OUTPUT_LINES = 200;

const AGENT = "Frontend Agent";

/** The made-up output of a run: what an agent's log shows, one more step every half second. */
const STEPS = [
  "Read src/pages/TasksPage.tsx", "Grep \"useShortcut\" src", "Edit src/components/Board.tsx", "Bash npx tsc -b",
  "I'll add the shortcut to the board's key handler and show it in the help.", "Edit src/lib/keys.ts", "Bash git add -A && git commit",
  "Read docs/research/claude-desktop-mcp.md",
];

/** An ISO time `ago` milliseconds before now, as Hersir writes times (UTC, no milliseconds). */
const iso = (ms: number) => new Date(ms).toISOString().replace(/\.\d{3}Z$/, "Z");

/** A card's number: GA-109 → 109. */
const numberOf = (identifier: string) => Number(/-(\d+)$/.exec(identifier)?.[1] ?? 0);

/** What isn't on the board's face: comments, runs, and since when an agent works on it. */
type Extra = { comments: CardComment[]; runs: CardRun[]; liveSince: number | null };

export class FakeCards {
  private extra = new Map<string, Extra>();

  constructor(private board: FakeBoard, private now: () => number = () => Date.now()) {}

  /** The card as view_read gives it; null when there is none. */
  data(identifier: string): CardData | null {
    const found = this.board.find(identifier);
    if (!found) return null;
    const { card, column, kind } = found;
    const x = this.sync(card.identifier, card.running);
    const n = numberOf(card.identifier);
    return {
      identifier: card.identifier, title: card.title, project: card.project, column, assignee: card.assignee, labels: card.labels.map((l) => ({ ...l })),
      priority: card.priority, hold: card.hold,
      hold_reason: card.hold ? "Which shortcut should a new card get: **N** or **C**? Both are free on the board page, and the Chat page uses C for its composer." : null,
      description_md: n === 104 ? long(card.title) : description(card.title),
      acceptance_md: n % 5 === 0 ? null : "- [ ] The button shows on the page\n- [ ] A click does what its label says\n- [x] Texts in sentence case",
      pr_url: kind === "review" || kind === "done" || kind === "testing" ? `https://github.com/example/hersir/pull/${n + 200}` : null,
      comments: x.comments.slice(-COMMENTS).map((c) => ({ ...c })),
      runs: x.runs.slice(0, RUNS).map((r) => ({ ...r })),
      live: card.running ? { agent: card.assignee ?? AGENT, output: this.output(x.liveSince ?? this.now()) } : null,
    };
  }

  /** comment_on_task: a comment as you. Why it can't, or null. */
  comment(identifier: string, body: string, author = "You"): string | null {
    const found = this.board.find(identifier);
    if (!found) return `There is no card ${identifier} in Hersir.`;
    if (!body.trim()) return "missing \"body_md\": the comment (Markdown)";
    this.of(found.card.identifier).comments.push({ author, at: iso(this.now()), body_md: body });
    this.board.touch(found.card.identifier);
    return null;
  }

  /** start_agent_run: the assigned agent (else the Frontend Agent) starts now. Why it can't, or null. */
  start(identifier: string, agent?: string): string | null {
    const found = this.board.find(identifier);
    if (!found) return `There is no card ${identifier} in Hersir.`;
    if (found.card.running) return `${found.card.identifier} already has an agent working on it: stop it first.`;
    if (found.kind === "done") return `${found.card.identifier} is in ${found.column}: there is nothing left to run.`;
    const x = this.of(found.card.identifier);
    x.liveSince = this.now();
    x.runs.unshift({ agent: agent || found.card.assignee || AGENT, status: "running", outcome: null, started_at: iso(this.now()), ended_at: null, summary: null });
    this.board.setRunning(found.card.identifier, true);
    return null;
  }

  /** stop_agent_run: the agent working on it stops. Why it can't, or null. */
  stop(identifier: string): string | null {
    const found = this.board.find(identifier);
    if (!found) return `There is no card ${identifier} in Hersir.`;
    if (!found.card.running) return `No agent is working on ${found.card.identifier} right now.`;
    const x = this.sync(found.card.identifier, true);
    const run = x.runs[0];
    if (run) Object.assign(run, { status: "stopped", outcome: null, ended_at: iso(this.now()), summary: "Stopped by you." });
    x.liveSince = null;
    this.board.setRunning(found.card.identifier, false);
    return null;
  }

  private of(identifier: string): Extra {
    let x = this.extra.get(identifier);
    if (!x) {
      const n = numberOf(identifier), t = this.now();
      x = {
        comments: n % 3 === 0 ? [] : [
          { author: "Jefsev", at: iso(t - 26 * 3600_000), body_md: "Keep it small: one button, and the shortcut in the help." },
          { author: AGENT, at: iso(t - 3 * 3600_000), body_md: `Done on branch \`hersir/ga-${n}\`, pushed.\n\nQA should test:\n1. Open the board and press the shortcut.\n2. See [the spec](https://example.com/spec).` },
        ],
        runs: n % 4 === 0 ? [] : [
          { agent: AGENT, status: "done", outcome: "ready_for_testing", started_at: iso(t - 4 * 3600_000), ended_at: iso(t - 3 * 3600_000),
            summary: "Added the button and its shortcut. **Pushed**; the tests are QA's." },
        ],
        liveSince: null,
      };
      this.extra.set(identifier, x);
    }
    return x;
  }

  /** The card's runs follow the board: an agent the board started (fakeBoard's change) has a run; one that ended there
   *  ends its run. */
  private sync(identifier: string, running: boolean): Extra {
    const x = this.of(identifier);
    const last = x.runs[0];
    if (running && (!last || last.status !== "running")) {
      x.liveSince = this.now() - 20_000;
      x.runs.unshift({ agent: AGENT, status: "running", outcome: null, started_at: iso(x.liveSince), ended_at: null, summary: null });
    } else if (!running && last?.status === "running") {
      Object.assign(last, { status: "done", outcome: "ready_for_testing", ended_at: iso(this.now()), summary: "Done: the card moved on." });
      x.liveSince = null;
    }
    return x;
  }

  /** The live output: three lines to start, one more every half second; past 200 lines, the last 200 and CUT. */
  private output(since: number): string {
    const count = 3 + Math.max(0, Math.floor((this.now() - since) / 500));
    const lines = Array.from({ length: count }, (_, i) => (i === 0 ? "Started · claude-opus-5-5" : `${STEPS[(i - 1) % STEPS.length]}`));
    return count > OUTPUT_LINES ? `${lines.slice(-OUTPUT_LINES).join("\n")}\n${CUT}` : lines.join("\n");
  }
}

function description(title: string): string {
  return `## Why\n${title}: people ask for it every week.\n\n## What\n- A button on the board page\n- The same in the card's menu\n- \`Ctrl+N\` as its shortcut\n\nSee [the design](https://example.com/design) and **keep the texts short**.`;
}

/** A long description, for the card view's long text. */
function long(title: string): string {
  const para = "The board page lists every card of the project in its columns. This part of the text is here to make the description long, so the card view shows how a long text looks in Claude's chat, inline and in full screen.";
  return `## ${title}\n\n${Array.from({ length: 12 }, (_, i) => `### Part ${i + 1}\n${para}\n\n- One point\n- Another point with a [link](https://example.com/${i + 1})`).join("\n\n")}`;
}
