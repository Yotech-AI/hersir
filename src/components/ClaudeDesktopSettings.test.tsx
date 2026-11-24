// GA-340 QA: Settings → MCP servers → Claude Desktop (ClaudeDesktopSettings, on GA-338's ClaudeDesktopStatus and
// ClaudeDesktopConnection). Not connected: the checkbox (on when Claude Desktop's settings file was found, else off and
// disabled with its hint) and Connect (disabled with its hint without hersir-mcp), which sends the box as addToConfig.
// After Connect: "Added to…", the backup and "Quit and reopen…" when Hersir wrote the file, else configError, the entry with
// Copy, the Edit Config line and the key warning. Connected: "Connected since …", the missing-entry line only when the file
// is there without the entry, Connect again and Disconnect, which asks first. A failing command shows its error and the
// section stays usable. The card's texts are written out here word for word, not taken from src/lib/claudeDesktop.ts.
//
// Rendered to HTML on the server, so no data loads: the component's state is handed in, in the order of its useState calls
// (st, ticked, result, busy, err, asking, then ConnectResult's copied). Buttons are used on the element tree with the hooks
// replaced (each setter records what it is given), and the api is mocked to see what Connect and Disconnect send.
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ReactElement, ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import type { ClaudeDesktopConnection, ClaudeDesktopStatus } from "../types";

const queue: unknown[] = [];
// Direct: components are called as functions; each useState's setter records what it is given, in `sets[i]`, and each
// effect is kept in `effects` to run by hand.
let sets: unknown[][] | null = null;
const effects: (() => unknown)[] = [];
vi.mock("react", async (orig) => {
  const R = await orig<typeof import("react")>();
  const useState = ((init: unknown) => {
    const v = queue.length ? queue.shift() : typeof init === "function" ? (init as () => unknown)() : init;
    if (!sets) return R.useState(v);
    const mine: unknown[] = [];
    sets.push(mine);
    return [v, (x: unknown) => mine.push(x)];
  }) as typeof R.useState;
  const useEffect = ((f: () => void, deps?: unknown[]) => (sets ? void effects.push(f) : R.useEffect(f, deps))) as typeof R.useEffect;
  const useRef = ((init: unknown) => (sets ? { current: init } : R.useRef(init))) as typeof R.useRef;
  return { ...R, default: { ...R, useState, useEffect, useRef }, useState, useEffect, useRef };
});

const calls: [string, unknown[]][] = [];
const answers: Record<string, (...a: unknown[]) => unknown> = {};
vi.mock("../api", async (orig) => {
  const real = await orig<Record<string, unknown>>();
  const out: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(real)) {
    out[k] = typeof v !== "function" ? v : (...a: unknown[]) => { calls.push([k, a]); return new Promise((ok) => ok(answers[k]?.(...a))); };
  }
  return out;
});

const { ClaudeDesktopSettings } = await import("./ClaudeDesktopSettings");

// The card's texts, word for word.
const ADD = "Add it to Claude Desktop's settings";
const NO_CONFIG = "Claude Desktop's settings weren't found on this computer: you'll paste the entry yourself.";
const NO_SHIM = "Hersir can't find its hersir-mcp program next to Hersir: reinstall Hersir.";
const REOPEN = "Quit and reopen Claude Desktop, then ask Claude: \"What's on my Hersir board?\"";
const PASTE = "In Claude Desktop: Settings → Developer → Edit Config. Paste this into the file (add the \"hersir\" server next to any servers already there), save it, then quit and reopen Claude Desktop.";
const KEY = "This entry holds a key with the Chief's powers over your board: any program running as you could use it. Hersir shows it only now; Connect again makes a new one and ends this one.";
const NO_ENTRY = "Claude Desktop's settings don't have the Hersir entry.";
const ASK_TITLE = "Disconnect Claude Desktop?";
const ASK_TEXT = "Claude Desktop can no longer use your board until you connect it again. Its settings keep the entry; Claude will say that Hersir disconnected it.";

