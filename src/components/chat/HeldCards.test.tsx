// GA-312 QA: the cards a Chief answer put on the waiting-for-OK hold (GA-101), under that answer in "Waiting for your OK":
// identifier and title (a card chip), its column and OK, which takes the hold off as Clear hold on the card does
// (updateTask(id, { hold: "" })). The section shows when only cards wait; Approve all shows when two or more things wait
// there, runs the waiting calls and then takes the listed cards' holds off. And the line above the text box of a chat that
// used a tool from outside Hersir, with New chat. Rendered to HTML on the server with the conversation handed in; the
// buttons' handlers are called from the elements PendingActions returns (usePending is a stand-in that runs the call).
import { describe, expect, it, vi } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import type { ReactElement, ReactNode } from "react";
import type { ChatMessage, ChatThread as Thread, Member, PendingAction, Task } from "../../types";

// the api: what was called, in order; updateTask and approvePendingActions answer, the rest never settles
const api = vi.hoisted(() => ({ calls: [] as unknown[][], approved: [] as unknown[], failApprove: null as string | null }));
vi.mock("../../api", async (orig) => {
  const real = await orig<Record<string, unknown>>();
  const out: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(real)) out[k] = typeof v !== "function" ? v : () => new Promise(() => {});
  out.updateTask = async (...a: unknown[]) => { api.calls.push(["updateTask", ...a]); };
  out.approvePendingActions = async (...a: unknown[]) => {
    api.calls.push(["approvePendingActions", ...a]);
    if (api.failApprove) throw api.failApprove;
    return api.approved;
  };
  return out;
});
// usePending: not busy; act runs the call at once and keeps what it did, and `done` is kept to ask when a button stops spinning
const busy = vi.hoisted(() => ({ acts: [] as string[], runs: [] as Promise<unknown>[], done: null as null | ((k: string, v: unknown) => boolean), scope: undefined as unknown }));
vi.mock("../../lib/usePending", () => ({
  usePending: (done: (k: string, v: unknown) => boolean, scope?: unknown) => {
    busy.done = done; busy.scope = scope;
    return { busy: null, act: (k: string, call: () => Promise<unknown>, onError: (e: unknown) => void) => {
      busy.acts.push(k); busy.runs.push(call().catch((e) => { onError(e); return undefined; }));
    } };
  },
}));
const chat = vi.hoisted(() => ({ messages: [] as ChatMessage[], pending: [] as PendingAction[] }));
vi.mock("./useChat", () => ({
  useChat: () => ({ messages: chat.messages, draft: "", tool: null, working: false, queue: [], pending: chat.pending, error: null, setWorking: () => {} }),
}));
// the cards as they are now: those on the waiting-for-OK hold (useHeldCards' own filter is tested in useHeldCards.test.ts)
const board = vi.hoisted(() => ({ waiting: new Map<string, unknown>(), asked: [] as string[][] }));
vi.mock("./useHeldCards", () => ({
  useHeldCards: (ids: string[]) => { board.asked.push(ids); return new Map(ids.filter((id) => board.waiting.has(id)).map((id) => [id, board.waiting.get(id)])); },
}));

const { PendingActions } = await import("./PendingActions");
const { ChatThread } = await import("./ChatThread");
const { heldCards, placeHeld } = await import("../../lib/pending");
const { groupMessages } = await import("../../lib/chat");

const OUTSIDE = "mcp__chrome-devtools__new_page";
const card = (n: number, o: Partial<Task> = {}): Task => ({
  id: `t${n}`, identifier: `DU-${n}`, title: `Card ${n}`, descriptionMd: "", stateId: "todo", stateName: "To do", stateCategory: "unstarted",
  priority: 0, labels: [], hold: "needs_decision", holdKind: "waiting_for_ok", bounceCount: 0, failCount: 0, sortKey: "a", ...o,
} as Task);
const action = (id: string, o: Partial<PendingAction> = {}): PendingAction => ({
  id, threadId: "c1", runId: "r2", actorId: "lead", tool: "memory_append", args: {}, outsideTool: OUTSIDE, status: "pending", createdAt: 0,
  preview: { line: "Add to the note Chief/Notes", fields: [] }, ...o,
});
const text = (html: string) => html.replace(/<!-- -->/g, "");
const count = (html: string, s: string) => html.split(s).length - 1;

