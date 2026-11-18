// GA-314 QA: the Hold row of the Card tab beside the Chief's chat (ChatCard's HoldRow). Read in comments, shown when the
// reason was shortened (as before), now opens the card's own page on its Comments tab: at the latest comment whose text
// holds the hold's reason (openCommentOnPage), else at the top of its comments (openCommentsOnPage, CARD_THREAD), and the
// Card tab stays open in the chat. Clear hold (btn sm, its title) shows on every held card, also With the Chief and the
// waiting-for-OK hold, and does what Clear hold on the card's page does (updateTask(id, { hold: "" })): disabled while it
// saves, the row goes once the card comes back without its hold (no reload), the foot then shows Run; a failure shows in
// red under the row (role="alert") and the hold stays.
//
// React itself renders the real ChatCardBody (react-dom/client) into a small stand-in DOM, as in ../BoardBulk.test.tsx: the
// project has no DOM library. The api is mocked (in the style of HeldCards.test.tsx), and so is useData: it reads once, and
// again on `write()`, which stands for the backend's notice that the card changed. The router's go and reveal record
// where the page would open. MarkdownView, MarkdownEditor and PullPanel are stand-ins.
import { act } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Comment, Run, Task, Team } from "../../types";

// ---- a stand-in DOM, as much of it as React DOM uses (from ../BoardBulk.test.tsx) ----
const HTML = "http://www.w3.org/1999/xhtml";
type Listener = { f: (e: unknown) => void; capture: boolean };
class FakeNode {
  childNodes: FakeNode[] = [];
  parentNode: FakeNode | null = null;
  attrs = new Map<string, string>();
  listeners = new Map<string, Listener[]>();
  style: Record<string, unknown> = { setProperty(k: string, v: string) { (this as Record<string, unknown>)[k] = v; }, removeProperty() {} };
  nodeValue: string | null = null;
  dataset: Record<string, string> = {};
  declare value?: string;
  declare type?: string;
  declare disabled?: boolean;
  constructor(public nodeType: number, public nodeName: string, public ownerDocument: FakeNode | null, public namespaceURI = HTML) {}
  get tagName() { return this.nodeName; }
  get firstChild(): FakeNode | null { return this.childNodes[0] ?? null; }
  get lastChild(): FakeNode | null { return this.childNodes[this.childNodes.length - 1] ?? null; }
  get nextSibling(): FakeNode | null { const p = this.parentNode; return p ? p.childNodes[p.childNodes.indexOf(this) + 1] ?? null : null; }
  get textContent(): string { return this.nodeType === 3 ? this.nodeValue ?? "" : this.childNodes.map((c) => c.textContent).join(""); }
  set textContent(t: string) {
    for (const c of this.childNodes) c.parentNode = null;
    this.childNodes = [];
    if (t) this.appendChild(doc.createTextNode(t));
  }
  appendChild(c: FakeNode) { return this.insertBefore(c, null); }
  insertBefore(c: FakeNode, ref: FakeNode | null) {
    c.parentNode?.removeChild(c);
    const i = ref ? this.childNodes.indexOf(ref) : -1;
    if (i < 0) this.childNodes.push(c); else this.childNodes.splice(i, 0, c);
    c.parentNode = this;
    return c;
  }
  removeChild(c: FakeNode) { this.childNodes.splice(this.childNodes.indexOf(c), 1); c.parentNode = null; return c; }
  setAttribute(k: string, v: unknown) { this.attrs.set(k, String(v)); }
  setAttributeNS(_ns: string, k: string, v: unknown) { this.attrs.set(k, String(v)); }
  removeAttribute(k: string) { this.attrs.delete(k); }
  removeAttributeNS(_ns: string, k: string) { this.attrs.delete(k); }
  getAttribute(k: string) { return this.attrs.get(k) ?? null; }
  hasAttribute(k: string) { return this.attrs.has(k); }
  addEventListener(type: string, f: (e: unknown) => void, opt?: boolean | { capture?: boolean }) {
    const capture = typeof opt === "boolean" ? opt : !!opt?.capture;
    this.listeners.set(type, [...(this.listeners.get(type) ?? []), { f, capture }]);
  }
  removeEventListener(type: string, f: (e: unknown) => void) { this.listeners.set(type, (this.listeners.get(type) ?? []).filter((l) => l.f !== f)); }
  contains(n: FakeNode | null) { for (let x = n; x; x = x.parentNode) if (x === this) return true; return false; }
  closest() { return null; }
  scrollIntoView() {}
  focus() {}
  blur() {}
  getBoundingClientRect() { return { top: 0, left: 0, right: 0, bottom: 0, width: 0, height: 0 }; }
  all(): FakeNode[] { return [this, ...this.childNodes.flatMap((c) => c.all())]; }
  createElement(tag: string) { return new FakeNode(1, tag.toUpperCase(), this); }
  createElementNS(ns: string, tag: string) { return new FakeNode(1, tag, this, ns); }
  createTextNode(t: string) { const n = new FakeNode(3, "#text", this); n.nodeValue = t; return n; }
  getElementById(id: string) { return this.all().find((n) => n.getAttribute("id") === id) ?? null; }
}
const doc = new FakeNode(9, "#document", null);
const body = doc.createElement("body");
doc.appendChild(body);
(doc as unknown as { oninput: null }).oninput = null;
const win = { document: doc, event: undefined, HTMLIFrameElement: class {}, devicePixelRatio: 1, addEventListener() {}, removeEventListener() {},
  matchMedia: () => ({ matches: false, addEventListener() {}, removeEventListener() {} }) };
