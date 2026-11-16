// GA-257 QA: Chat → Pinned on the Chat page. Pinned chats show under a Pinned heading above Recent, the most recently
// pinned first, with their pulse, label, title and time, and aren't listed again under Recent; a pinned chat older than the
// 30 newest shows too. Each chat on the left, the conversation's top bar and each Archive row has a pin button (Pin to the
// top, or Unpin on a pinned chat) that sits beside the chat's link, never inside it, so pressing it doesn't open the chat.
// Rendered to HTML on the server as in ChatPage.test.tsx: the data hooks are replaced so the page gets its data at once.
import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import type { ChatHit, ChatStatus, ChatThread, Member } from "../types";
import type { Pins } from "../components/chat/usePins";

// What the mocked api answers, by function name; anything else never answers (its useData stays empty).
const answers: Record<string, (...a: unknown[]) => unknown> = {};
const calls: [string, unknown[]][] = [];
vi.mock("../api", async (orig) => {
  const real = await orig<Record<string, unknown>>();
  const out: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(real)) {
    out[k] = typeof v !== "function" ? v : (...a: unknown[]) => { calls.push([k, a]); return answers[k] ? answers[k](...a) : new Promise(() => {}); };
  }
  return out;
});
// useData hands over what the api answered at once (a pending promise: nothing yet).
vi.mock("../lib/useData", () => ({
  useData: (fetch: () => unknown) => {
    let data: unknown = null;
    try { const v = fetch(); if (!(v instanceof Promise)) data = v; } catch { /* no data */ }
    return { data, error: null, reload: () => {}, setData: () => {} };
  },
}));
let live: ChatStatus[] = [];
vi.mock("../components/chat/useChat", () => ({ useChatLive: () => live }));
vi.mock("../components/chat/ChatThread", () => ({
  ChatThread: ({ threadId, thread }: { threadId: string | null; thread?: ChatThread }) =>
    <section data-conversation={threadId ?? "new"}>{thread ? `title=${thread.title}` : "no thread"}</section>,
}));
// The Archive's search results: its first null-initialised states (what was found, the error) take these values.
const states: unknown[] = [];
vi.mock("react", async (orig) => {
  const R = await orig<typeof import("react")>();
  const useState = ((init: unknown) => R.useState(init === null && states.length > 0 ? states.shift() : init)) as typeof R.useState;
  return { ...R, default: { ...R, useState }, useState };
});

const { ChatPage } = await import("./ChatPage");
const { ChatArchive } = await import("../components/chat/ChatArchive");

const HOUR = 3_600_000;
const now = Date.now();
const lead = { actorId: "lead", name: "Team Lead", kind: "agent", roleKey: "lead", handle: "lead", status: "active", isLead: false, allowedTools: [],
  chatEnabled: true } as Member;
const chat = (n: number, more: Partial<ChatThread> = {}): ChatThread => ({
  id: `c${n}`, agentId: "lead", title: `Chat ${n}`, createdAt: now - n * HOUR, updatedAt: now - n * HOUR, costUsdMicros: 0, inputTokens: 0, outputTokens: 0, ...more,
});
const atOnce = <T,>(v: T) => ({ then: (f: (x: T) => unknown) => f(v) });