const CONFIG = "/home/you/.config/Claude/claude_desktop_config.json";
const BACKUP = "/home/you/.config/Claude/claude_desktop_config.json.before-hersir";
const SHIM = "/home/you/.local/lib/hersir/hersir-mcp";
const AT = new Date(2026, 10, 23, 14, 2).getTime(); // 23 Nov 2026, 14:02 local time
const ENTRY = JSON.stringify({ mcpServers: { hersir: { command: SHIM, args: [], env: { HERSIR_MCP_KEY: "hk_3f9a<&>'\"c1" } } } }, null, 2);

const status = (s: Partial<ClaudeDesktopStatus> = {}): ClaudeDesktopStatus =>
  ({ connected: false, connectedAt: null, configPath: CONFIG, configHasHersir: false, shimPath: SHIM, ...s });
const connected = (s: Partial<ClaudeDesktopStatus> = {}) => status({ connected: true, connectedAt: AT, configHasHersir: true, ...s });
const connection = (c: Partial<ClaudeDesktopConnection> = {}): ClaudeDesktopConnection =>
  ({ entry: ENTRY, wroteConfig: true, configPath: CONFIG, backupPath: null, configError: null, status: connected(), ...c });
const pasted = (c: Partial<ClaudeDesktopConnection> = {}) =>
  connection({ wroteConfig: false, configPath: CONFIG, status: connected({ configHasHersir: false }), ...c });