Object.assign(doc, { body, documentElement: body, activeElement: body, defaultView: win, hidden: false });
vi.stubGlobal("window", win);
vi.stubGlobal("document", doc);
vi.stubGlobal("requestAnimationFrame", (f: () => void) => { f(); return 0; });
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

/** A click at `target`, through the listeners on the way as a browser calls them (capture down, then bubble up). */
function fire(target: FakeNode, type: string) {
  const path: FakeNode[] = [];
  for (let x: FakeNode | null = target; x; x = x.parentNode) path.push(x);
  let stopped = false;
  const e = { type, target, bubbles: true, button: 0, defaultPrevented: false, timeStamp: 0, shiftKey: false, ctrlKey: false, metaKey: false, altKey: false,
    preventDefault() { e.defaultPrevented = true; }, stopPropagation() { stopped = true; } };
  for (const n of [...path].reverse()) for (const l of n.listeners.get(type) ?? []) if (l.capture && !stopped) l.f(e);
  for (const n of path) for (const l of n.listeners.get(type) ?? []) if (!l.capture && !stopped) l.f(e);
}

// ---- the api: the card, its comments and runs as the backend has them now; what was called, in order ----
const api = vi.hoisted(() => ({
  task: null as Task | null, comments: [] as Comment[], runs: [] as Run[], calls: [] as unknown[][],
  failUpdate: null as string | null, suggestAsked: 0,
}));
vi.mock("../../api", async (orig) => {
  const real = await orig<Record<string, unknown>>();
  const out: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(real)) out[k] = typeof v !== "function" ? v : () => new Promise(() => {});
  out.itemId = async (_kind: string, identifier: string) => (identifier === api.task?.identifier ? api.task.id : null);
  out.getTask = async () => ({ ...api.task });
  out.listComments = async () => [...api.comments];
  out.listRuns = async () => [...api.runs];
  out.getProject = async () => ({ id: "p1", name: "Docs", teamId: "team1" });
  out.getTeam = async () => team;
  out.onRunsChanged = async () => () => {};
  out.cardChanges = async () => null;
  // as runs::choose_agent: a held card has no agent to run it
  out.suggestAgent = async () => { api.suggestAsked += 1; if (api.task?.hold) throw new Error("On hold"); return "a-dev"; };
  // Clear hold on the card's page: updateTask(task.id, { hold: "" }); the backend takes the hold, its reason and kind off
  out.updateTask = async (...a: unknown[]) => {
    api.calls.push(["updateTask", ...a]);
    if (api.failUpdate) throw new Error(api.failUpdate);
    const patch = a[1] as { hold?: string };
    if (patch.hold === "" && api.task) api.task = { ...api.task, hold: null, holdReason: null, holdKind: null, holdAt: null, withLead: false };
  };
  return out;
});
// useData: reads on mount and when its deps change, and again on write() (the backend's notice that a row changed)
const reads = vi.hoisted(() => ({ reloads: new Set<() => void>() }));
vi.mock("../../lib/useData", async () => {
  const R = await import("react");
  return {
    useData: <T,>(fn: () => Promise<T>, deps: unknown[]) => {
      const [state, setState] = R.useState<{ data?: T; error?: unknown }>({});
      const [n, setN] = R.useState(0);
      R.useEffect(() => { const f = () => setN((x) => x + 1); reads.reloads.add(f); return () => { reads.reloads.delete(f); }; }, []);
      R.useEffect(() => {
        let alive = true;
        fn().then((d) => { if (alive) setState({ data: d }); }, (e) => { if (alive) setState({ error: e }); });
        return () => { alive = false; };
      }, [...deps, n]); // eslint-disable-line react-hooks/exhaustive-deps
      return { data: state.data, error: state.error ?? null };
    },
  };
});
vi.mock("../../lib/useLiveRuns", () => ({ useLiveRuns: () => [] }));
// where the card's page would open
const page = vi.hoisted(() => ({ log: [] as unknown[][] }));
vi.mock("../../router", async (orig) => ({ ...(await orig<Record<string, unknown>>()), go: (r: unknown) => { page.log.push(["go", r]); } }));
vi.mock("../../lib/reveal", () => ({ reveal: (id: string) => { page.log.push(["reveal", id]); } }));
vi.mock("../MarkdownView", async () => {
  const R = await import("react");
  return { MarkdownView: ({ md }: { md: string }) => R.createElement("div", { className: "md" }, md) };
});
vi.mock("../MarkdownEditor", () => ({ MarkdownEditor: () => null }));
vi.mock("../PullPanel", () => ({ PullPanel: () => null }));

