// GA-304 QA: an image pasted into the Chief Chat's text box. The editor calls the composer's onPasteNoText for a paste
// with no text (MarkdownEditorPaste.test.tsx checks when); the composer then asks Hersir for the clipboard's image
// (pasteClipboardImage). A path is added like a picked file (checkFiles, then a chip that says Pasted from the clipboard
// where a picked file's chip names its folder), null does nothing and shows no banner, and an error shows its sentence in
// the warning banner. The image goes with the message (alone too), or with the queued one while the Chief answers. While
// the Chief is paused the editor gets no onPasteNoText. There is no DOM here: a small hook runtime renders the real
// ChatThread (components called as functions, their hooks kept by their place in the tree, effects run after each
// render); the editor is a stand-in that records its props, and the api is a fake that records calls.
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ReactElement, ReactNode } from "react";
import type { Member, QueuedMessage } from "../../types";

type Inst = { slots: unknown[]; i: number };
type EffectSlot = { effect: true; deps?: unknown[]; first: boolean; cleanup?: () => void };
const h = vi.hoisted(() => ({
  cur: null as Inst | null,
  dirty: false,
  effects: [] as (() => void)[],
  calls: [] as [string, unknown[]][],
  answers: {} as Record<string, (...a: unknown[]) => unknown>,
  editor: null as null | Record<string, unknown>,
  chat: { messages: [], draft: "", tool: null, working: false, queue: [] as unknown[], pending: [], error: null, setWorking: () => {}, reload: () => {} },
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
  const useCallback = ((f: unknown, deps: unknown[]) => (h.cur ? useMemo(() => f, deps) : R.useCallback(f as () => void, deps))) as typeof R.useCallback;
  const useEffect = ((f: () => void | (() => void), deps?: unknown[]) => {
    if (!h.cur) return R.useEffect(f, deps);
    const s = slot<EffectSlot>(() => ({ effect: true, first: true }));
    if (s.first || changed(deps, s.deps)) {
      s.first = false;
      s.deps = deps;
      h.effects.push(() => { s.cleanup?.(); const c = f(); s.cleanup = typeof c === "function" ? c : undefined; });
    }
  }) as typeof R.useEffect;
  // the handle is made again on each render: what it reads is the composer's newest state
  const useImperativeHandle = ((ref: unknown, make: () => unknown) => {
    if (!h.cur) return R.useImperativeHandle(ref as never, make as never);
    if (ref && typeof ref === "object") (ref as { current: unknown }).current = make();
  }) as typeof R.useImperativeHandle;
  const hooks = { useState, useRef, useMemo, useCallback, useEffect, useLayoutEffect: useEffect, useImperativeHandle };
  return { ...R, default: { ...R, ...hooks }, ...hooks };
});
vi.mock("@tanstack/react-virtual", () => ({
  useVirtualizer: () => ({ getVirtualItems: () => [], getTotalSize: () => 0, measureElement: () => {} }),
}));
vi.mock("./useChat", () => ({ useChat: () => h.chat }));
vi.mock("../MarkdownEditor", () => ({ MarkdownEditor: (p: Record<string, unknown>) => { h.editor = p; return null; } }));
vi.mock("../Avatar", () => ({ Avatar: () => null }));
vi.mock("../../lib/useDropZone", () => ({ useDropZone: () => false }));
vi.mock("../../router", async (orig) => ({ ...(await orig<Record<string, unknown>>()), go: (r: unknown) => { h.calls.push(["go", [r]]); } }));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: (opts: unknown) => { h.calls.push(["open", [opts]]); return Promise.resolve(["/home/me/Downloads/spec.pdf"]); },
}));
vi.mock("../../api", async (orig) => {
  const real = await orig<Record<string, unknown>>();
  const out: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(real)) {
    out[k] = typeof v !== "function" ? v : (...a: unknown[]) => {
      h.calls.push([k, a]);
      const answer = h.answers[k];
      return answer ? Promise.resolve().then(() => answer(...a)) : new Promise(() => {});
    };
  }
  return out;
});
vi.stubGlobal("window", { devicePixelRatio: 1, addEventListener: () => {}, removeEventListener: () => {} });

