// GA-304 QA: MarkdownEditor's `onPasteNoText`. A paste whose clipboard data has neither text/plain nor text/uri-list (an
// image only, such as a screenshot) is stopped before CodeMirror's own paste, so the text and its selection stay as they
// are, and the prop is called. A paste with text (also with an image beside it) is CodeMirror's as before, and so is every
// paste without the prop, in a read-only (disabled) editor, and within 1.5 s of a middle-click (Linux's primary selection).
// There is no DOM here: a small hook runtime runs the component, and a stand-in EditorView (a real EditorState, no DOM)
// takes its extensions; the paste and mousedown handlers are the ones the editor gives EditorView.domEventHandlers.
// CodeMirror runs a handler that returns true (or calls preventDefault) instead of its own paste.
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { EditorSelection, EditorState, type TransactionSpec } from "@codemirror/state";

type Inst = { slots: unknown[]; i: number };
type EffectSlot = { effect: true; deps?: unknown[]; first: boolean; cleanup?: () => void };
type View = { state: EditorState; dispatch: (spec: TransactionSpec) => void };
type Handlers = Record<string, (e: unknown, v: View) => boolean | void>;
const h = vi.hoisted(() => ({
  cur: null as Inst | null,
  dirty: false,
  effects: [] as (() => void)[],
  views: [] as View[],
  handlers: [] as Record<string, (e: unknown, v: unknown) => boolean | void>[],
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
    const box = slot(() => {
      const b = { v: typeof init === "function" ? (init as () => unknown)() : init, set: (x: unknown) => {
        const v = typeof x === "function" ? (x as (p: unknown) => unknown)(b.v) : x;
        if (!Object.is(v, b.v)) { b.v = v; h.dirty = true; }
      } };
      return b;
    });
    return [box.v, box.set];
  }) as typeof R.useState;
  const useRef = ((init: unknown) => (h.cur ? slot(() => ({ current: init })) : R.useRef(init))) as typeof R.useRef;
  const useMemo = ((f: () => unknown, deps: unknown[]) => {
    if (!h.cur) return R.useMemo(f, deps);
    const s = slot(() => ({ deps: undefined as unknown[] | undefined, v: undefined as unknown }));
    if (changed(deps, s.deps)) { s.v = f(); s.deps = deps; }
    return s.v;
  }) as typeof R.useMemo;
  const useEffect = ((f: () => void | (() => void), deps?: unknown[]) => {
    if (!h.cur) return R.useEffect(f, deps);
    const s = slot<EffectSlot>(() => ({ effect: true, first: true }));
    if (s.first || changed(deps, s.deps)) {
      s.first = false;
      s.deps = deps;
      h.effects.push(() => { s.cleanup?.(); const c = f(); s.cleanup = typeof c === "function" ? c : undefined; });
    }
  }) as typeof R.useEffect;
  const hooks = { useState, useRef, useMemo, useEffect, useLayoutEffect: useEffect };
  return { ...R, default: { ...R, ...hooks }, ...hooks };
});

// The EditorView stand-in: the component's state, dispatch applied to it; domEventHandlers records the handlers given.
vi.mock("@codemirror/view", async (orig) => {
  const real = await orig<typeof import("@codemirror/view")>();
  class FakeView {
    state: EditorState;
    hasFocus = false;
    constructor(config: { state: EditorState }) { this.state = config.state; h.views.push(this); }
    get visibleRanges() { return [{ from: 0, to: this.state.doc.length }]; }
    get viewport() { return { from: 0, to: this.state.doc.length }; }
    dispatch(spec: TransactionSpec) { this.state = this.state.update(spec).state; }
    focus() { this.hasFocus = true; }
    destroy() {}
    requestMeasure() {}
    static domEventHandlers(handlers: Record<string, (e: unknown, v: unknown) => boolean | void>) {
      h.handlers.push(handlers);
      return real.EditorView.domEventHandlers(handlers as never);
    }
  }
  Object.setPrototypeOf(FakeView, real.EditorView); // its statics: theme, editorAttributes, updateListener, …
  return { ...real, EditorView: FakeView };
});
vi.stubGlobal("navigator", { userAgent: "Linux" });

const { MarkdownEditor } = await import("./MarkdownEditor");
type Props = Parameters<typeof MarkdownEditor>[0];

// ---- the hook runtime: one MarkdownEditor ----
let inst: Inst = { slots: [], i: 0 };
function render(props: Props) {
  for (let n = 0; n < 30; n++) {
    h.dirty = false;
    inst.i = 0;
    h.cur = inst;
    try { MarkdownEditor(props); } finally { h.cur = null; }
    for (const f of h.effects.splice(0)) f();
    if (!h.dirty) return;
  }
  throw new Error("the render never settled");
}
beforeEach(() => { inst = { slots: [], i: 0 }; h.views.length = 0; h.effects.length = 0; h.handlers.length = 0; });
afterEach(() => { vi.useRealTimers(); });

const TEXT = "Look at this screenshot please";
/** An editor showing TEXT with "this screenshot" selected, and its paste and mousedown handlers. */
function mount(props: Partial<Props> = {}) {
  render({ value: TEXT, ...props });
  expect(h.views).toHaveLength(1);
  const view = h.views[0];
  view.state = view.state.update({ selection: EditorSelection.single(8, 23) }).state;
  const all = h.handlers.filter((x) => x.paste || x.mousedown);
  expect(all).toHaveLength(1);
  return { view, paste: all[0].paste as Handlers["paste"], mousedown: all[0].mousedown as Handlers["mousedown"] };
}
/** A paste event whose clipboard data has these types (with their text); `files` as WebKit may give an image. */
function pasteEvent(data: Record<string, string> | null, files: unknown[] = []) {
  const clipboardData = data === null ? null : {
    types: [...Object.keys(data), ...(files.length ? ["Files"] : [])],
    files,
    getData: (t: string) => data[t] ?? "",
  };
  return { clipboardData, preventDefault: vi.fn(), defaultPrevented: false };
}
const IMAGE = { name: "image.png", type: "image/png", size: 10 };
const unchanged = (view: View) => {
  expect(view.state.doc.toString()).toBe(TEXT);
  expect(view.state.selection.main.from).toBe(8);
  expect(view.state.selection.main.to).toBe(23);
};

