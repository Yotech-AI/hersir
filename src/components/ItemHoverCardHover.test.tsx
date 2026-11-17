// GA-287 QA: when the @ chip's hover card shows and goes (useItemHover in src/components/ItemHoverCard.tsx), against GA-284's
// README (design/islands/README.md → The chip's hover card → When it shows): the pointer rests 350 ms; leaving the chip and
// the card hides it after 250 ms; the keyboard focus shows it, Escape and blur hide it; one card at a time; a click opens
// the item. An item is read only when its card is about to show, and kept for a minute.
// GA-297 QA: on the Chat page (its BesideChat context) a click on a card chip opens the card beside the chat, Ctrl+click
// (Cmd+click on macOS) its page; other kinds of chip, and chips anywhere else, open the page as before.
// There is no DOM here, so a small hook runtime stands in for React (as in HandFlow.test): a chip's hook keeps its state by
// its place, a state change renders it again, effects run after each render. The backend is faked and counts its reads;
// timers are vitest's.
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ReactElement } from "react";

type Inst = { slots: unknown[]; i: number; render: () => void; effects: (() => void)[]; cleanups: (() => void)[] };
const h = vi.hoisted(() => ({
  cur: null as null | Inst,
  ids: 0,
  /** How long the fake backend takes to answer, in ms (0: at once). */
  delay: 0,
  reads: [] as string[],
  /** The contexts the chip is rendered in (the Chat page gives BesideChat); a context left out has its default. */
  contexts: new Map<unknown, unknown>(),
}));

vi.mock("react", async (orig) => {
  const R = await orig<typeof import("react")>();
  const slot = <T,>(make: () => T): T => {
    const c = h.cur!;
    if (c.i === c.slots.length) c.slots.push(make());
    return c.slots[c.i++] as T;
  };
  const changed = (deps?: unknown[], prev?: unknown[]) => !deps || !prev || deps.length !== prev.length || deps.some((d, i) => !Object.is(d, prev[i]));
  const useState = ((init: unknown) => {
    if (!h.cur) return R.useState(init);
    const inst = h.cur;
    const box = slot(() => {
      const b = { v: typeof init === "function" ? (init as () => unknown)() : init, set: (x: unknown) => {
        const v = typeof x === "function" ? (x as (p: unknown) => unknown)(b.v) : x;
        if (!Object.is(v, b.v)) { b.v = v; inst.render(); }
      } };
      return b;
    });
    return [box.v, box.set];
  }) as typeof R.useState;
  const useRef = ((init: unknown) => (h.cur ? slot(() => ({ current: init })) : R.useRef(init))) as typeof R.useRef;
  const useCallback = ((fn: unknown, deps: unknown[]) => {
    if (!h.cur) return R.useCallback(fn as () => void, deps);
    const s = slot(() => ({ fn, deps }));
    if (changed(deps, s.deps)) { s.fn = fn; s.deps = deps; }
    return s.fn;
  }) as typeof R.useCallback;
  const useId = (() => (h.cur ? slot(() => `:hc${++h.ids}:`) : R.useId())) as typeof R.useId;
  const effect = (real: typeof R.useEffect) => ((fn: () => void | (() => void), deps?: unknown[]) => {
    if (!h.cur) return real(fn, deps);
    const inst = h.cur;
    const s = slot(() => ({ deps: undefined as unknown[] | undefined, first: true, cleanup: undefined as void | (() => void), at: -1 }));
    if (s.first || changed(deps, s.deps)) {
      s.first = false;
      s.deps = deps;
      inst.effects.push(() => {
        if (s.cleanup) s.cleanup();
        s.cleanup = fn();
        if (s.at < 0) { s.at = inst.cleanups.length; inst.cleanups.push(() => s.cleanup?.()); }
      });
    }
  }) as typeof R.useEffect;
  const useEffect = effect(R.useEffect);
  const useLayoutEffect = effect(R.useLayoutEffect);
  const useContext = ((ctx: { _currentValue?: unknown }) => {
    if (!h.cur) return R.useContext(ctx as never);
    return h.contexts.has(ctx) ? h.contexts.get(ctx) : ctx._currentValue;
  }) as typeof R.useContext;
  const hooks = { useState, useRef, useCallback, useId, useEffect, useLayoutEffect, useContext };
  return { ...R, ...hooks, default: { ...R, ...hooks } };
});