/** Every element in a tree a component returned, as it returned it (components inside aren't called). */
function elements(node: ReactNode): ReactElement<Record<string, unknown>>[] {
  const out: ReactElement<Record<string, unknown>>[] = [];
  const walk = (n: unknown) => {
    if (Array.isArray(n)) { n.forEach(walk); return; }
    if (!n || typeof n !== "object" || !("props" in n)) return;
    const el = n as ReactElement<Record<string, unknown>>;
    out.push(el);
    walk(el.props.children);
  };
  walk(node);
  return out;
}
const button = (tree: ReactNode, name: string) => {
  const b = elements(tree).find((e) => e.props.name === name);
  if (!b) throw new Error(`no button ${name}`);
  return b;
};
const click = async (tree: ReactNode, name: string) => {
  (button(tree, name).props.onClick as () => void)();
  await Promise.all(busy.runs);
};
const reset = () => { api.calls = []; api.approved = []; api.failApprove = null; busy.acts = []; busy.runs = []; };
const call = (o: Parameters<typeof PendingActions>[0]) => PendingActions(o) as ReactNode;

// ---- the list under one answer ----

describe("cards an answer put on hold, in Waiting for your OK", () => {
  const three = [card(37, { title: "Docx import" }), card(38, { title: "Docx export" }), card(39, { title: "Mail merge", stateName: "In progress" })];
  const html = renderToStaticMarkup(<PendingActions actions={[]} cards={three} outsideTool={OUTSIDE} threadId="c1" onError={() => {}} />);

  it("shows the section when only cards wait, and says why in words", () => {
    expect(html).toContain('aria-label="Waiting for your OK"');
    expect(html).toContain("Waiting for your OK</span>");
    expect(html).toContain("This chat used new page on the chrome-devtools server, a tool from outside Hersir, so no agent starts these cards until you OK them.");
    expect(html).not.toContain("didn&#x27;t run");
  });

  it("lists each card with its identifier and title as a card chip, its column and an OK", () => {
    const t = text(html);
    for (const [id, title, column] of [["DU-37", "Docx import", "To do"], ["DU-38", "Docx export", "To do"], ["DU-39", "Mail merge", "In progress"]]) {
      expect(t).toMatch(new RegExp(`<div class="pa-card pa-held" role="group" aria-label="${id} waits on hold"><div class="pa-line">`
        + `<span class="pa-card-name"><a class="item-chip kind-task" href="gizai:task/${id}"[^>]*>${id} ${title}</a></span>`
        + `<span class="pa-column" title="Its column">${column}</span>`));
    }
    expect(count(html, ">OK</button>")).toBe(3);
    expect(count(html, 'title="Takes the hold off, so its column&#x27;s agents can start it"')).toBe(3);
    expect(t.indexOf("DU-37")).toBeLessThan(t.indexOf("DU-38"));
    expect(t.indexOf("DU-38")).toBeLessThan(t.indexOf("DU-39"));
    expect(html).not.toContain(">Approve</button>");
    expect(html).not.toContain(">Discard</button>");
  });

  it("has Approve all for two or more cards, and none for one", () => {
    expect(count(html, ">Approve all</button>")).toBe(1);
    expect(html).toContain('title="Takes each card&#x27;s hold off, so its column&#x27;s agents can start it"');
    const one = renderToStaticMarkup(<PendingActions actions={[]} cards={[card(37)]} outsideTool={OUTSIDE} threadId="c1" onError={() => {}} />);
    expect(one).not.toContain("Approve all");
    expect(one).toContain("so no agent starts this card until you OK it.");
    expect(count(one, ">OK</button>")).toBe(1);
  });

  it("with one call and one card, two things wait: Approve all shows, and the line names both", () => {
    const h = renderToStaticMarkup(<PendingActions actions={[action("p1")]} cards={[card(40)]} outsideTool={OUTSIDE} threadId="c1" onError={() => {}} />);
    expect(count(h, ">Approve all</button>")).toBe(1);
    expect(h).toContain('title="Runs each call, in this order, then takes the cards&#x27; holds off"');
    expect(h).toContain("so Hersir didn&#x27;t run this call, and no agent starts this card until you OK it.");
    expect(count(h, ">Approve</button>")).toBe(1);
    expect(count(h, ">Discard</button>")).toBe(1);
    expect(count(h, ">OK</button>")).toBe(1);
    // the call comes first, then the card
    expect(h.indexOf("Add to the note Chief/Notes")).toBeLessThan(h.indexOf("DU-40"));
  });

  it("is as before for calls only (GA-102): same line, same Approve all", () => {
    const h = renderToStaticMarkup(<PendingActions actions={[action("p1"), action("p2", { tool: "memory_write" })]} onError={() => {}} />);
    expect(h).toContain("This chat used new page on the chrome-devtools server, a tool from outside Hersir, so Hersir didn&#x27;t run these.");
    expect(h).toContain('title="Runs each of these, in this order"');
    expect(h).not.toContain("pa-held");
  });
});