const { createRoot } = await import("react-dom/client");
const { ChatCardBody } = await import("./ChatCard");
const { CARD_THREAD, openCommentOnPage, openCommentsOnPage } = await import("../../lib/cardPage");
const { reasonComment, REASON_CHARS } = await import("../../lib/chatCard");
const { textEnd } = await import("../../lib/format");
// node:fs through a computed specifier: the build type-checks tests without Node's types (as editorContract.test.ts)
const fs = await import(/* @vite-ignore */ ["node", "fs"].join(":"));
const readFileSync = (u: URL, enc: "utf8"): string => fs.readFileSync(u, enc);
const { openSideTabs, showChat, sideNow, tabKey } = await import("../../lib/chatSide");

// ---- the card ----
const team = {
  id: "team1", name: "Team", labels: [],
  states: [
    { id: "s-todo", name: "To do", category: "unstarted", sortKey: "a1" },
    { id: "s-prog", name: "In progress", category: "started", sortKey: "a2" },
    { id: "s-test", name: "Testing", category: "testing", sortKey: "a3" },
    { id: "s-review", name: "Review", category: "review", sortKey: "a4" },
  ],
  members: [{ actorId: "a-dev", name: "Frontend Agent", kind: "agent", status: "active", roleKey: "frontend", handle: "frontend", isLead: false, allowedTools: [], chatEnabled: false }],
} as unknown as Team;
const REASON = "Which of the two exports should go first: the CSV for the bookkeeper or the Exact Online import file?";
const card = (o: Partial<Task> = {}): Task => ({
  id: "t45", identifier: "DU-45", projectId: "p1", projectName: "Docs", title: "Docx import", descriptionMd: "", acceptanceMd: "",
  stateId: "s-prog", stateName: "In progress", stateCategory: "started", priority: 0, labels: [], sortKey: "a", bounceCount: 0, failCount: 0,
  hold: "needs_decision", holdReason: REASON, holdAt: 1_000, holdKind: null, withLead: false, ...o,
} as Task);
const comment = (id: string, bodyMd: string, at: number): Comment => ({ id, authorId: "qa", authorName: "QA Agent", authorKind: "agent", bodyMd, createdAt: at });
const run = (o: Partial<Run> = {}): Run => ({
  id: "r1", taskId: "t45", agentId: "a-dev", agentName: "Frontend Agent", status: "cancelled", outcome: null, createdAt: 500, endedAt: 900,
  sessionId: "s1", worktreePath: "/w/DU-45", costUsdMicros: 1200, ...o,
} as unknown as Run);