const after = <T,>(v: T) => (h.delay ? new Promise<T>((r) => setTimeout(() => r(v), h.delay)) : Promise.resolve(v));
vi.mock("../api", () => ({
  itemId: (kind: string, key: string) => { h.reads.push(`itemId ${kind} ${key}`); return key === "GA-999" ? after("").then(() => { throw new Error(`No task ${key}`); }) : after(`id-${key}`); },
  getTask: (id: string) => { h.reads.push(`getTask ${id}`); return after({ id, identifier: id.slice(3), title: "Fix the login", stateName: "In progress", stateCategory: "in_progress" }); },
  getProject: (id: string) => { h.reads.push(`getProject ${id}`); return after({ id, key: "GA", name: "Giz AI", status: "active" }); },
  getClient: (id: string) => { h.reads.push(`getClient ${id}`); return after({ id, name: "Kade", status: "lead" }); },
  getAgent: (id: string) => { h.reads.push(`getAgent ${id}`); return after({ actorId: id, name: "Design Agent", roleKey: "design", status: "active" }); },
  getDoc: (id: string) => { h.reads.push(`getDoc ${id}`); return after({ id, title: "Release checklist", projectId: null }); },
  listUsers: () => { h.reads.push("listUsers"); return after([]); },
  liveRuns: () => { h.reads.push("liveRuns"); return after([]); },
}));
const opened = vi.hoisted(() => [] as [string, string][]);
vi.mock("../lib/openItem", () => ({ openItem: (kind: string, key: string) => { opened.push([kind, key]); return Promise.resolve(); } }));

const { useItemHover, SHOW_MS, HIDE_MS, WAIT_MS } = await import("./ItemHoverCard");
const { ItemChip } = await import("./MarkdownView");
const { forgetPreviews } = await import("../lib/itemPreview");
const { BesideChat } = await import("../lib/besideChat");
import type { ItemKind } from "../lib/itemLinks";
import type { ItemPreview } from "../lib/itemPreview";

type Hover = ReturnType<typeof useItemHover>;
type CardProps = { id: string; kind: ItemKind; text: string; item: ItemPreview | undefined; onEnter: () => void; onLeave: () => void };

/** The document's keydown listeners (the card's Escape). */
const keys = new Set<(e: KeyboardEvent) => void>();
let mounted: Inst[] = [];

/** A chip on the screen: its hook mounted in the runtime, its element a stand-in with its text. */
function chip(kind: ItemKind, key: string, text = `${key} text`) {
  let out!: Hover;
  const el = { textContent: text, getClientRects: () => [{ left: 10, top: 10, bottom: 30, right: 90 }] };
  const inst: Inst = { slots: [], i: 0, effects: [], cleanups: [], render: () => {
    const prev = h.cur;
    h.cur = inst;
    inst.i = 0;
    try { out = useItemHover(kind, key); } finally { h.cur = prev; }
    (out.chip.ref as { current: unknown }).current = el;
    const run = inst.effects.splice(0);
    for (const f of run) f();
  } };
  inst.render();
  mounted.push(inst);
  const ev = { clientX: 12, clientY: 20 } as React.MouseEvent;
  return {
    get chip() { return out.chip; },
    get card() { return out.card as ReactElement<CardProps> | null; },
    enter: () => out.chip.onMouseEnter(ev),
    move: () => out.chip.onMouseMove(ev),
    leave: () => out.chip.onMouseLeave(),
    focus: () => out.chip.onFocus(),
    blur: () => out.chip.onBlur(),
    unmount: () => { for (const c of inst.cleanups) c(); mounted = mounted.filter((m) => m !== inst); },
  };
}
const press = (key: string) => {
  const e = { key, preventDefault: vi.fn() } as unknown as KeyboardEvent;
  for (const f of [...keys]) f(e);
  return e;
};
const wait = (ms: number) => vi.advanceTimersByTimeAsync(ms);