describe("a paste with no text in it", () => {
  it("is stopped before CodeMirror's own paste and calls onPasteNoText once; the text and its selection stay", () => {
    const take = vi.fn();
    const { view, paste } = mount({ onPasteNoText: take });
    for (const data of [{}, { "image/png": "" }] as Record<string, string>[]) {
      const e = pasteEvent(data, [IMAGE]);
      expect(paste(e, view)).toBe(true);
      expect(e.preventDefault).toHaveBeenCalledTimes(1);
      unchanged(view);
    }
    expect(take).toHaveBeenCalledTimes(2);
  });
  it("also with nothing on the clipboard at all (the parent finds no image)", () => {
    const take = vi.fn();
    const { view, paste } = mount({ onPasteNoText: take });
    const e = pasteEvent({});
    expect(paste(e, view)).toBe(true);
    expect(take).toHaveBeenCalledTimes(1);
    unchanged(view);
  });
  it("calls the newest onPasteNoText, and none once it is taken away (the composer while the Chief is paused)", () => {
    const a = vi.fn(), b = vi.fn();
    const { view, paste } = mount({ onPasteNoText: a });
    render({ value: TEXT, onPasteNoText: b });
    expect(paste(pasteEvent({}), view)).toBe(true);
    expect([a.mock.calls.length, b.mock.calls.length]).toEqual([0, 1]);
    render({ value: TEXT, onPasteNoText: undefined });
    const e = pasteEvent({}, [IMAGE]);
    expect(paste(e, view)).toBe(false);
    expect(e.preventDefault).not.toHaveBeenCalled();
    expect(b).toHaveBeenCalledTimes(1);
  });
});

describe("a paste CodeMirror handles as before", () => {
  it("has text: text/plain, also with an image beside it (cells copied from a spreadsheet)", () => {
    const take = vi.fn();
    const { view, paste } = mount({ onPasteNoText: take });
    for (const [data, files] of [
      [{ "text/plain": "hello" }, []],
      [{ "text/plain": "a\tb\n1\t2", "text/html": "<table></table>", "image/png": "" }, [IMAGE]],
      [{ "text/plain": " " }, []],
    ] as [Record<string, string>, unknown[]][]) {
      const e = pasteEvent(data, files);
      expect(paste(e, view)).toBe(false);
      expect(e.preventDefault).not.toHaveBeenCalled();
    }
    expect(take).not.toHaveBeenCalled();
    unchanged(view);
  });
  it("has a link only (text/uri-list)", () => {
    const take = vi.fn();
    const { view, paste } = mount({ onPasteNoText: take });
    const e = pasteEvent({ "text/uri-list": "https://example.com/" });
    expect(paste(e, view)).toBe(false);
    expect(take).not.toHaveBeenCalled();
  });
  it("has no clipboard data", () => {
    const take = vi.fn();
    const { view, paste } = mount({ onPasteNoText: take });
    expect(paste(pasteEvent(null), view)).toBe(false);
    expect(take).not.toHaveBeenCalled();
  });
  it("is in an editor without onPasteNoText: every other text box (comments, the New task drawer)", () => {
    const { view, paste } = mount();
    const e = pasteEvent({}, [IMAGE]);
    expect(paste(e, view)).toBe(false);
    expect(e.preventDefault).not.toHaveBeenCalled();
  });
  it("is in a disabled (read-only) editor: CodeMirror then pastes nothing", () => {
    const take = vi.fn();
    const { view, paste } = mount({ onPasteNoText: take, disabled: true });
    expect(view.state.readOnly).toBe(true);
    expect(paste(pasteEvent({}, [IMAGE]), view)).toBe(false);
    expect(take).not.toHaveBeenCalled();
  });
});

describe("a middle-click (Linux pastes the primary selection, not the clipboard)", () => {
  const down = (button: number) => ({ button, ctrlKey: false, metaKey: false, clientX: 0, clientY: 0, preventDefault: vi.fn() });
  it("never adds the clipboard's image: a paste with no text within 1.5 s of it is CodeMirror's", () => {
    vi.useFakeTimers({ now: 1_763_400_000_000, toFake: ["Date"] });
    const take = vi.fn();
    const { view, paste, mousedown } = mount({ onPasteNoText: take });
    expect(mousedown(down(1), view)).toBe(false);
    vi.setSystemTime(Date.now() + 1499);
    expect(paste(pasteEvent({}), view)).toBe(false);
    expect(take).not.toHaveBeenCalled();
    // later, Ctrl+V adds it again
    vi.setSystemTime(Date.now() + 2);
    expect(paste(pasteEvent({}), view)).toBe(true);
    expect(take).toHaveBeenCalledTimes(1);
  });
  it("is the middle button only: a left or right click right before Ctrl+V or the menu's Paste changes nothing", () => {
    vi.useFakeTimers({ now: 1_763_400_000_000, toFake: ["Date"] });
    const take = vi.fn();
    const { view, paste, mousedown } = mount({ onPasteNoText: take });
    mousedown(down(0), view);
    expect(paste(pasteEvent({}), view)).toBe(true);
    mousedown(down(2), view);
    expect(paste(pasteEvent({}), view)).toBe(true);
    expect(take).toHaveBeenCalledTimes(2);
  });
});