const recent30 = Array.from({ length: 30 }, (_, i) => chat(i, i === 2 ? { kind: "question", waiting: true } : {}));
const page = (id?: string, archive = false) => renderToStaticMarkup(<ChatPage id={id} archive={archive} />);
const between = (html: string, from: string, to: string) => html.slice(html.indexOf(from), html.indexOf(to, html.indexOf(from)));
const left = (html: string) => between(html, 'class="chat-threads-list"', 'class="chat-threads-foot"');
/** Pinned's part of the list on the left (from its heading to Recent's, or to the end). */
const pinnedPart = (html: string) => { const l = left(html), at = l.indexOf(">Pinned<"), r = l.indexOf(">Recent<"); return at < 0 ? "" : l.slice(at, r < 0 ? undefined : r); };
const recentPart = (html: string) => { const l = left(html), r = l.indexOf(">Recent<"); return r < 0 ? "" : l.slice(r); };
const ids = (part: string) => [...part.matchAll(/<a class="th[^"]*" href="#\/chat\/(c\d+)"/g)].map((m) => m[1]);
/** Each chat row on the left: its div.th-row with the link and the pin button. */
const rows = (part: string) => part.split('<div class="th-row').slice(1).map((r) => r.slice(0, r.indexOf("</button>") + 9));
const pins = (list: ChatThread[]): Pins => ({ list, has: (id) => list.some((t) => t.id === id), toggle: () => {}, error: "" });

beforeEach(() => {
  for (const k of Object.keys(answers)) delete answers[k];
  calls.length = 0;
  states.length = 0;
  live = [];
  answers.chatAgent = () => atOnce(lead);
  answers.listChatThreads = () => recent30;
});

describe("Chat → Pinned on the left", () => {
  it("asks the backend for the pinned chats", () => {
    page();
    expect(calls.filter(([k]) => k === "pinnedChats")).toEqual([["pinnedChats", []]]);
  });

  it("has no Pinned heading while nothing is pinned, and Recent keeps its 30", () => {
    answers.pinnedChats = () => [];
    const html = page();
    expect(html).not.toContain(">Pinned<");
    expect(ids(recentPart(html))).toEqual(recent30.map((t) => t.id));
  });

  it("lists the pinned chats under Pinned, above Recent, in the order the backend gives (the newest pin first)", () => {
    answers.pinnedChats = () => [recent30[7], recent30[2], recent30[20]];
    const html = page();
    const list = left(html);
    expect(list.indexOf(">New chat")).toBeLessThan(list.indexOf(">Pinned<"));
    expect(list.indexOf(">Pinned<")).toBeLessThan(list.indexOf(">Recent<"));
    expect(list).toContain('<div class="nav-label">Pinned</div>');
    expect(ids(pinnedPart(html))).toEqual(["c7", "c2", "c20"]);
  });

  it("doesn't list a pinned chat again under Recent; Recent keeps the others in their order", () => {
    answers.pinnedChats = () => [recent30[7], recent30[2]];
    const html = page();
    const recent = ids(recentPart(html));
    expect(recent).toEqual(recent30.filter((t) => t.id !== "c7" && t.id !== "c2").map((t) => t.id));
    expect(recent).toHaveLength(28);
    // each pinned chat is on the left once
    expect(left(html).match(/href="#\/chat\/c7"/g)).toHaveLength(1);
    expect(left(html).match(/href="#\/chat\/c2"/g)).toHaveLength(1);
  });

  it("shows a pinned chat with its pulse, label, title and time, as Recent does", () => {
    live = [{ threadId: "c7", runId: "r", draft: "", seq: 0 }];
    answers.pinnedChats = () => [recent30[7], recent30[2]];
    const html = page();
    const [seven, two] = rows(pinnedPart(html));
    expect(seven).toContain('class="pulse"');
    expect(seven).toContain('<span class="ellipsis">Chat 7</span>');
    expect(seven).toContain('<span class="when">7h ago</span>');
    expect(two).not.toContain("pulse");
    expect(two).toContain('class="badge needs"');
    expect(two).toContain(">Question<");
    expect(two).toContain('<span class="when">2h ago</span>');
    // the same link Recent shows for it
    answers.pinnedChats = () => [];
    const unpinned = rows(recentPart(page())).find((r) => r.includes('href="#/chat/c2"'))!;
    const link = (r: string) => r.slice(r.indexOf("<a "), r.indexOf("</a>") + 4);
    expect(link(two)).toBe(link(unpinned));
  });

  it("shows a pinned chat older than the 30 newest, with Recent still the 30 newest", () => {
    answers.pinnedChats = () => [chat(45, { title: "An old plan" })];
    const html = page();
    expect(ids(pinnedPart(html))).toEqual(["c45"]);
    expect(pinnedPart(html)).toContain(">An old plan<");
    expect(pinnedPart(html)).toContain('<span class="when">2d ago</span>');
    expect(ids(recentPart(html))).toEqual(recent30.map((t) => t.id));
  });

  it("highlights the open chat under Pinned", () => {
    answers.pinnedChats = () => [recent30[4]];
    const html = page("c4");
    expect(pinnedPart(html)).toMatch(/<a class="th on" href="#\/chat\/c4" aria-current="page"/);
    expect(recentPart(html)).not.toContain('class="th on"');
  });
});

describe("the pin button beside each chat on the left", () => {
  it("says Pin to the top on a Recent row and Unpin on a Pinned row, as its label and tooltip", () => {
    answers.pinnedChats = () => [recent30[7]];
    const html = page();
    const recent = rows(recentPart(html));
    expect(recent).toHaveLength(29);
    for (const r of recent) {
      const id = r.match(/href="#\/chat\/(c\d+)"/)![1];
      expect(r).toContain(`<button type="button" class="btn ghost sm icon-only pin-btn" aria-label="Pin to the top" title="Pin to the top" data-pin="${id}">`);
      expect(r.startsWith('">')).toBe(true);
    }
    const [pinned] = rows(pinnedPart(html));
    expect(pinned.startsWith(' pinned">')).toBe(true);
    expect(pinned).toContain('<button type="button" class="btn ghost sm icon-only pin-btn on" aria-label="Unpin" title="Unpin" data-pin="c7">');
    // the pin and the crossed-out pin (shown on hover of a pinned chat)
    expect(pinned).toContain("lucide-pin ");
    expect(pinned).toContain("lucide-pin-off");
  });

  it("sits beside the chat's link, never inside it", () => {
    answers.pinnedChats = () => [recent30[7], chat(45)];
    const html = page();
    const links = [...left(html).matchAll(/<a [^>]*>[\s\S]*?<\/a>/g)].map((m) => m[0]);
    expect(links.length).toBe(32); // New chat, 2 pinned, 29 in Recent, and none holds a button
    for (const a of links) expect(a).not.toContain("<button");
    for (const r of rows(left(html))) expect(r.indexOf("</a><button")).toBeGreaterThan(0);
    // New chat and Archive have no pin
    expect(left(html).match(/pin-btn/g)).toHaveLength(31);
    expect(between(html, 'class="chat-threads-foot"', "</aside>")).not.toContain("pin-btn");
  });
});

describe("the pin button in the conversation's top bar", () => {
  const crumbs = (html: string) => between(html, 'class="crumbs"', 'class="actions"');

  it("follows the chat's title: Pin to the top on a chat that isn't pinned", () => {
    answers.pinnedChats = () => [];
    const c = crumbs(page("c3"));
    expect(c).toMatch(/<span class="ellipsis">Chat 3<\/span><button type="button" class="btn ghost sm icon-only pin-btn" aria-label="Pin to the top" title="Pin to the top" data-pin="c3">/);
  });

  it("says Unpin on a pinned chat", () => {
    answers.pinnedChats = () => [recent30[3]];
    expect(crumbs(page("c3"))).toContain('class="btn ghost sm icon-only pin-btn on" aria-label="Unpin" title="Unpin" data-pin="c3"');
  });

  it("is there for a pinned chat older than the 30 newest before its own answer comes, with its title", () => {
    answers.pinnedChats = () => [chat(45, { title: "An old plan" })];
    const html = page("c45");
    expect(calls.filter(([k]) => k === "getChatThread")).toEqual([["getChatThread", ["c45"]]]);
    expect(crumbs(html)).toContain('<span class="ellipsis">An old plan</span>');
    expect(crumbs(html)).toContain('aria-label="Unpin"');
    expect(html).toContain("title=An old plan");
  });

  it("isn't on a new chat or on the Archive", () => {
    answers.pinnedChats = () => [recent30[3]];
    expect(crumbs(page())).not.toContain("pin-btn");
    expect(crumbs(page(undefined, true))).not.toContain("pin-btn");
  });
});

describe("the pin button on the Archive's rows", () => {
  const archive = (hits: ChatHit[], p?: Pins) => {
    states.push({ query: "", hits }, null);
    return renderToStaticMarkup(<ChatArchive live={live} pins={p} />);
  };
  const items = (html: string) => html.split('<div role="listitem" class="archive-item').slice(1);

  it("is on each row, beside its link: Pin to the top, or Unpin on a pinned chat", () => {
    const html = archive([chat(0), chat(5), chat(40)].map((thread) => ({ thread, message: null })), pins([chat(40)]));
    const all = items(html);
    expect(all).toHaveLength(3);
    expect(all[0].startsWith('"><a class="archive-row" href="#/chat/c0">')).toBe(true);
    expect(all[0]).toContain('</a><button type="button" class="btn ghost sm icon-only pin-btn" aria-label="Pin to the top" title="Pin to the top" data-pin="c0">');
    expect(all[1]).toContain('aria-label="Pin to the top" title="Pin to the top" data-pin="c5"');
    expect(all[2].startsWith(' pinned"><a class="archive-row" href="#/chat/c40">')).toBe(true);
    expect(all[2]).toContain('class="btn ghost sm icon-only pin-btn on" aria-label="Unpin" title="Unpin" data-pin="c40"');
    for (const a of html.match(/<a class="archive-row"[\s\S]*?<\/a>/g)!) expect(a).not.toContain("<button");
  });

  it("is handed the page's pins on #/chats", () => {
    answers.pinnedChats = () => [recent30[1]];
    states.push({ query: "", hits: [recent30[0], recent30[1]].map((thread) => ({ thread })) });
    const html = page(undefined, true);
    const all = items(html.slice(html.indexOf('class="chat-main chat-archive"')));
    expect(all).toHaveLength(2);
    expect(all[0]).toContain('aria-label="Pin to the top"');
    expect(all[1]).toContain('aria-label="Unpin"');
  });

  it("isn't there without pins", () => {
    expect(archive([{ thread: chat(0) }])).not.toContain("pin-btn");
  });
});