beforeEach(() => {
  vi.useFakeTimers();
  forgetPreviews();
  h.delay = 0;
  h.reads = [];
  opened.length = 0;
  h.contexts.clear();
  keys.clear();
  vi.stubGlobal("window", { setTimeout: (f: () => void, ms: number) => setTimeout(f, ms), clearTimeout: (id: number) => clearTimeout(id) });
  vi.stubGlobal("document", {
    addEventListener: (type: string, f: (e: KeyboardEvent) => void) => { if (type === "keydown") keys.add(f); },
    removeEventListener: (type: string, f: (e: KeyboardEvent) => void) => { if (type === "keydown") keys.delete(f); },
  });
});
afterEach(() => {
  for (const m of mounted) for (const c of m.cleanups) c();
  mounted = [];
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe("the README's timing", () => {
  it("350 ms to show, 250 ms to hide", () => {
    expect([SHOW_MS, HIDE_MS]).toEqual([350, 250]);
    expect(WAIT_MS).toBeLessThan(SHOW_MS);
  });
});

describe("resting the pointer on a chip", () => {
  it("chips on the screen read nothing until one is rested on", async () => {
    chip("task", "GA-1");
    chip("project", "GA");
    chip("agent", "a-1");
    await wait(5_000);
    expect(h.reads).toEqual([]);
  });

  it("shows nothing and reads nothing before 350 ms, then the card with the item", async () => {
    const c = chip("task", "GA-12", "GA-12 - Fix the login");
    c.enter();
    c.move();
    await wait(SHOW_MS - 1);
    expect(c.card).toBeNull();
    expect(h.reads).toEqual([]);
    await wait(1);
    expect(h.reads).toEqual(["itemId task GA-12", "getTask id-GA-12"]);
    expect(c.card?.props.item).toMatchObject({ kind: "task", task: { identifier: "GA-12" } });
    expect(c.card?.props.text).toBe("GA-12 - Fix the login");
    expect(c.card?.props.kind).toBe("task");
  });

  it("while it shows, the chip's aria-describedby points to the card; the chip never has a title", async () => {
    const c = chip("task", "GA-12");
    expect(c.chip["aria-describedby"]).toBeUndefined();
    expect(c.chip).not.toHaveProperty("title");
    c.enter();
    await wait(SHOW_MS);
    expect(c.card).not.toBeNull();
    expect(c.chip["aria-describedby"]).toBe(c.card!.props.id);
    expect(c.chip).not.toHaveProperty("title");
  });

  it("a slow read: the chip's text and kind first, then the item when it arrives", async () => {
    h.delay = 1_000;
    const c = chip("project", "GA", "Giz AI");
    c.enter();
    await wait(SHOW_MS + WAIT_MS - 1);
    expect(c.card).toBeNull();
    await wait(1);
    expect(c.card?.props.item).toBeUndefined();
    expect(c.card?.props.text).toBe("Giz AI");
    await wait(2_000);
    expect(c.card?.props.item).toMatchObject({ kind: "project", project: { name: "Giz AI" } });
  });

  it("leaving before 350 ms shows nothing and reads nothing", async () => {
    const c = chip("task", "GA-12");
    c.enter();
    await wait(200);
    c.leave();
    await wait(5_000);
    expect(c.card).toBeNull();
    expect(h.reads).toEqual([]);
  });

  it("a read that arrives after the pointer left shows no card", async () => {
    h.delay = 1_000;
    const c = chip("task", "GA-12");
    c.enter();
    await wait(SHOW_MS + 50);
    c.leave();
    await wait(5_000);
    expect(c.card).toBeNull();
  });

  it("a chip whose item doesn't exist shows Not found", async () => {
    const c = chip("task", "GA-999", "GA-999");
    c.enter();
    await wait(SHOW_MS);
    expect(c.card?.props.item).toEqual({ kind: "missing" });
    expect(c.card?.props.text).toBe("GA-999");
  });
});

describe("leaving", () => {
  it("leaving the chip hides the card after 250 ms", async () => {
    const c = chip("task", "GA-12");
    c.enter();
    await wait(SHOW_MS);
    c.leave();
    await wait(HIDE_MS - 1);
    expect(c.card).not.toBeNull();
    await wait(1);
    expect(c.card).toBeNull();
    expect(c.chip["aria-describedby"]).toBeUndefined();
  });

  it("moving onto the card keeps it; leaving the card hides it after 250 ms", async () => {
    const c = chip("task", "GA-12");
    c.enter();
    await wait(SHOW_MS);
    c.leave();
    await wait(120);
    c.card!.props.onEnter();
    await wait(10_000);
    expect(c.card).not.toBeNull();
    c.card!.props.onLeave();
    await wait(HIDE_MS);
    expect(c.card).toBeNull();
  });

  it("from the card back to the chip keeps it", async () => {
    const c = chip("task", "GA-12");
    c.enter();
    await wait(SHOW_MS);
    c.leave();
    await wait(100);
    c.card!.props.onEnter();
    c.card!.props.onLeave();
    await wait(100);
    c.enter();
    await wait(5_000);
    expect(c.card).not.toBeNull();
  });
});

describe("the keyboard", () => {
  it("tabbing to a chip shows its card after 350 ms; Escape hides it", async () => {
    const c = chip("agent", "a-1", "Design Agent");
    c.focus();
    await wait(SHOW_MS - 1);
    expect(c.card).toBeNull();
    await wait(1);
    expect(c.card?.props.item).toMatchObject({ kind: "agent", agent: { name: "Design Agent" } });
    expect(keys.size).toBe(1);
    const e = press("Escape");
    expect(c.card).toBeNull();
    expect(e.preventDefault).toHaveBeenCalled();
    expect(keys.size).toBe(0);
  });

  it("other keys leave it; leaving the chip by Tab (blur) hides it", async () => {
    const c = chip("client", "c-1");
    c.focus();
    await wait(SHOW_MS);
    press("ArrowDown");
    expect(c.card).not.toBeNull();
    c.blur();
    expect(c.card).toBeNull();
    expect(keys.size).toBe(0);
  });

  it("listens for Escape only while a card shows", async () => {
    chip("task", "GA-12");
    chip("doc", "d-1");
    await wait(1_000);
    expect(keys.size).toBe(0);
  });
});

describe("one card at a time", () => {
  it("a second chip's card hides the first", async () => {
    const a = chip("task", "GA-12");
    const b = chip("client", "c-1");
    a.enter();
    await wait(SHOW_MS);
    expect(a.card).not.toBeNull();
    a.leave();
    b.enter();
    await wait(SHOW_MS);
    expect(b.card).not.toBeNull();
    expect(a.card).toBeNull();
  });

  it("even when the first still has the focus", async () => {
    const a = chip("task", "GA-12");
    const b = chip("doc", "d-1");
    a.focus();
    await wait(SHOW_MS);
    b.enter();
    await wait(SHOW_MS);
    expect(a.card).toBeNull();
    expect(b.card).not.toBeNull();
  });
});

describe("kept for a minute", () => {
  it("resting on the same chip again within a minute reads nothing, and shows the item at once", async () => {
    const c = chip("task", "GA-12");
    c.enter();
    await wait(SHOW_MS);
    c.leave();
    await wait(HIDE_MS);
    const reads = h.reads.length;
    h.delay = 5_000;
    await wait(30_000);
    c.enter();
    await wait(SHOW_MS);
    expect(c.card?.props.item).toMatchObject({ kind: "task" });
    expect(h.reads.length).toBe(reads);
  });

  it("another chip of the same item within the minute reads nothing either", async () => {
    const a = chip("project", "GA");
    a.enter();
    await wait(SHOW_MS);
    a.leave();
    await wait(HIDE_MS);
    const b = chip("project", "GA");
    b.enter();
    await wait(SHOW_MS);
    expect(b.card?.props.item).toMatchObject({ kind: "project" });
    expect(h.reads).toEqual(["itemId project GA", "getProject id-GA"]);
  });

  it("after a minute it reads the item again", async () => {
    const c = chip("client", "c-1");
    c.enter();
    await wait(SHOW_MS);
    c.leave();
    await wait(61_000);
    c.enter();
    await wait(SHOW_MS);
    expect(h.reads).toEqual(["getClient c-1", "getClient c-1"]);
    expect(c.card?.props.item).toMatchObject({ kind: "client" });
  });
});

describe("a chip that goes away", () => {
  it("while the pointer rests on it reads nothing later", async () => {
    const c = chip("task", "GA-12");
    c.enter();
    await wait(100);
    c.unmount();
    await wait(5_000);
    expect(h.reads).toEqual([]);
  });
});

describe("a click on the chip", () => {
  /** ItemChip itself in the runtime: its <a> and its card. */
  function itemChip(kind: ItemKind, key: string) {
    let out!: ReactElement<{ children: [ReactElement<Record<string, unknown>>, ReactElement<CardProps> | null] }>;
    const inst: Inst = { slots: [], i: 0, effects: [], cleanups: [], render: () => {
      const prev = h.cur;
      h.cur = inst;
      inst.i = 0;
      try { out = ItemChip({ kind, itemKey: key, children: "GA-12 - Fix the login" }) as typeof out; } finally { h.cur = prev; }
      const a = out.props.children[0];
      (a.props.ref as { current: unknown }).current = { textContent: "GA-12 - Fix the login", getClientRects: () => [] };
      for (const f of inst.effects.splice(0)) f();
    } };
    inst.render();
    mounted.push(inst);
    return { get a() { return out.props.children[0]; }, get card() { return out.props.children[1]; } };
  }

  it("still opens the item's page, and hides its card", async () => {
    const c = itemChip("task", "GA-12");
    expect(c.a.type).toBe("a");
    expect(c.a.props.href).toBe("gizai:task/GA-12");
    expect(c.a.props.title).toBeUndefined();
    (c.a.props.onMouseEnter as (e: unknown) => void)({ clientX: 1, clientY: 1 });
    await wait(SHOW_MS);
    expect(c.card).not.toBeNull();
    const e = { preventDefault: vi.fn(), stopPropagation: vi.fn() };
    (c.a.props.onClick as (e: unknown) => void)(e);
    expect(opened).toEqual([["task", "GA-12"]]);
    expect(e.preventDefault).toHaveBeenCalled();
    expect(c.card).toBeNull();
  });

  it("opens it when no card shows too", () => {
    const c = itemChip("doc", "d-1");
    (c.a.props.onClick as (e: unknown) => void)({ preventDefault: () => {}, stopPropagation: () => {} });
    expect(opened).toEqual([["doc", "d-1"]]);
  });
});

describe("a click on a chip on the Chat page (GA-297)", () => {
  /** ItemChip in the runtime, inside the Chat page's BesideChat: what it opened beside the chat. */
  function chatChip(kind: ItemKind, key: string) {
    const beside: string[] = [];
    h.contexts.set(BesideChat, (id: string) => { beside.push(id); });
    let out!: ReactElement<{ children: [ReactElement<Record<string, unknown>>, ReactElement<CardProps> | null] }>;
    const inst: Inst = { slots: [], i: 0, effects: [], cleanups: [], render: () => {
      const prev = h.cur;
      h.cur = inst;
      inst.i = 0;
      try { out = ItemChip({ kind, itemKey: key, children: `${key} text` }) as typeof out; } finally { h.cur = prev; }
      const a = out.props.children[0];
      (a.props.ref as { current: unknown }).current = { textContent: `${key} text`, getClientRects: () => [] };
      for (const f of inst.effects.splice(0)) f();
    } };
    inst.render();
    mounted.push(inst);
    const click = (mods: { ctrlKey?: boolean; metaKey?: boolean } = {}) => {
      const e = { preventDefault: vi.fn(), stopPropagation: vi.fn(), ctrlKey: false, metaKey: false, ...mods };
      (out.props.children[0].props.onClick as (e: unknown) => void)(e);
      return e;
    };
    return { beside, click, get a() { return out.props.children[0]; }, get card() { return out.props.children[1]; } };
  }
  const onMac = () => vi.stubGlobal("navigator", { userAgent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0) AppleWebKit/605.1.15" });

  it("a card chip opens the card beside the chat, not its page, and hides its hover card", async () => {
    const c = chatChip("task", "GA-12");
    expect(c.a.props.href).toBe("gizai:task/GA-12");
    (c.a.props.onMouseEnter as (e: unknown) => void)({ clientX: 1, clientY: 1 });
    await wait(SHOW_MS);
    expect(c.card).not.toBeNull();
    const e = c.click();
    expect(c.beside).toEqual(["GA-12"]);
    expect(opened).toEqual([]);
    expect(e.preventDefault).toHaveBeenCalled();
    expect(e.stopPropagation).toHaveBeenCalled();
    expect(c.card).toBeNull();
  });

  it("Ctrl+click opens the card's page instead (Linux and Windows), Super+click too", () => {
    const c = chatChip("task", "GA-12");
    c.click({ ctrlKey: true });
    c.click({ metaKey: true });
    expect(opened).toEqual([["task", "GA-12"], ["task", "GA-12"]]);
    expect(c.beside).toEqual([]);
  });

  it("on macOS Cmd+click opens the page, and Ctrl+click (a right click there) doesn't", () => {
    onMac();
    const c = chatChip("task", "GA-12");
    c.click({ metaKey: true });
    expect(opened).toEqual([["task", "GA-12"]]);
    expect(c.beside).toEqual([]);
    c.click({ ctrlKey: true });
    expect(opened).toEqual([["task", "GA-12"]]);
    expect(c.beside).toEqual(["GA-12"]);
  });

  it("chips of other kinds still open their page", () => {
    for (const [kind, key] of [["project", "GA"], ["client", "c-1"], ["agent", "a-1"], ["person", "u-1"], ["doc", "d-1"]] as [ItemKind, string][]) {
      const c = chatChip(kind, key);
      c.click();
      expect(c.beside).toEqual([]);
    }
    expect(opened).toEqual([["project", "GA"], ["client", "c-1"], ["agent", "a-1"], ["person", "u-1"], ["doc", "d-1"]]);
  });

  it("outside the Chat page (no BesideChat) a card chip opens its page, as before", () => {
    const out = itemChipOutside("task", "GA-12");
    (out.props.onClick as (e: unknown) => void)({ preventDefault: () => {}, stopPropagation: () => {}, ctrlKey: false, metaKey: false });
    expect(opened).toEqual([["task", "GA-12"]]);
  });
  /** ItemChip's <a> rendered with no BesideChat. */
  function itemChipOutside(kind: ItemKind, key: string) {
    let out!: ReactElement<{ children: [ReactElement<Record<string, unknown>>, unknown] }>;
    const inst: Inst = { slots: [], i: 0, effects: [], cleanups: [], render: () => {
      const prev = h.cur;
      h.cur = inst;
      inst.i = 0;
      try { out = ItemChip({ kind, itemKey: key, children: key }) as typeof out; } finally { h.cur = prev; }
      (out.props.children[0].props.ref as { current: unknown }).current = { textContent: key, getClientRects: () => [] };
      for (const f of inst.effects.splice(0)) f();
    } };
    inst.render();
    mounted.push(inst);
    return out.props.children[0];
  }
});
