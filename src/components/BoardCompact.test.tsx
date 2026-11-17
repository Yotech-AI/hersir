// GA-298 QA: the compact board of the Chat's Board tab (components/Board.tsx compact and marked; design/islands-chat → Board,
// Figma frame 3), rendered to HTML on the server as BoardArchive.test.tsx does: the columns as on the Tasks page with the
// same rails, each card compact (the id, the assignee's avatar, the title, On hold when held; no labels, priority or column
// note), the chat's cards marked (the class, the chat icon after the id, the label "GA-259, in this chat"), the others not.
// Its CSS (180 wide columns on
// side, 8 apart; the 1.5px accent border) is read from components.css. Dragging is the Tasks page's (the same Board and drop
// rules); the cage's UI test measures that board's drag.
import { describe, expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import type { Task, WorkflowState } from "../types";

const { Board } = await import("./Board");

const states: WorkflowState[] = [
  { id: "s-backlog", name: "Backlog", category: "backlog", sortKey: "a0" },
  { id: "s-todo", name: "To do", category: "ready", sortKey: "a1" },
  { id: "s-doing", name: "In progress", category: "in_progress", sortKey: "a2" },
  { id: "s-done", name: "Done", category: "done", sortKey: "a5" },
];
const task = (n: number, s: WorkflowState, more: Partial<Task> = {}): Task => ({
  id: `t${n}`, identifier: `GA-${n}`, projectId: "p1", title: `Card ${n}`, descriptionMd: "", stateId: s.id, stateName: s.name, stateCategory: s.category,
  priority: 2, labels: [{ id: "l1", name: "frontend", color: "blue" }], bounceCount: 0, failCount: 0, sortKey: `a${n}`, testing: true, createdAt: 0, updatedAt: 0,
  byHand: false, screenshots: null, ...more,
} as Task);
const tasks = [
  task(1, states[1]!, { assigneeName: "Frontend Agent", assigneeKind: "agent" }),
  task(2, states[1]!, { hold: "decision" }),
  task(3, states[2]!),
  task(4, states[0]!),
];
const card = (html: string, id: string) => {
  const at = html.indexOf(`data-card="${id}"`);
  if (at < 0) return "";
  const next = html.indexOf("data-card=", at + 1);
  return html.slice(html.lastIndexOf("<div", at), next < 0 ? undefined : next);
};
const draw = (compact = true) => renderToStaticMarkup(
  <Board tasks={tasks} states={states} onMove={() => {}} onOpen={() => {}} compact={compact} marked={new Set(["GA-1", "GA-3"])} />);

describe("the compact board", () => {
  it("has the board's columns as the Tasks page does, the compact class on it, and Backlog and Done as rails", () => {
    const html = draw();
    expect(html).toMatch(/<div class="board compact"/);
    expect([...html.matchAll(/data-col="([^"]+)"/g)].map((m) => m[1])).toEqual(["Backlog", "To do", "In progress", "Done"]);
    expect(card(html, "GA-4"), "a card in the Backlog rail isn't drawn").toBe("");
  });
  it("draws each card compact: the id, the assignee's avatar, the title; no labels, priority or column note", () => {
    const one = card(draw(), "GA-1");
    expect(one).toMatch(/^<div[^>]*class="card compact marked"/);
    expect(one).toContain('<span class="id">GA-1</span>');
    expect(one).toContain('<div class="title">Card 1</div>');
    expect(one).toMatch(/class="avatar[^"]*"/);
    expect(one).not.toContain("frontend");
    expect(draw()).not.toContain('class="col-note');
  });
  it("marks the chat's cards: the class, the chat icon after the id, and the label for screen readers", () => {
    const html = draw();
    const one = card(html, "GA-1");
    expect(one).toContain('aria-label="GA-1, in this chat"');
    expect(one).toMatch(/<span class="id">GA-1<\/span><svg[^>]*class="lucide lucide-messages-square icon in-chat" aria-hidden="true"/);
    const three = card(html, "GA-3");
    expect(three).toMatch(/class="card compact marked"/);
    const two = card(html, "GA-2");
    expect(two).toMatch(/class="card compact"/);
    expect(two).not.toContain("in this chat");
    expect(two).not.toContain("lucide-messages-square");
  });
  it("shows On hold on a held card", () => {
    expect(card(draw(), "GA-2")).toContain('<span class="badge needs">On hold</span>');
  });
  it("leaves the Tasks page's cards as they were without compact", () => {
    const html = draw(false);
    expect(html).toMatch(/<div class="board"/);
    expect(html).not.toContain("compact");
    expect(html).not.toContain("in this chat");
  });
});

describe("its CSS (components.css)", () => {
  const fs = (globalThis as unknown as { process: { getBuiltinModule: (m: string) => { readFileSync: (u: URL, e: string) => string } } }).process.getBuiltinModule("node:fs");
  const css = fs.readFileSync(new URL("../styles/components.css", import.meta.url), "utf8");
  const rule = (sel: string) => new RegExp(`(^|\\n)${sel.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\s*\\{([^}]*)\\}`).exec(css)?.[2] ?? "";
  it("has columns 180 wide, 8 apart, on side with radius 8 and padding 6", () => {
    expect(rule(".board.compact")).toContain("gap: 8px");
    expect(rule(".board.compact .col")).toMatch(/width: 180px.*padding: 6px.*border-radius: 8px.*background: var\(--side\)/);
  });
  it("marks the chat's cards with a 1.5px accent border, and the chat icon 12px in accent", () => {
    expect(rule(".card.compact.marked")).toContain("border: 1.5px solid var(--accent)");
    expect(rule(".card.compact .top .in-chat")).toMatch(/width: 12px; height: 12px; color: var\(--accent\)/);
  });
  it("keeps a compact card's title to 3 lines", () => {
    expect(rule(".card.compact .title")).toMatch(/-webkit-line-clamp: 3/);
  });
});
