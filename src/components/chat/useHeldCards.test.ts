// GA-312 QA: of the cards a chat's answers put on the waiting-for-OK hold, useHeldCards keeps only those on that hold now,
// and asks again after each change to a card, so one whose hold went (OK, Clear hold on the card, a move by hand, another
// hold) leaves the list without a reload. Cards on other holds (a question, blocked) never count. Called as a function with
// stand-ins for React's useState and useEffect; the api and the window's focus are stand-ins too.
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Task } from "../../types";

const r = vi.hoisted(() => ({ state: undefined as unknown, effects: [] as (() => void | (() => void))[] }));
vi.mock("react", async (orig) => {
  const R = await orig<typeof import("react")>();
  const useState = (init: unknown) => {
    if (r.state === undefined) r.state = init;
    return [r.state, (v: unknown) => { r.state = typeof v === "function" ? (v as (s: unknown) => unknown)(r.state) : v; }];
  };
  const useEffect = (effect: () => void | (() => void)) => { r.effects.push(effect); };
  return { ...R, useState, useEffect, default: { ...R, useState, useEffect } };
});
const api = vi.hoisted(() => ({ cards: new Map<string, unknown>(), asked: [] as string[], listeners: [] as ((table: string) => void)[], unlistened: 0 }));
vi.mock("../../api", () => ({
  getTask: async (id: string) => { api.asked.push(id); const t = api.cards.get(id); if (!t) throw new Error("No such task"); return t; },
  onRowsChanged: async (cb: (table: string) => void) => { api.listeners.push(cb); return () => { api.unlistened++; }; },
}));
const win = vi.hoisted(() => ({ hidden: false, missed: 0, stopped: 0 }));
vi.mock("../../lib/windowFocus", () => ({
  whenShown: (fn: () => void) => ({ run: () => { if (win.hidden) win.missed++; else fn(); }, stop: () => { win.stopped++; } }),
}));

const { useHeldCards } = await import("./useHeldCards");

const card = (n: number, o: Partial<Task> = {}): Task => ({
  id: `t${n}`, identifier: `DU-${n}`, title: `Card ${n}`, descriptionMd: "", stateId: "todo", stateName: "To do", stateCategory: "unstarted",
  priority: 0, labels: [], hold: "needs_decision", holdKind: "waiting_for_ok", bounceCount: 0, failCount: 0, sortKey: "a", ...o,
} as Task);
const settle = () => new Promise((ok) => setTimeout(ok, 0));
/** Renders the hook once and runs its effect; what it shows after the first answer came. */
async function mount(ids: string[]) {
  const shown = () => [...(useHeldCards(ids) as ReadonlyMap<string, Task>).keys()];
  shown();
  const cleanup = r.effects.at(-1)!();
  await settle();
  return { shown, cleanup: cleanup as () => void };
}
const tasksChanged = async (table = "tasks") => { api.listeners.forEach((l) => l(table)); await settle(); };

beforeEach(() => {
  r.state = undefined; r.effects = [];
  api.cards = new Map(); api.asked = []; api.listeners = []; api.unlistened = 0;
  win.hidden = false; win.missed = 0; win.stopped = 0;
});