// ---- rendering ----
let container: FakeNode;
let root: ReturnType<typeof createRoot> | null = null;
const settle = async () => { for (let i = 0; i < 6; i++) await act(async () => { await Promise.resolve(); }); };
async function draw(t: Task, comments: Comment[] = [], runs: Run[] = []) {
  api.task = t; api.comments = comments; api.runs = runs;
  // a fresh tab each time: the card is read anew
  act(() => { root!.render(null); });
  act(() => { root!.render(<ChatCardBody identifier={t.identifier} />); });
  await settle();
}
/** The backend's notice that the card changed: every read on screen reads again (useData), with no reload of the page. */
const write = async () => { await act(async () => { for (const f of [...reads.reloads]) f(); }); await settle(); };
const click = async (n: FakeNode | undefined) => { expect(n, "the element to click").toBeTruthy(); act(() => { fire(n!, "click"); }); await settle(); };

beforeEach(() => {
  api.calls = []; api.failUpdate = null; api.suggestAsked = 0; page.log = [];
  container = doc.createElement("div");
  body.appendChild(container);
  root = createRoot(container as unknown as HTMLElement);
});
afterEach(async () => {
  await act(async () => { root!.unmount(); });
  root = null;
  container.parentNode?.removeChild(container);
});

const cls = (n: FakeNode) => (n.getAttribute("class") ?? "").split(" ");
const props = () => container.all().filter((n) => n.nodeName === "DT").map((n) => n.textContent);
const holdRow = () => container.all().find((n) => n.nodeName === "DD" && cls(n).includes("hold"));
const buttons = (root: FakeNode | undefined) => (root?.all() ?? []).filter((n) => n.nodeName === "BUTTON");
const buttonIn = (root: FakeNode | undefined, text: string) => buttons(root).find((b) => b.textContent.trim() === text);
const readIn = () => buttonIn(holdRow(), "Read in comments");
const clearHold = () => buttons(holdRow()).find((b) => b.getAttribute("name") === "clear" || /Clear hold|Clearing…/.test(b.textContent));
const disabled = (b: FakeNode | undefined) => !!b && (b.disabled === true || b.hasAttribute("disabled"));
const foot = () => container.all().find((n) => cls(n).includes("chat-card-foot"));
const footButtons = () => buttons(foot()).map((b) => b.textContent.trim());
/** The Hold row's parts, in order: the badge, the reason, its link, the button, an error. */
const rowParts = () => (holdRow()?.childNodes ?? []).map((n) => {
  const c = cls(n);
  if (c.includes("badge")) return "badge";
  if (c.includes("reason")) return "reason";
  if (c.includes("link")) return n.textContent;
  if (c.includes("err")) return "error";
  return n.textContent.trim();
});
const TITLE = "Takes the hold off, so its column's agents can start it";

// ---- Read in comments ----