describe("OK and Approve all", () => {
  it("OK on one card takes its hold off as Clear hold does, and only its own", async () => {
    reset();
    const cards = [card(37), card(38), card(39)];
    const tree = call({ actions: [], cards, outsideTool: OUTSIDE, threadId: "c1", onError: () => {} });
    await click(tree, "ok:t38");
    // Clear hold on the card: updateTask(task.id, { hold: "" }) (Properties.tsx)
    expect(api.calls).toEqual([["updateTask", "t38", { hold: "" }]]);
    // the button spins until the card has left the list, and stops once it has
    expect(busy.done!("ok:t38", undefined)).toBe(false);
    call({ actions: [], cards: [card(37), card(39)], outsideTool: OUTSIDE, threadId: "c1", onError: () => {} });
    expect(busy.done!("ok:t38", undefined)).toBe(true);
    expect(busy.scope).toBe("c1");
  });

  it("Approve all under a kept call and two held cards runs the call, then takes both holds off", async () => {
    reset();
    const p1 = action("p1");
    api.approved = [{ ...p1, status: "approved" }];
    const tree = call({ actions: [p1], cards: [card(40), card(42)], outsideTool: OUTSIDE, threadId: "c1", onError: () => {} });
    await click(tree, "all");
    expect(api.calls).toEqual([["approvePendingActions", ["p1"]], ["updateTask", "t40", { hold: "" }], ["updateTask", "t42", { hold: "" }]]);
    // busy until the call and both cards have left the list
    const v = await busy.runs[0];
    expect(v).toEqual({ ran: [{ ...p1, status: "approved" }], cards: ["t40", "t42"] });
    expect(busy.done!("all", v)).toBe(false);
    call({ actions: [], cards: [card(42)], outsideTool: OUTSIDE, threadId: "c1", onError: () => {} });
    expect(busy.done!("all", v)).toBe(false);
    call({ actions: [], cards: [], outsideTool: OUTSIDE, threadId: "c1", onError: () => {} });
    expect(busy.done!("all", v)).toBe(true);
  });

  it("Approve all with only cards takes each hold off and runs no calls", async () => {
    reset();
    await click(call({ actions: [], cards: [card(37), card(38)], outsideTool: OUTSIDE, threadId: "c1", onError: () => {} }), "all");
    expect(api.calls).toEqual([["updateTask", "t37", { hold: "" }], ["updateTask", "t38", { hold: "" }]]);
  });

  it("Approve all leaves a call that isn't waiting any more out, as before", async () => {
    reset();
    const tree = call({ actions: [action("p1", { status: "running" }), action("p2")], cards: [card(37)], outsideTool: OUTSIDE, threadId: "c1", onError: () => {} });
    await click(tree, "all");
    expect(api.calls[0]).toEqual(["approvePendingActions", ["p2"]]);
  });

  it("when approving the calls fails, no hold comes off and the error is shown", async () => {
    reset();
    api.failApprove = "The chat is busy";
    const errors: string[] = [];
    await click(call({ actions: [action("p1")], cards: [card(40), card(42)], outsideTool: OUTSIDE, threadId: "c1", onError: (e) => errors.push(e) }), "all");
    expect(api.calls).toEqual([["approvePendingActions", ["p1"]]]);
    expect(errors).toEqual(["The chat is busy"]);
  });

  it("keeps a call's Approve and Discard, after Approve all and before the cards' OK", () => {
    const tree = call({ actions: [action("p1")], cards: [card(40)], outsideTool: OUTSIDE, threadId: "c1", onError: () => {} });
    const names = elements(tree).map((e) => e.props.name).filter(Boolean);
    expect(names).toEqual(["all", "approve:p1", "discard:p1", "ok:t40"]);
  });
});