describe("useHeldCards", () => {
  it("keeps only the cards on the waiting-for-OK hold: not other holds, not off hold, not archived or gone", async () => {
    for (const t of [card(37), card(38, { stateName: "In progress" }),
      card(39, { holdKind: null }),                                      // a question (needs_decision of no kind)
      card(40, { hold: "blocked", holdKind: null }),                     // blocked
      card(41, { hold: null, holdKind: null }),                          // off hold
      card(42, { hold: null, holdKind: "waiting_for_ok" }),              // no hold, whatever the kind says
      card(43, { archivedAt: 5 } as Partial<Task>)])                     // archived
      api.cards.set(t.id, t);
    const { shown } = await mount(["t37", "t38", "t39", "t40", "t41", "t42", "t43", "t44"]); // t44 is gone
    expect(shown()).toEqual(["t37", "t38"]);
    expect(api.asked).toEqual(["t37", "t38", "t39", "t40", "t41", "t42", "t43", "t44"]);
  });

  it("drops a card at once when its hold is taken off (OK, Clear hold) or it is moved by hand, without a reload", async () => {
    for (const n of [37, 38, 39]) api.cards.set(`t${n}`, card(n));
    const { shown } = await mount(["t37", "t38", "t39"]);
    expect(shown()).toEqual(["t37", "t38", "t39"]);
    // OK or Clear hold on DU-38: the backend says tasks changed
    api.cards.set("t38", card(38, { hold: null, holdKind: null }));
    await tasksChanged();
    expect(shown()).toEqual(["t37", "t39"]);
    // DU-37 moved to Backlog by hand: its hold comes off with the move (GA-261)
    api.cards.set("t37", card(37, { hold: null, holdKind: null, stateName: "Backlog" }));
    await tasksChanged();
    expect(shown()).toEqual(["t39"]);
    // DU-39 on another hold now: blocked
    api.cards.set("t39", card(39, { hold: "blocked", holdKind: null }));
    await tasksChanged();
    expect(shown()).toEqual([]);
  });

  it("shows a card's new column or title once it changes, and asks again only for changes to cards", async () => {
    api.cards.set("t37", card(37));
    const { shown } = await mount(["t37"]);
    const before = useHeldCards(["t37"]).get("t37");
    api.cards.set("t37", card(37, { stateName: "In progress" }));
    await tasksChanged();
    expect(useHeldCards(["t37"]).get("t37")?.stateName).toBe("In progress");
    expect(useHeldCards(["t37"]).get("t37")).not.toBe(before);
    const asked = api.asked.length;
    await tasksChanged("chat_messages");
    expect(api.asked.length).toBe(asked);
    expect(shown()).toEqual(["t37"]);
  });

  it("keeps the same list when nothing it shows changed, so the chat doesn't draw again", async () => {
    api.cards.set("t37", card(37));
    await mount(["t37"]);
    const first = r.state;
    api.cards.set("t37", card(37, { descriptionMd: "changed" }));
    await tasksChanged();
    expect(r.state).toBe(first);
  });

  it("GA-312 fix round: takes a card's new hold time (held again, e.g. by another chat), though nothing else it shows changed", async () => {
    api.cards.set("t37", card(37, { holdAt: 1_000 }));
    await mount(["t37"]);
    const first = r.state;
    api.cards.set("t37", card(37, { holdAt: 2_000 }));
    await tasksChanged();
    expect(r.state).not.toBe(first);
    expect(useHeldCards(["t37"]).get("t37")?.holdAt).toBe(2_000);
    // the same time again: the same list
    const second = r.state;
    api.cards.set("t37", card(37, { holdAt: 2_000, descriptionMd: "changed" }));
    await tasksChanged();
    expect(r.state).toBe(second);
  });

  it("while the window is hidden, asks once it shows again", async () => {
    api.cards.set("t37", card(37));
    await mount(["t37"]);
    win.hidden = true;
    const asked = api.asked.length;
    await tasksChanged();
    expect(api.asked.length).toBe(asked);
    expect(win.missed).toBe(1);
  });

  it("asks nothing without cards, and stops listening when the chat goes", async () => {
    const none = await mount([]);
    expect(none.shown()).toEqual([]);
    expect(api.asked).toEqual([]);
    expect(api.listeners).toEqual([]);
    api.cards.set("t37", card(37));
    const one = await mount(["t37"]);
    one.cleanup();
    expect(api.unlistened).toBe(1);
    expect(win.stopped).toBe(1);
    // an answer after that changes nothing
    const was = r.state;
    await tasksChanged();
    expect(r.state).toBe(was);
  });
});