const decode = (s: string) => s.replace(/&#x27;/g, "'").replace(/&quot;/g, '"').replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&amp;/g, "&");
const text = (html: string) => decode(html.replace(/<[^>]+>/g, " ")).replace(/\s+/g, " ").trim();

type State = { st?: ClaudeDesktopStatus | null; ticked?: boolean | null; result?: ClaudeDesktopConnection | null; busy?: string; err?: string | null;
  asking?: boolean; copied?: boolean };
const states = (s: State) => {
  const v: unknown[] = [s.st === undefined ? status() : s.st, s.ticked ?? null, s.result ?? null, s.busy ?? "", s.err ?? null, s.asking ?? false];
  if (s.copied !== undefined) v.push(s.copied);
  return v;
};
function render(s: State = {}) {
  queue.push(...states(s));
  try { return renderToStaticMarkup(<ClaudeDesktopSettings />); } finally { queue.length = 0; }
}

function expand(node: ReactNode): ReactNode {
  if (Array.isArray(node)) return node.map(expand);
  if (node && typeof node === "object" && "props" in node) {
    const el = node as ReactElement<Record<string, unknown>>;
    if (typeof el.type === "function") return expand((el.type as (p: unknown) => ReactNode)(el.props));
    return { ...el, props: { ...el.props, children: expand(el.props.children as ReactNode) } } as ReactNode;
  }
  return node;
}
function findAll(node: ReactNode, pred: (p: Record<string, unknown>) => boolean, out: Record<string, unknown>[] = []) {
  if (Array.isArray(node)) node.forEach((n) => findAll(n, pred, out));
  else if (node && typeof node === "object" && "props" in node) {
    const p = (node as ReactElement<Record<string, unknown>>).props;
    if (pred(p)) out.push(p);
    findAll(p.children as ReactNode, pred, out);
  }
  return out;
}
const label = (p: Record<string, unknown>) => [p.children].flat(Infinity).filter((c) => typeof c === "string").join("");
type Button = { onClick: (e?: unknown) => unknown; disabled?: boolean; className?: string };
// the index of each state among the useState calls
const ST = 0, TICKED = 1, RESULT = 2, BUSY = 3, ERR = 4, ASKING = 5, COPIED = 6;

/** The section as an element tree with state `s`; `got[i]` gets what the i-th useState's setter was given. */
function tree(s: State = {}) {
  sets = [];
  effects.length = 0;
  queue.push(...states(s));
  try {
    const t = expand(ClaudeDesktopSettings());
    const got = sets;
    const fx = [...effects];
    const button = (name: string) => findAll(t, (p) => typeof p.onClick === "function" && label(p) === name) as Button[];
    const byLabel = (name: string) => findAll(t, (p) => p["aria-label"] === name)[0] as Button;
    const checkbox = () => findAll(t, (p) => p.type === "checkbox")[0] as { checked: boolean; disabled: boolean; onChange: (e: unknown) => void };
    const dialog = () => findAll(t, (p) => p.role === "alertdialog")[0] as { onKeyDown: (e: unknown) => void } | undefined;
    const scrim = () => findAll(t, (p) => p.className === "ask-scrim")[0] as { onMouseDown: (e: unknown) => void } | undefined;
    return { t, got, fx, button, byLabel, checkbox, dialog, scrim };
  } finally { sets = null; queue.length = 0; }
}
const settle = () => new Promise((r) => setTimeout(r, 0));
/** The entry in its read-only block, as text. */
const entryOf = (html: string) => decode(/<pre[^>]*>([\s\S]*?)<\/pre>/.exec(html)?.[1] ?? "");

beforeEach(() => {
  calls.length = 0;
  for (const k of Object.keys(answers)) delete answers[k];
});
afterEach(() => {
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

describe("not connected, with Claude Desktop's settings found", () => {
  it("has the checkbox on and enabled, and Connect as the primary button, without the hints", () => {
    const html = render();
    expect(html).toContain(`<label class="check"><input type="checkbox" checked=""/>Add it to Claude Desktop&#x27;s settings</label>`);
    expect(html).toContain('<button type="button" class="btn primary">Connect</button>');
    const t = text(html);
    expect(t).toContain(ADD);
    for (const s of [NO_CONFIG, NO_SHIM, "Connected", "Connect again", "Disconnect", NO_ENTRY]) expect(t).not.toContain(s);
    const { checkbox } = tree();
    expect(checkbox()).toMatchObject({ checked: true, disabled: false });
  });

  it("Connect calls claude_desktop_connect with addToConfig true, then keeps its answer and the status after it", async () => {
    const answer = connection({ backupPath: BACKUP });
    answers.claudeDesktopConnect = () => answer;
    const { got, button } = tree();
    button("Connect")[0].onClick();
    await settle();
    expect(calls).toEqual([["claudeDesktopConnect", [true]]]);
    expect(got[RESULT]).toEqual([answer]);
    expect(got[ST]).toEqual([answer.status]);
    expect(got[BUSY]).toEqual(["connect", ""]);
    expect(got[ERR]).toEqual([null]);
  });

  it("while Connect runs, Connect says Connecting… and is off, as is the checkbox", () => {
    const html = render({ busy: "connect" });
    expect(html).toContain('<button type="button" class="btn primary" disabled="">Connecting…</button>');
    expect(html).toMatch(/<input type="checkbox" (checked="" disabled=""|disabled="" checked="")\/>/);
  });

  it("with wroteConfig true: Added to … (configPath), the old file's backup, then Quit and reopen, and no entry to paste", () => {
    const html = render({ st: connected(), result: connection({ backupPath: BACKUP }) });
    const t = text(html);
    const added = `Added to Claude Desktop's settings (${CONFIG}).`, backup = `The old file is saved as ${BACKUP}.`;
    expect(t).toContain(`${added} ${backup} ${REOPEN}`);
    for (const s of [PASTE, KEY, "Copy"]) expect(t).not.toContain(s);
    expect(html).not.toContain("<pre");
  });

  it("with wroteConfig true and no backup: no backup sentence", () => {
    const t = text(render({ st: connected(), result: connection({ backupPath: null }) }));
    expect(t).toContain(`Added to Claude Desktop's settings (${CONFIG}). ${REOPEN}`);
    expect(t).not.toContain("The old file is saved as");
  });
});

describe("the checkbox", () => {
  it("unticked, Connect sends addToConfig false; ticked again, true", async () => {
    const first = tree();
    first.checkbox().onChange({ target: { checked: false } });
    expect(first.got[TICKED]).toEqual([false]);

    const off = tree({ ticked: false });
    expect(off.checkbox()).toMatchObject({ checked: false, disabled: false });
    answers.claudeDesktopConnect = () => pasted();
    off.button("Connect")[0].onClick();
    await settle();
    expect(calls).toEqual([["claudeDesktopConnect", [false]]]);

    off.checkbox().onChange({ target: { checked: true } });
    expect(off.got[TICKED]).toEqual([true]);
    calls.length = 0;
    tree({ ticked: true }).button("Connect")[0].onClick();
    await settle();
    expect(calls).toEqual([["claudeDesktopConnect", [true]]]);
  });

  it("without Claude Desktop's settings (configPath null): off and disabled, with its hint, and Connect sends addToConfig false", async () => {
    const st = status({ configPath: null });
    const html = render({ st });
    expect(html).toContain(`<label class="check"><input type="checkbox" disabled=""/>Add it to Claude Desktop&#x27;s settings</label>`);
    expect(text(html)).toContain(`${ADD} ${NO_CONFIG}`);
    expect(html).toContain('<button type="button" class="btn primary">Connect</button>');
    answers.claudeDesktopConnect = () => pasted({ configPath: null, status: connected({ configPath: null, configHasHersir: false }) });
    // also when the box was ticked before the file went away
    for (const ticked of [null, true]) {
      calls.length = 0;
      const t = tree({ st, ticked });
      expect(t.checkbox()).toMatchObject({ checked: false, disabled: true });
      t.button("Connect")[0].onClick();
      await settle();
      expect(calls).toEqual([["claudeDesktopConnect", [false]]]);
    }
  });
});

describe("wroteConfig false: the entry to paste", () => {
  it("shows the entry read-only in monospace with Copy, then the Edit Config line, and the key warning under it", () => {
    const html = render({ st: connected({ configHasHersir: false }), result: pasted() });
    expect(html).toMatch(/<pre class="mono"[^>]*>/);
    expect(entryOf(html)).toBe(ENTRY);
    // read-only: a block of text, no field to type in
    expect(html).not.toMatch(/<(textarea|input)(?![^>]*type="checkbox")/);
    expect(html).toContain('<button type="button" class="btn sm" aria-label="Copy the entry">');
    const t = text(html);
    expect(t).toContain(`Copy ${PASTE} ${KEY}`);
    expect(t.indexOf('"mcpServers"')).toBeLessThan(t.indexOf(PASTE));
    expect(t).not.toContain("Added to Claude Desktop's settings");
    expect(t).not.toContain(REOPEN);
  });

  it("Copy puts exactly the entry on the clipboard, and reads Copied for 2 s", async () => {
    const writeText = vi.fn(async (_s: string) => {});
    vi.stubGlobal("navigator", { clipboard: { writeText } });
    vi.useFakeTimers();
    const { got, byLabel } = tree({ st: connected({ configHasHersir: false }), result: pasted() });
    await byLabel("Copy the entry").onClick();
    expect(writeText).toHaveBeenCalledTimes(1);
    expect(writeText.mock.calls[0][0]).toBe(ENTRY);
    expect(got[COPIED]).toEqual([true]);
    expect(got[ERR]).toEqual([null]);
    vi.advanceTimersByTime(2000);
    expect(got[COPIED]).toEqual([true, false]);
    expect(text(render({ st: connected(), result: pasted(), copied: true }))).toContain("Copied");
  });

  it("says so in the section when the clipboard refuses", async () => {
    vi.stubGlobal("navigator", { clipboard: { writeText: async () => { throw new Error("not allowed"); } } });
    const { got, byLabel } = tree({ st: connected(), result: pasted() });
    await byLabel("Copy the entry").onClick();
    expect(got[ERR]).toEqual(["Couldn't copy to the clipboard: select the entry and copy it yourself."]);
    expect(got[COPIED]).toEqual([]);
  });

  it("shows configError first, above the entry", () => {
    const why = "Claude Desktop's settings file isn't valid JSON, so Hersir didn't change it.";
    const html = render({ st: connected({ configHasHersir: false }), result: pasted({ configError: why }) });
    expect(html).toContain(`<span class="warn">Claude Desktop&#x27;s settings file isn&#x27;t valid JSON, so Hersir didn&#x27;t change it.</span><div class="cd-entry">`);
    const t = text(html);
    expect(t.indexOf(why)).toBeLessThan(t.indexOf('"mcpServers"'));
    expect(t).toContain(`Copy ${PASTE} ${KEY}`);
  });

  it("shows no error line without configError", () => {
    const html = render({ st: connected({ configHasHersir: false }), result: pasted() });
    expect(html).toContain('<div class="cd-result" role="status"><div class="cd-entry">');
  });
});

describe("without hersir-mcp (shimPath null)", () => {
  it("Connect is off, with its hint", () => {
    const html = render({ st: status({ shimPath: null }) });
    expect(html).toMatch(/<button type="button" class="btn primary" disabled="" title="[^"]*">Connect<\/button>/);
    expect(text(html)).toContain(NO_SHIM);
    const { button } = tree({ st: status({ shimPath: null }) });
    expect(button("Connect")[0].disabled).toBe(true);
  });

  it("Connect again is off too, with its hint", () => {
    const html = render({ st: connected({ shimPath: null }) });
    expect(html).toMatch(/<button type="button" class="btn" disabled="" title="[^"]*">Connect again<\/button>/);
    expect(text(html)).toContain(NO_SHIM);
  });
});

describe("connected", () => {
  it("shows Connected since … in the app's date and time, Connect again and Disconnect, and no Connect", () => {
    const html = render({ st: connected() });
    const t = text(html);
    expect(t).toContain("Connected since 23 Nov 2026, 14:02");
    expect(html).toContain('<button type="button" class="btn">Connect again</button>');
    expect(html).toContain('<button type="button" class="btn ghost">Disconnect</button>');
    expect(html).not.toContain(">Connect</button>");
    expect(t).not.toContain(NO_ENTRY);
  });

  it("says the settings don't have the Hersir entry only when the file is there without it", () => {
    expect(text(render({ st: connected({ configHasHersir: false }) }))).toContain(NO_ENTRY);
    expect(text(render({ st: connected({ configHasHersir: true }) }))).not.toContain(NO_ENTRY);
    expect(text(render({ st: connected({ configPath: null, configHasHersir: false }) }))).not.toContain(NO_ENTRY);
  });

  it("right after Connect, shows Connected with what Connect said", () => {
    const answer = connection({ backupPath: BACKUP });
    const t = text(render({ st: answer.status, result: answer }));
    expect(t).toContain("Connected since 23 Nov 2026, 14:02");
    expect(t).toContain("Connect again Disconnect");
    expect(t).toContain(`Added to Claude Desktop's settings (${CONFIG}).`);
  });

  it("Connect again runs Connect's flow: a new key in its answer, with the checkbox as addToConfig", async () => {
    const again = pasted({ entry: ENTRY.replace("hk_3f9a", "hk_new"), status: connected({ connectedAt: AT + 60_000, configHasHersir: false }) });
    answers.claudeDesktopConnect = () => again;
    const { got, button } = tree({ st: connected(), result: connection() });
    button("Connect again")[0].onClick();
    await settle();
    expect(calls).toEqual([["claudeDesktopConnect", [true]]]);
    expect(got[RESULT]).toEqual([again]);
    expect(got[ST]).toEqual([again.status]);
    calls.length = 0;
    tree({ st: connected(), ticked: false }).button("Connect again")[0].onClick();
    await settle();
    expect(calls).toEqual([["claudeDesktopConnect", [false]]]);
  });
});

describe("Disconnect asks first", () => {
  it("Disconnect opens the confirm dialog and calls nothing yet", () => {
    const { got, button } = tree({ st: connected() });
    button("Disconnect")[0].onClick();
    expect(got[ASKING]).toEqual([true]);
    expect(calls).toEqual([]);
  });

  it("the dialog has the card's title, text and buttons Disconnect and Cancel", () => {
    const html = render({ st: connected(), asking: true });
    expect(html).toContain('role="alertdialog" aria-modal="true"');
    expect(html).toContain(`<h2 id="cd-ask-q">${ASK_TITLE}</h2>`);
    expect(decode(/<p id="cd-ask-why">([^<]*)<\/p>/.exec(html)?.[1] ?? "")).toBe(ASK_TEXT);
    const dialog = html.slice(html.indexOf('class="ask"'));
    expect([...dialog.matchAll(/<button[^>]*>([^<]*)<\/button>/g)].map((m) => m[1]).sort()).toEqual(["Cancel", "Disconnect"]);
    expect(render({ st: connected() })).not.toContain("alertdialog");
  });

  it("Cancel, Escape and a click beside it close it and change nothing", () => {
    const first = tree({ st: connected(), result: connection(), asking: true });
    first.button("Cancel")[0].onClick();
    expect(first.got[ASKING]).toEqual([false]);

    const second = tree({ st: connected(), result: connection(), asking: true });
    const stop = { preventDefault: vi.fn(), stopPropagation: vi.fn() };
    second.dialog()!.onKeyDown({ key: "Escape", ...stop });
    expect(second.got[ASKING]).toEqual([false]);

    const third = tree({ st: connected(), result: connection(), asking: true });
    const target = {};
    third.scrim()!.onMouseDown({ target: {}, currentTarget: target }); // a click in the dialog
    expect(third.got[ASKING]).toEqual([]);
    third.scrim()!.onMouseDown({ target, currentTarget: target });
    expect(third.got[ASKING]).toEqual([false]);

    expect(calls).toEqual([]);
    for (const t of [first, second, third]) for (const i of [ST, TICKED, RESULT, BUSY, ERR]) expect(t.got[i]).toEqual([]);
  });

  it("Disconnect in the dialog calls claude_desktop_disconnect, and the section shows Not connected", async () => {
    const after = status({ configHasHersir: true });
    answers.claudeDesktopDisconnect = () => after;
    const { got, t } = tree({ st: connected(), result: connection(), asking: true });
    const confirm = findAll(t, (p) => p.className === "btn danger" && label(p) === "Disconnect") as Button[];
    expect(confirm).toHaveLength(1);
    confirm[0].onClick();
    await settle();
    expect(calls).toEqual([["claudeDesktopDisconnect", []]]);
    expect(got[ASKING]).toEqual([false]);
    expect(got[ST]).toEqual([after]);
    expect(got[RESULT]).toEqual([null]);
    expect(got[BUSY]).toEqual(["disconnect", ""]);

    const html = render({ st: after });
    expect(html).toContain('<button type="button" class="btn primary">Connect</button>');
    const text_ = text(html);
    for (const s of ["Connected since", "Connect again", "Disconnect", NO_ENTRY]) expect(text_).not.toContain(s);
  });
});

describe("a command that fails", () => {
  it("Connect: its error shows in the section and Connect works again", async () => {
    const why = "Hersir couldn't make a key: the database is busy";
    answers.claudeDesktopConnect = () => { throw why; };
    answers.claudeDesktopStatus = () => status();
    const { got, button } = tree();
    button("Connect")[0].onClick();
    await settle();
    expect(got[ERR]).toEqual([null, why]);
    expect(got[BUSY]).toEqual(["connect", ""]);
    expect(got[RESULT].filter((r) => typeof r !== "function")).toEqual([]);
    // it asks what is connected now
    expect(calls.map(([k]) => k)).toEqual(["claudeDesktopConnect", "claudeDesktopStatus"]);
    expect(got[ST]).toEqual([status()]);

    const html = render({ err: why });
    expect(html).toContain(`<span class="warn" role="alert">Hersir couldn&#x27;t make a key: the database is busy</span>`);
    expect(html).toContain('<button type="button" class="btn primary">Connect</button>');
    expect(html).toContain('<input type="checkbox" checked=""/>');
  });

  it("Disconnect: its error shows, the section stays connected, and Disconnect works again", async () => {
    const why = "Hersir couldn't end the key: the database is busy";
    answers.claudeDesktopDisconnect = () => { throw why; };
    answers.claudeDesktopStatus = () => connected();
    const { got, t } = tree({ st: connected(), asking: true });
    (findAll(t, (p) => p.className === "btn danger")[0] as Button).onClick();
    await settle();
    expect(got[ERR]).toEqual([null, why]);
    expect(got[BUSY]).toEqual(["disconnect", ""]);
    expect(got[ST]).toEqual([connected()]);
    const html = render({ st: connected(), err: why });
    expect(html).toContain(`<span class="warn" role="alert">Hersir couldn&#x27;t end the key: the database is busy</span>`);
    expect(html).toContain('<button type="button" class="btn ghost">Disconnect</button>');
    expect(html).toContain('<button type="button" class="btn">Connect again</button>');
  });

  it("the status: its error shows with Try again, which asks again", async () => {
    const why = "Hersir couldn't read Claude Desktop's folder";
    expect(text(render({ st: null, busy: "load" }))).toBe("Checking Claude Desktop…");
    const html = render({ st: null, busy: "", err: why });
    expect(html).toContain(`<span class="warn" role="alert">Hersir couldn&#x27;t read Claude Desktop&#x27;s folder</span>`);
    expect(html).toContain('<button type="button" class="btn sm">Try again</button>');
    answers.claudeDesktopStatus = () => status();
    const { got, button } = tree({ st: null, busy: "", err: why });
    button("Try again")[0].onClick();
    await settle();
    expect(calls).toEqual([["claudeDesktopStatus", []]]);
    expect(got[ST]).toEqual([status()]);
    expect(got[ERR]).toEqual([null]);
    expect(got[BUSY]).toEqual(["load", ""]);
  });
});

describe("loading and coming back to the window", () => {
  it("asks for the status when it opens", async () => {
    vi.stubGlobal("window", new EventTarget());
    answers.claudeDesktopStatus = () => status();
    const { got, fx } = tree({ st: null, busy: "load" });
    for (const f of fx) f();
    await settle();
    expect(calls).toEqual([["claudeDesktopStatus", []]]);
    expect(got[ST]).toEqual([status()]);
    expect(got[BUSY]).toEqual(["load", ""]);
  });

  it("asks again when the window gets focus, and keeps Connect's answer only while its connection is the one", async () => {
    const win = new EventTarget();
    vi.stubGlobal("window", win);
    const now = connected({ configHasHersir: true });
    answers.claudeDesktopStatus = () => now;
    const result = pasted();
    const { got, fx } = tree({ st: connected({ configHasHersir: false }), result });
    const stops = fx.map((f) => f()).filter((s): s is () => void => typeof s === "function");
    // opening asks once; then only the focus
    await settle();
    calls.length = 0;
    for (const g of got) g.length = 0;
    win.dispatchEvent(new Event("focus"));
    await settle();
    expect(calls).toEqual([["claudeDesktopStatus", []]]);
    expect(got[ST]).toEqual([now]);
    const keep = got[RESULT][0] as (r: ClaudeDesktopConnection | null) => ClaudeDesktopConnection | null;
    expect(keep(result)).toBe(result);
    expect(keep(pasted({ status: connected({ connectedAt: AT - 1 }) }))).toBeNull();
    for (const s of stops) s();
    win.dispatchEvent(new Event("focus"));
    await settle();
    expect(calls).toHaveLength(1);
  });
});