const { ChatThread } = await import("./ChatThread");
const { fileFolder } = await import("../../lib/files");

const lead = (status = "active") => ({ actorId: "lead", name: "Chief", kind: "agent", roleKey: "lead", handle: "chief", status, isLead: false,
  allowedTools: [], chatEnabled: true }) as Member;
const ui = { threadId: "c1" as string | null, status: "active" };
function Root() {
  return <ChatThread threadId={ui.threadId} agent={lead(ui.status)} />;
}

// ---- the hook runtime ----
const insts = new Map<string, Inst>();
const unmount = (inst: Inst) => { for (const s of inst.slots) if ((s as EffectSlot)?.effect) (s as EffectSlot).cleanup?.(); };
const keyOf = (n: unknown) => (n && typeof n === "object" && "key" in n ? (n as { key: string | null }).key : null);
function expand(node: ReactNode, path: string, seen: Set<string>): ReactNode {
  if (Array.isArray(node)) return node.map((n, i) => expand(n, `${path}.${keyOf(n) ?? i}`, seen));
  if (!node || typeof node !== "object" || !("props" in node)) return node;
  const el = node as ReactElement<Record<string, unknown>>;
  if (typeof el.type === "function") {
    const key = `${path}>${el.type.name}`;
    seen.add(key);
    let inst = insts.get(key);
    if (!inst) insts.set(key, (inst = { slots: [], i: 0 }));
    inst.i = 0;
    h.cur = inst;
    let out: ReactNode;
    try { out = (el.type as (p: unknown) => ReactNode)(el.props); } finally { h.cur = null; }
    return expand(out, key, seen);
  }
  const p = el.props;
  return { ...el, props: { ...p, children: expand(p.children as ReactNode, `${path}>${String(el.type)}`, seen) } } as ReactNode;
}
let tree: ReactNode = null;
function render() {
  for (let n = 0; n < 30; n++) {
    h.dirty = false;
    const seen = new Set<string>();
    tree = expand(<Root />, "", seen);
    for (const [k, inst] of insts) if (!seen.has(k)) { unmount(inst); insts.delete(k); }
    for (const f of h.effects.splice(0)) f();
    if (!h.dirty) return;
  }
  throw new Error("the render never settled");
}
async function settle() {
  for (let i = 0; i < 4; i++) { await new Promise((r) => setTimeout(r, 0)); render(); }
}

// ---- finding things on screen ----
type Found = { type: unknown; props: Record<string, unknown> };
function findAll(node: ReactNode, pred: (f: Found) => boolean, out: Found[] = []): Found[] {
  if (Array.isArray(node)) node.forEach((n) => findAll(n, pred, out));
  else if (node && typeof node === "object" && "props" in node) {
    const el = node as ReactElement<Record<string, unknown>>;
    const f = { type: el.type, props: el.props };
    if (pred(f)) out.push(f);
    findAll(el.props.children as ReactNode, pred, out);
  }
  return out;
}
function textOf(node: ReactNode): string {
  if (typeof node === "string" || typeof node === "number") return ` ${node} `;
  if (Array.isArray(node)) return node.map(textOf).join("");
  if (node && typeof node === "object" && "props" in node) return textOf((node as ReactElement<{ children?: ReactNode }>).props.children);
  return "";
}
const squash = (s: string) => s.replace(/\s+/g, " ").trim();
/** The chips above the text box: each one's name and what is under it. */
const chips = () => findAll(tree, (f) => f.type === "span" && f.props.className === "fname").map((f) => {
  const [b, under] = f.props.children as ReactElement<{ children: ReactNode }>[];
  return [squash(textOf(b.props.children)), squash(textOf(under.props.children))];
});
/** The warning banner's text, or null. */
const banner = () => {
  const b = findAll(tree, (f) => f.type === "div" && f.props.className === "chat-banner warn")[0];
  return b ? squash(textOf(b.props.children as ReactNode)) : null;
};
type Btn = { onClick: (e?: unknown) => unknown; disabled?: boolean };
const button = (label: string) => {
  const all = findAll(tree, (f) => f.type === "button" && f.props["aria-label"] === label).map((f) => f.props as Btn);
  if (all.length !== 1) throw new Error(`${all.length} buttons labelled ${label}`);
  return all[0];
};
const called = (name: string) => h.calls.filter(([k]) => k === name).map(([, a]) => a);
/** Ctrl+V with no text on the clipboard: what the editor does then. */
async function paste() {
  const take = h.editor?.onPasteNoText as (() => void) | undefined;
  if (!take) throw new Error("the editor has no onPasteNoText");
  take();
  await settle();
}
/** Types in the text box (the editor's onChange). */
function type(text: string) { (h.editor!.onChange as (md: string) => void)(text); render(); }