// ---- which cards: from the chat's own create_task, move_task and update_task results with on_hold ----

const lead = { actorId: "lead", name: "Chief", kind: "agent", roleKey: "lead", handle: "lead", status: "active", isLead: false, allowedTools: [],
  chatEnabled: true } as Member;
const at = Date.now() - 3_600_000;
const msg = (id: string, role: string, body: string | null, runId: string | null, minute: number, o: Partial<ChatMessage> = {}): ChatMessage => ({
  id, threadId: "c1", role, authorId: role === "user" ? "you" : "lead", authorName: role === "user" ? "You" : "Chief", bodyMd: body, runId,
  toolName: null, tool: null, createdAt: at + minute * 60_000, ...o,
});
const done = (n: number, onHold: boolean) => JSON.stringify({ ok: true, task: { id: `t${n}`, identifier: `DU-${n}`, column: "To do" },
  link: { page: "task", id: `t${n}`, label: `DU-${n} Card ${n}` }, ...(onHold ? { on_hold: `DU-${n} waits on hold for the user's OK, …` } : {}) });
const toolMsg = (id: string, name: string, n: number, onHold: boolean, runId: string, minute: number, o: Record<string, unknown> = {}) =>
  msg(id, "tool", null, runId, minute, { toolName: `mcp__gizai__${name}`, tool: { input: { task: `DU-${n}` }, result: done(n, onHold), isError: false, ...o } });

describe("heldCards and placeHeld", () => {
  it("takes the cards from create_task, move_task and update_task results with on_hold, in order, each once", () => {
    const list = heldCards([
      toolMsg("m1", "move_task", 37, true, "r1", 1), toolMsg("m2", "create_task", 38, true, "r1", 1), toolMsg("m3", "update_task", 39, true, "r1", 1),
      toolMsg("m4", "update_task", 50, false, "r1", 1),                                    // worked without a hold
      toolMsg("m5", "get_task", 51, true, "r1", 1),                                        // not a tool that holds
      msg("m6", "tool", null, "r1", 1, { toolName: "mcp__otus__move_task", tool: { input: {}, result: done(52, true), isError: false } }), // another server
      toolMsg("m7", "move_task", 53, true, "r1", 1, { isError: true }),                    // refused
      toolMsg("m8", "update_task", 37, true, "r2", 12),                                    // held again by a later answer
      msg("m9", "agent", done(54, true), "r2", 12),                                        // the Chief's text, not a result
    ]);
    expect(list.map((c) => [c.id, c.identifier, c.messageId, c.runId])).toEqual([
      ["t38", "DU-38", "m2", "r1"], ["t39", "DU-39", "m3", "r1"], ["t37", "DU-37", "m8", "r2"]]);
  });

  it("puts each card under the last group of messages of the answer that held it", () => {
    const messages = [msg("u1", "user", "Q1", null, 0), msg("a1", "agent", "A1", "r1", 1), toolMsg("m1", "move_task", 37, true, "r1", 1),
      msg("a1b", "agent", "A1 more", "r1", 7), msg("u2", "user", "Q2", null, 10), toolMsg("m2", "move_task", 38, true, "r2", 11)];
    const groups = groupMessages(messages);
    const placed = placeHeld(groups, heldCards(messages));
    const keyOf = (id: string) => groups.find((g) => g.items.some((m) => m.id === id))!.key;
    expect(placed.get(keyOf("a1b"))?.map((c) => c.identifier)).toEqual(["DU-37"]);
    expect(placed.get(keyOf("m2"))?.map((c) => c.identifier)).toEqual(["DU-38"]);
    // a card whose message isn't shown goes at the end
    expect(placeHeld([], heldCards(messages)).get("")?.length).toBe(2);
  });
});