describe("Read in comments in the Card tab's Hold row (GA-314)", () => {
  const thread = [
    comment("c1", "First comment: the export should follow the bookkeeper's sheet.", 1),
    comment("c2", `From QA, earlier: ${REASON}`, 2),
    comment("c3", `Third comment, from QA: ${REASON}`, 3),
    comment("c4", "A later comment without the reason.", 4),
  ];

  it("shows, as before, only when the reason was shortened: the reason's last 60 characters, then the link", async () => {
    await draw(card(), thread);
    const reason = holdRow()!.all().find((n) => cls(n).includes("reason"))!;
    expect(reason.textContent).toBe(textEnd(REASON, REASON_CHARS));
    expect(reason.textContent.startsWith("…") && REASON.endsWith(reason.textContent.slice(1)) && reason.textContent.length <= REASON_CHARS + 1).toBe(true);
    expect(reason.getAttribute("title")).toBe(REASON);
    expect(readIn()?.getAttribute("class")).toBe("link");
    expect(readIn()?.getAttribute("type")).toBe("button");
    // a reason of 60 characters or fewer shows whole, with no link
    await draw(card({ holdReason: "Pick the CSV or the Exact Online file." }), thread);
    expect(holdRow()!.all().find((n) => cls(n).includes("reason"))!.textContent).toBe("Pick the CSV or the Exact Online file.");
    expect(holdRow()!.all().find((n) => cls(n).includes("reason"))!.getAttribute("title")).toBeNull();
    expect(readIn()).toBeUndefined();
    // a hold without a reason: no reason, no link
    await draw(card({ holdReason: null }), thread);
    expect(rowParts()).toEqual(["badge", "Clear hold"]);
  });

  it("opens the card's page on its Comments tab at the latest comment that holds the reason", async () => {
    await draw(card(), thread);
    await click(readIn());
    expect(page.log).toEqual([["go", { page: "task", id: "t45" }], ["reveal", "comment-c3"]]);
  });

  it("goes by the whole reason, trimmed, as the card's page does", async () => {
    await draw(card({ holdReason: `  ${REASON}\n` }), thread.slice(0, 2));
    await click(readIn());
    expect(page.log).toEqual([["go", { page: "task", id: "t45" }], ["reveal", "comment-c2"]]);
  });

  it("with the reason in no comment (the waiting-for-OK hold on a card without comments), opens the page at the top of its comments", async () => {
    const waiting = card({ hold: "needs_decision", holdKind: "waiting_for_ok",
      holdReason: "DU-45 waits on hold for your OK: this chat used new page on the chrome-devtools server, a tool from outside Hersir." });
    await draw(waiting, []);
    expect(container.all().some((n) => n.textContent === "No comments yet.")).toBe(true);
    await click(readIn());
    expect(page.log).toEqual([["go", { page: "task", id: "t45" }], ["reveal", CARD_THREAD]]);
    // comments, but none that holds the reason: the same
    page.log = [];
    await draw(waiting, [thread[0], thread[3]]);
    await click(readIn());
    expect(page.log).toEqual([["go", { page: "task", id: "t45" }], ["reveal", "card-thread"]]);
  });

  it("leaves the Card tab open in the chat and the tab as it was (no comments unfolded)", async () => {
    showChat("c1");
    openSideTabs([{ kind: "card", id: "DU-45" }], "c1");
    await draw(card(), thread);
    expect(footButtons()).toContain("Comment");
    const showAll = () => buttons(container).filter((b) => b.textContent === "Show all 4" || b.textContent === "Show less").map((b) => b.textContent);
    expect(showAll()).toEqual(["Show all 4"]);
    await click(readIn());
    expect(sideNow("c1").tabs.map(tabKey)).toEqual(["card:DU-45"]);
    expect(sideNow("c1").current).toBe("card:DU-45");
    expect(holdRow()).toBeTruthy();
    expect(showAll()).toEqual(["Show all 4"]);
  });
});

describe("the card's page helpers (lib/cardPage)", () => {
  it("openCommentOnPage opens the page and brings the comment into sight; openCommentsOnPage the top of its comments", () => {
    page.log = [];
    openCommentOnPage("t9", "c7");
    openCommentsOnPage("t9");
    expect(page.log).toEqual([["go", { page: "task", id: "t9" }], ["reveal", "comment-c7"], ["go", { page: "task", id: "t9" }], ["reveal", "card-thread"]]);
  });

  it("the card's page gives its Comments, Activity and Runs tabs row that id once its comments have loaded", () => {
    const src = readFileSync(new URL("../../pages/TaskPage.tsx", import.meta.url), "utf8");
    expect(src).toMatch(/<div className="tabs" role="tablist" ref=\{thread\} id=\{comments \? CARD_THREAD : undefined\}>/);
  });

  it("reasonComment: the latest comment holding the trimmed reason, else none", () => {
    const cs = [{ id: "a", bodyMd: "x REASON y" }, { id: "b", bodyMd: "REASON again" }, { id: "c", bodyMd: "other" }];
    expect(reasonComment("REASON", cs)?.id).toBe("b");
    expect(reasonComment("  REASON \n", cs)?.id).toBe("b");
    expect(reasonComment("missing", cs)).toBeUndefined();
    expect(reasonComment("", cs)).toBeUndefined();
    expect(reasonComment("   ", cs)).toBeUndefined();
    expect(reasonComment(null, cs)).toBeUndefined();
    expect(reasonComment("REASON", null)).toBeUndefined();
    expect(reasonComment("REASON", [])).toBeUndefined();
  });
});