const PASTED = "/home/me/.local/share/hersir/pasted/019a-1/pasted-image-2026-11-17_14-35-02.png";
const PASTED_2 = "/home/me/.local/share/hersir/pasted/019a-2/pasted-image-2026-11-17_14-35-09.png";
const SENTENCE = "The clipboard's image couldn't be pasted: the native clipboard is not accessible due to being held by another party.";
let clipboard: (string | null)[] = [];

beforeEach(() => {
  for (const [, inst] of insts) unmount(inst);
  insts.clear();
  h.calls.length = 0;
  h.editor = null;
  h.chat = { ...h.chat, working: false, queue: [] };
  ui.threadId = "c1";
  ui.status = "active";
  clipboard = [];
  h.answers = {
    chatClis: () => [],
    checkFiles: (paths) => ({ ok: paths as string[], failed: [] }),
    pasteClipboardImage: () => (clipboard.length ? clipboard.shift() : null),
    sendChat: () => "c1",
  };
  render();
});

describe("Ctrl+V with an image on the clipboard", () => {
  it("adds one chip named after the file, with Pasted from the clipboard under its name", async () => {
    clipboard = [PASTED];
    await paste();
    expect(called("pasteClipboardImage")).toEqual([[]]);
    expect(called("checkFiles")).toEqual([[[PASTED]]]);
    expect(chips()).toEqual([["pasted-image-2026-11-17_14-35-02.png", "Pasted from the clipboard"]]);
    expect(banner()).toBeNull();
  });
  it("leaves the typed text as it is", async () => {
    type("Here is the error");
    clipboard = [PASTED];
    await paste();
    expect(h.editor!.value).toBe("Here is the error");
    expect(chips()).toHaveLength(1);
  });
  it("twice gives two chips; a chip's X removes only that one", async () => {
    clipboard = [PASTED, PASTED_2];
    await paste();
    await paste();
    expect(chips()).toEqual([
      ["pasted-image-2026-11-17_14-35-02.png", "Pasted from the clipboard"],
      ["pasted-image-2026-11-17_14-35-09.png", "Pasted from the clipboard"],
    ]);
    button("Remove pasted-image-2026-11-17_14-35-02.png").onClick();
    render();
    expect(chips()).toEqual([["pasted-image-2026-11-17_14-35-09.png", "Pasted from the clipboard"]]);
  });
  it("twice in the same second (two folders, one name) still gives two chips, and X removes the one clicked", async () => {
    const same = PASTED.replace("019a-1", "019a-3");
    clipboard = [PASTED, same];
    await paste();
    await paste();
    expect(chips()).toHaveLength(2);
    const xs = findAll(tree, (f) => f.type === "button" && f.props["aria-label"] === "Remove pasted-image-2026-11-17_14-35-02.png");
    (xs[1].props as Btn).onClick();
    render();
    const left = findAll(tree, (f) => f.type === "span" && f.props.className === "file pending").map((f) => f.props.title);
    expect(left).toEqual([PASTED]);
  });
  it("beside a file from + → Add files: that chip still names its folder, and both go with the message", async () => {
    clipboard = [PASTED];
    await paste();
    // + opens the menu (a click on the Popover's span), then Add files picks /home/me/Downloads/spec.pdf
    const toggle = findAll(tree, (f) => f.type === "span" && f.props["aria-haspopup"] === "menu");
    expect(toggle).toHaveLength(1);
    (toggle[0].props.onClick as () => void)();
    render();
    const add = findAll(tree, (f) => f.type === "button" && f.props.role === "menuitem" && squash(textOf(f.props.children as ReactNode)) === "Add files");
    expect(add).toHaveLength(1);
    (add[0].props.onClick as () => void)();
    await settle();
    expect(called("open")).toHaveLength(1);
    expect(called("checkFiles")).toEqual([[[PASTED]], [["/home/me/Downloads/spec.pdf"]]]);
    expect(chips()).toEqual([
      ["pasted-image-2026-11-17_14-35-02.png", "Pasted from the clipboard"],
      ["spec.pdf", squash(fileFolder("/home/me/Downloads/spec.pdf"))],
    ]);
    expect(chips()[1][1]).not.toBe("Pasted from the clipboard");
    button("Send").onClick();
    await settle();
    expect(called("sendChat")).toEqual([["c1", "", null, [PASTED, "/home/me/Downloads/spec.pdf"]]]);
  });
  it("a file the check refuses shows Not added in the banner, as for a picked file", async () => {
    h.answers.checkFiles = () => ({ ok: [], failed: ["pasted-image-2026-11-17_14-35-02.png is over 1 GB"] });
    clipboard = [PASTED];
    await paste();
    expect(chips()).toEqual([]);
    expect(banner()).toBe("Not added: pasted-image-2026-11-17_14-35-02.png is over 1 GB");
  });
});