// ---- in the chat ----

const thread = (o: Partial<Thread> = {}): Thread => ({ id: "c1", agentId: "lead", title: "DU", createdAt: 0, updatedAt: 0, costUsdMicros: 0,
  inputTokens: 0, outputTokens: 0, createdBy: "you", taskIds: [], ...o } as Thread);
const LINE = "This chat used new page on the chrome-devtools server, a tool from outside Hersir: what the Chief does here waits for your OK.";

describe("in the chat", () => {
  const kept = "memory_append wasn't run: this chat used mcp__chrome-devtools__new_page, a tool from outside Hersir. Hersir keeps the call for the user to approve with one click. Tell them in a sentence what it does.";
  const setup = () => {
    chat.messages = [
      msg("u1", "user", "QUESTION-ONE", null, 0), msg("a1", "agent", "ANSWER-ONE", "r1", 1),
      toolMsg("t1", "move_task", 37, true, "r1", 1), toolMsg("t2", "move_task", 38, true, "r1", 1), toolMsg("t3", "move_task", 39, true, "r1", 1),
      msg("u2", "user", "QUESTION-TWO", null, 10), msg("a2", "agent", "ANSWER-TWO", "r2", 11),
      msg("t4", "tool", null, "r2", 11, { toolName: "mcp__gizai__memory_append", tool: { input: { note: "Chief/Notes", text: "x" }, result: kept, isError: true } }),
      toolMsg("t5", "create_task", 40, true, "r2", 11), toolMsg("t6", "update_task", 42, true, "r2", 11),
      toolMsg("t7", "update_task", 43, true, "r2", 11),  // its hold was taken off on the card since
      msg("u3", "user", "QUESTION-THREE", null, 20),
    ];
    chat.pending = [action("p1", { runId: "r2", preview: { line: "CALL-OF-TWO", fields: [] } }),
      action("p9", { threadId: "other", runId: "r2", preview: { line: "CALL-OF-ANOTHER-CHAT", fields: [] } })];
    board.waiting = new Map([37, 38, 39, 40, 41, 42].map((n) => [`t${n}`, card(n, { title: `TITLE-${n}` })]));
    board.asked = [];
  };
  setup();
  const html = text(renderToStaticMarkup(<ChatThread threadId="c1" thread={thread({ outsideTool: OUTSIDE })} agent={lead} />));
  const i = (s: string) => { const n = html.indexOf(s); expect(n, s).toBeGreaterThan(-1); return n; };

  it("asks only for this chat's held cards (another chat's DU-41 never)", () => {
    expect(board.asked.at(-1)).toEqual(["t37", "t38", "t39", "t40", "t42", "t43"]);
    expect(html).not.toContain("DU-41");
  });

  it("shows the three cards an answer moved into To do under it, before your next message, each with its column and OK", () => {
    expect(i("ANSWER-ONE")).toBeLessThan(i("DU-37 TITLE-37"));
    expect(i("DU-37 TITLE-37")).toBeLessThan(i("DU-38 TITLE-38"));
    expect(i("DU-38 TITLE-38")).toBeLessThan(i("DU-39 TITLE-39"));
    expect(i("DU-39 TITLE-39")).toBeLessThan(i("QUESTION-TWO"));
    const first = html.slice(i("ANSWER-ONE"), i("QUESTION-TWO"));
    expect(count(first, ">OK</button>")).toBe(3);
    expect(count(first, '<span class="pa-column" title="Its column">To do</span>')).toBe(3);
    expect(first).toContain("so no agent starts these cards until you OK them.");
  });

  it("shows a kept call and two held cards under the same answer in one section, with one Approve all", () => {
    expect(i("ANSWER-TWO")).toBeLessThan(i("CALL-OF-TWO"));
    expect(i("CALL-OF-TWO")).toBeLessThan(i("DU-40 TITLE-40"));
    expect(i("DU-40 TITLE-40")).toBeLessThan(i("DU-42 TITLE-42"));
    expect(i("DU-42 TITLE-42")).toBeLessThan(i("QUESTION-THREE"));
    const second = html.slice(i("ANSWER-TWO"), i("QUESTION-THREE"));
    expect(count(second, 'aria-label="Waiting for your OK"')).toBe(1);
    expect(count(second, ">Approve all</button>")).toBe(1);
    expect(second).toContain('title="Runs each call, in this order, then takes the cards&#x27; holds off"');
    // the kept call's own line says it waits
    expect(second).toContain('<span class="tc-verb">Waiting for your OK: add to note</span><span class="tc-label">Chief/Notes</span>');
  });

  it("leaves out a card whose hold is gone, and another chat's calls", () => {
    expect(html).not.toContain('aria-label="DU-43 waits on hold"');
    expect(html).not.toContain("CALL-OF-ANOTHER-CHAT");
    expect(count(html, 'aria-label="Waiting for your OK"')).toBe(2);
  });

  it("drops a card from the list once it is off hold (OK, Clear hold, a move by hand)", () => {
    setup();
    board.waiting.delete("t38");
    const h = text(renderToStaticMarkup(<ChatThread threadId="c1" thread={thread({ outsideTool: OUTSIDE })} agent={lead} />));
    expect(h).not.toContain("DU-38 TITLE-38");
    expect(h).toContain("DU-37 TITLE-37");
    expect(h).toContain("DU-39 TITLE-39");
    // all of an answer's cards gone: its section goes too
    for (const n of [37, 39]) board.waiting.delete(`t${n}`);
    const g = text(renderToStaticMarkup(<ChatThread threadId="c1" thread={thread({ outsideTool: OUTSIDE })} agent={lead} />));
    expect(count(g, 'aria-label="Waiting for your OK"')).toBe(1);
    expect(g.indexOf('aria-label="Waiting for your OK"')).toBeGreaterThan(g.indexOf("ANSWER-TWO"));
  });

  it("shows the line about the outside tool above the text box, word for word, with New chat", () => {
    expect(html).toContain(LINE);
    expect(i(LINE)).toBeLessThan(i('class="composer'));
    expect(html).toMatch(new RegExp(`<div class="chat-banner outside" role="note"><span>${LINE.replace(/[.:]/g, "\\$&")}</span>`
      + `<button class="btn sm" title="Starts a new chat, which hasn&#x27;t used that tool"><svg[^]*?</svg>New chat</button></div>`));
  });

  it("shows no such line in a chat that isn't marked, or when the thread handed in is another chat's", () => {
    setup();
    chat.messages = [msg("u1", "user", "Q", null, 0), msg("a1", "agent", "A", "r1", 1)];
    chat.pending = [];
    for (const t of [thread({ outsideTool: null }), thread({}), thread({ id: "c2", outsideTool: OUTSIDE }), undefined]) {
      const h = renderToStaticMarkup(<ChatThread threadId="c1" thread={t} agent={lead} />);
      expect(h).not.toContain("a tool from outside Hersir");
      expect(h).not.toContain("chat-banner outside");
      expect(h).not.toContain(">New chat</button>");
    }
  });

  it("names a web tool in words", () => {
    setup();
    const h = text(renderToStaticMarkup(<ChatThread threadId="c1" thread={thread({ outsideTool: "WebFetch" })} agent={lead} />));
    expect(h).toContain("This chat used fetching a web page, a tool from outside Hersir: what the Chief does here waits for your OK.");
  });
});