// ---- Clear hold ----

describe("Clear hold in the Card tab's Hold row (GA-314)", () => {
  const holds: [string, Partial<Task>, string][] = [
    ["Needs your decision", { hold: "needs_decision" }, "badge needs"],
    ["Blocked", { hold: "blocked" }, "badge needs"],
    ["Stalled", { hold: "stalled" }, "badge needs"],
    ["Merge conflict", { hold: "merge_conflict" }, "badge needs"],
    ["Waiting for approval", { hold: "waiting_approval" }, "badge needs"],
    ["Rate limited", { hold: "rate_limited" }, "badge needs"],
    ["With the Chief", { hold: "needs_decision", withLead: true }, "badge live"],
    ["Needs your decision", { hold: "needs_decision", holdKind: "waiting_for_ok" }, "badge needs"],
  ];

  it.each(holds)("shows on a card on hold (%s, %o): btn sm, its title, under the reason and after Read in comments", async (badge, o, badgeCls) => {
    await draw(card(o));
    expect(props()).toEqual(["Column", "Hold", "Assignee"]);
    const b = clearHold()!;
    expect(b.textContent).toBe("Clear hold");
    expect(b.getAttribute("class")).toBe("btn sm");
    expect(b.getAttribute("title")).toBe(TITLE);
    expect(disabled(b)).toBe(false);
    expect(rowParts()).toEqual(["badge", "reason", "Read in comments", "Clear hold"]);
    const badgeEl = holdRow()!.childNodes[0];
    expect(badgeEl.getAttribute("class")).toBe(badgeCls);
    expect(badgeEl.textContent).toBe(badge);
    expect(buttons(holdRow()).filter((x) => x.textContent === "Clear hold")).toHaveLength(1);
  });

  it("shows with a short reason (no Read in comments) right after the reason", async () => {
    await draw(card({ holdReason: "Pick one." }));
    expect(rowParts()).toEqual(["badge", "reason", "Clear hold"]);
  });

  it("isn't there without a hold: no Hold row at all", async () => {
    await draw(card({ hold: null, holdReason: null }));
    expect(props()).toEqual(["Column", "Assignee"]);
    expect(clearHold()).toBeUndefined();
    expect(buttons(container).some((b) => b.textContent === "Clear hold")).toBe(false);
  });

  it("takes the hold off as Clear hold on the card's page does, disabled while it saves; the row goes without a reload and Run shows", async () => {
    await draw(card(), [comment("c3", `Third comment, from QA: ${REASON}`, 3)]);
    // held: no agent to run it, so the foot has Comment and Move to
    expect(footButtons()).toEqual(["Comment", "Move to"]);
    const asked = api.suggestAsked;
    await click(clearHold());
    expect(api.calls).toEqual([["updateTask", "t45", { hold: "" }]]);
    // until the card comes back without its hold: busy, disabled, and a second click does nothing
    const busy = clearHold()!;
    expect(busy.textContent).toBe("Clearing…");
    expect(disabled(busy)).toBe(true);
    expect(busy.getAttribute("aria-busy")).toBe("true");
    await click(busy);
    expect(api.calls).toHaveLength(1);
    expect(holdRow()).toBeTruthy();
    // the backend's notice: the card reads again, its hold is gone, and with it the row
    await write();
    expect(holdRow()).toBeUndefined();
    expect(props()).toEqual(["Column", "Assignee"]);
    expect(container.all().some((n) => n.getAttribute("role") === "alert")).toBe(false);
    // the foot asks which agent would run it again, and shows Run where the card's page would
    expect(api.suggestAsked).toBeGreaterThan(asked);
    expect(footButtons()).toEqual(["Comment", "Run", "Move to"]);
    expect(buttonIn(foot(), "Run")?.getAttribute("title")).toBe("Run starts Frontend Agent");
    expect(api.calls).toEqual([["updateTask", "t45", { hold: "" }]]);
  });

  it("works on a card that waits for your OK after the Chief used a tool from outside Hersir", async () => {
    await draw(card({ hold: "needs_decision", holdKind: "waiting_for_ok", stateId: "s-todo", stateName: "To do", stateCategory: "unstarted",
      holdReason: "DU-45 waits on hold for your OK: this chat used new page on the chrome-devtools server, a tool from outside Hersir." }));
    await click(clearHold());
    expect(api.calls).toEqual([["updateTask", "t45", { hold: "" }]]);
    await write();
    expect(holdRow()).toBeUndefined();
    expect(api.task?.holdKind).toBeNull();
    expect(footButtons()).toEqual(["Comment", "Run", "Move to"]);
  });

  it("works With the Chief too", async () => {
    await draw(card({ withLead: true }));
    await click(clearHold());
    expect(api.calls).toEqual([["updateTask", "t45", { hold: "" }]]);
    await write();
    expect(holdRow()).toBeUndefined();
  });

  it("when taking the hold off fails, the error shows under the row (role=alert) and the hold stays; it can be tried again", async () => {
    api.failUpdate = "The card is gone";
    await draw(card());
    await click(clearHold());
    expect(api.calls).toEqual([["updateTask", "t45", { hold: "" }]]);
    const alert = holdRow()!.all().find((n) => n.getAttribute("role") === "alert");
    expect(alert?.nodeName).toBe("P");
    expect(alert?.getAttribute("class")).toBe("err t-small");
    // "Error: " is left off, as the foot's errors do
    expect(alert?.textContent).toBe("The card is gone");
    expect(rowParts()).toEqual(["badge", "reason", "Read in comments", "Clear hold", "error"]);
    // the hold stays, and the button can be clicked again
    await write();
    expect(holdRow()).toBeTruthy();
    expect(clearHold()?.textContent).toBe("Clear hold");
    expect(disabled(clearHold())).toBe(false);
    expect(footButtons()).toEqual(["Comment", "Move to"]);
    // a second try that works: the error goes with the click, and the row once the hold is off
    api.failUpdate = null;
    await click(clearHold());
    expect(holdRow()?.all().some((n) => n.getAttribute("role") === "alert")).toBe(false);
    expect(api.calls).toHaveLength(2);
    await write();
    expect(holdRow()).toBeUndefined();
  });

  it("shows the error in danger red (terminal.css)", () => {
    const css = readFileSync(new URL("../../styles/terminal.css", import.meta.url), "utf8");
    expect(css).toContain(".chat-card-props dd.hold .err { margin: 0; color: var(--danger); }");
  });
});

// ---- unchanged ----

describe("the rest of the tab, unchanged", () => {
  it("a held card whose last run can go on still offers Continue, which also clears the hold", async () => {
    await draw(card(), [], [run()]);
    expect(footButtons()).toEqual(["Comment", "Continue", "Move to"]);
    expect(buttonIn(foot(), "Continue")?.getAttribute("title")).toBe("Continue picks up Frontend Agent's session where it stopped and clears the hold");
  });

  it("in Review: no Run or Continue, Clear hold still in the Hold row", async () => {
    await draw(card({ stateId: "s-review", stateName: "Review", stateCategory: "review" }));
    expect(footButtons()).toEqual(["Comment", "Move to"]);
    expect(clearHold()?.textContent).toBe("Clear hold");
  });

  it("not on hold: Run where the card's page would", async () => {
    await draw(card({ hold: null, holdReason: null }));
    expect(footButtons()).toEqual(["Comment", "Run", "Move to"]);
  });
});