describe("Ctrl+V with no image on the clipboard", () => {
  it("adds no chip and shows no banner", async () => {
    await paste();
    expect(called("pasteClipboardImage")).toHaveLength(1);
    expect(called("checkFiles")).toEqual([]);
    expect(chips()).toEqual([]);
    expect(banner()).toBeNull();
  });
});

describe("an image that can't be read or saved", () => {
  it("shows its one sentence in the warning banner and changes nothing in the message", async () => {
    clipboard = [PASTED];
    await paste();
    type("keep this");
    h.answers.pasteClipboardImage = () => { throw SENTENCE; };
    await paste();
    expect(banner()).toBe(SENTENCE);
    expect(banner()!.startsWith("The clipboard's image couldn't be pasted: ")).toBe(true);
    expect(chips()).toEqual([["pasted-image-2026-11-17_14-35-02.png", "Pasted from the clipboard"]]);
    expect(h.editor!.value).toBe("keep this");
    // the banner's X closes it
    button("Close").onClick();
    render();
    expect(banner()).toBeNull();
  });
});

describe("sending", () => {
  it("a message of only a pasted image: Send is on and the image goes with it", async () => {
    expect(button("Send").disabled).toBe(true);
    clipboard = [PASTED];
    await paste();
    expect(button("Send").disabled).toBe(false);
    button("Send").onClick();
    await settle();
    expect(called("sendChat")).toEqual([["c1", "", null, [PASTED]]]);
    expect(chips()).toEqual([]);
  });
  it("with text and two pasted images", async () => {
    clipboard = [PASTED, PASTED_2];
    await paste();
    type("Two screenshots");
    await paste();
    (h.editor!.onEnter as (md: string) => void)("Two screenshots");
    await settle();
    expect(called("sendChat")).toEqual([["c1", "Two screenshots", null, [PASTED, PASTED_2]]]);
  });
  it("while the Chief answers: the pasted image goes with the queued message, alone too", async () => {
    h.chat = { ...h.chat, working: true, queue: [] as QueuedMessage[] };
    render();
    clipboard = [PASTED];
    await paste();
    expect(chips()).toHaveLength(1);
    const q = button("Queue");
    expect(q.disabled).toBe(false);
    q.onClick();
    await settle();
    expect(called("sendChat")).toEqual([["c1", "", null, [PASTED]]]);
    expect(chips()).toEqual([]);
  });
});

describe("while the Chief is paused", () => {
  it("the text box is off and paste adds nothing: the editor gets no onPasteNoText", async () => {
    ui.status = "paused";
    render();
    expect(h.editor!.disabled).toBe(true);
    expect(h.editor!.onPasteNoText).toBeUndefined();
    // resumed, it pastes again
    ui.status = "active";
    render();
    expect(typeof h.editor!.onPasteNoText).toBe("function");
  });
});
