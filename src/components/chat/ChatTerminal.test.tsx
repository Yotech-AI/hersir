// GA-278 QA: the Chat page with its terminal panel, rendered to HTML on the server as ChatPage.test.tsx does (the data hooks
// answer at once): Open terminal right before New chat; no panel and no rail without terminals; with terminals opened in the
// chat (openChatTerminal, terminalOpen faked) a third column on the right of the conversation, also beside the Archive, with
// the tabs (the running program, ended), + with its tooltip, Own page, Hide with its tooltip, the folder in mono, the left
// edge as a vertical separator at 45%, and the terminal in the chat's place; hidden, the rail with the number and its
// tooltip. The layout's CSS (the conversation at the left while the panel shows, the limits, the list hiding) is read from
// the style sheets. The interaction itself (drag, keys, focus, a real shell) is the headless UI test's.
import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import type { ChatThread, Member, TerminalSession } from "../../types";

const answers: Record<string, (...a: unknown[]) => unknown> = {};
const f = vi.hoisted(() => ({
  list: [] as TerminalSession[],
  subs: new Set<() => void>(),
  next: [] as TerminalSession[],
}));
vi.mock("../../api", async (orig) => {
  const real = await orig<Record<string, unknown>>();
  const out: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(real)) out[k] = typeof v !== "function" ? v : (...a: unknown[]) => (answers[k] ? answers[k](...a) : new Promise(() => {}));
  return {
    ...out,
    terminalOpen: () => { const s = f.next.shift(); return s ? Promise.resolve(s) : Promise.reject(new Error("none ready")); },
  };
});
vi.mock("../../lib/useData", () => ({
  useData: (fetch: () => unknown) => {
    let data: unknown = null;
    try { const v = fetch(); if (!(v instanceof Promise)) data = v; } catch { /* no data */ }
    return { data, error: null, reload: () => {}, setData: () => {} };
  },
}));
vi.mock("./useChat", () => ({ useChatLive: () => [] }));
vi.mock("./ChatThread", () => ({
  ChatThread: ({ threadId }: { threadId: string | null }) => <section className="chat-main" data-conversation={threadId ?? "new"}><div className="chat-col" /></section>,
}));
// TF0's store, faked: the list of terminals and where each shows (none yet: the panel asks for it once it is on screen)
vi.mock("../../lib/terminals", async (orig) => ({
  ...(await orig<typeof import("../../lib/terminals")>()),
  terminalsNow: () => f.list,
  watchTerminals: (g: () => void) => { f.subs.add(g); return () => { f.subs.delete(g); }; },
  noteOpened: (s: TerminalSession) => { if (!f.list.some((x) => x.id === s.id)) { f.list = [...f.list, s]; f.subs.forEach((g) => g()); } },
  useTerminals: () => f.list,
  useShownIn: () => ({ kind: "chat" }),
  useViewMade: () => false,
}));

// the styles are read from disk (vitest hands a .css import back empty); node:fs through a computed specifier, as band.test.ts does
const fs = await import(/* @vite-ignore */ ["node", "fs"].join(":"));
const read = (path: string): string => fs.readFileSync(new URL(`../../styles/${path}`, import.meta.url), "utf8");
const appCss = read("app.css");
const componentsCss = read("components.css");
const terminalCss = read("terminal.css");

const { ChatPage } = await import("../../pages/ChatPage");
const C = await import("../../lib/chatTerminal");

const lead = { actorId: "lead", name: "Chief", kind: "agent", roleKey: "lead", handle: "lead", status: "active", isLead: false, allowedTools: [],
  chatEnabled: true } as Member;
const atOnce = <T,>(v: T) => ({ then: (g: (x: T) => unknown) => g(v) });
const chat = (n: number): ChatThread => ({ id: `c${n}`, agentId: "lead", title: `Chat ${n}`, createdAt: n, updatedAt: n, costUsdMicros: 0, inputTokens: 0, outputTokens: 0 });
const HERD = "/home/you/Herd";
const session = (id: string, name: string, more: Partial<TerminalSession> = {}): TerminalSession => ({
  id, name, folder: `${HERD}/doq-studio`, folderPath: "~/Herd/doq-studio", taskId: null, shell: "sh", busy: null, ended: null,
  cols: 80, rows: 24, openedAt: 1, ...more,
});
const page = (id?: string, archive = false) => renderToStaticMarkup(<ChatPage id={id} archive={archive} youId="you" />);
const chatDiv = (html: string) => /<div class="(chat[^"]*)"/.exec(html)?.[1];
const between = (html: string, from: string, to: string) => html.slice(html.indexOf(from), html.indexOf(to, html.indexOf(from)));
const open = (s: TerminalSession) => { f.next.push(s); return C.openChatTerminal(s.folder, false); };

beforeEach(() => {
  answers.chatAgent = () => atOnce(lead);
  answers.listChatThreads = () => [chat(1), chat(2)];
  answers.listChatPins = () => atOnce([]);
});

// The chat's terminals are kept in the module while Hersir runs, so these tests follow each other: none, then two, hidden, …
describe("the Chat page's top bar", () => {
  it("has Open terminal right before New chat, with the terminal icon, opening a picker", () => {
    const html = page();
    const bar = between(html, 'class="topbar"', "</div></div>");
    const openAt = bar.indexOf("Open terminal");
    const newAt = bar.indexOf("New chat");
    expect(openAt).toBeGreaterThan(0);
    expect(newAt).toBeGreaterThan(openAt);
    // nothing between them but Open terminal's own button
    expect(bar.slice(openAt, newAt)).toMatch(/^Open terminal<\/button><button class="btn sm">(<svg[^]*?<\/svg>)?$/);
    const button = /<button class="btn sm" aria-haspopup="dialog" aria-expanded="false">(<svg[^>]*>)/.exec(bar);
    expect(button?.[1]).toContain("lucide-terminal");
  });

  it("has no Open terminal while the Chief isn't set up (as New chat)", () => {
    answers.chatAgent = () => atOnce(null);
    const html = page();
    expect(html).not.toContain("Open terminal");
    expect(html).not.toContain("New chat</button>");
  });
});

describe("no terminals in the chat", () => {
  it("draws the chat as before: two columns, no panel, no rail", () => {
    const html = page("c1");
    expect(chatDiv(html)).toBe("chat");
    expect(html).not.toContain("chat-term");
  });
});

describe("the panel, with terminals opened in the chat", () => {
  it("is a third column on the right of the conversation", async () => {
    expect(await open(session("t1", "doq-studio"))).toBeNull();
    expect(await open(session("t2", "doq-studio 2"))).toBeNull();
    const html = page("c1");
    expect(chatDiv(html)).toBe("chat has-term");
    const main = html.indexOf('data-conversation="c1"');
    const panel = html.indexOf('class="chat-term"');
    expect(main).toBeGreaterThan(0);
    expect(panel, "after the conversation").toBeGreaterThan(main);
    expect(html.slice(panel)).toMatch(/^class="chat-term" role="region" aria-label="Terminal"/);
  });

  it("has the chat's terminals as tabs, with what runs in each or ended, the shown one selected", () => {
    f.list = f.list.map((s) => (s.id === "t1" ? { ...s, busy: "sleep" } : s));
    let html = page("c1");
    const tabs = between(html, 'role="tablist" aria-label="Terminals in the chat"', '<span class="acts">');
    expect([...tabs.matchAll(/data-term="(t\d)"/g)].map((m) => m[1])).toEqual(["t1", "t2"]);
    expect(tabs).toMatch(/data-term="t1"[^>]*aria-selected="false"/);
    expect(tabs).toMatch(/data-term="t2"[^>]*aria-selected="true"/);
    expect(tabs).toContain('<span class="name">doq-studio</span><span class="prog">sleep</span>');
    expect(tabs).toContain('aria-label="Close doq-studio"');
    f.list = f.list.map((s) => (s.id === "t1" ? { ...s, busy: null, ended: { code: 0, at: 2, why: null } } : s));
    html = page("c1");
    expect(html).toContain('<span class="name">doq-studio</span><span class="state">ended</span>');
    f.list = f.list.map((s) => (s.id === "t1" ? { ...s, ended: null } : s));
  });

  it("has +, Own page and Hide with the card's tooltips", () => {
    const acts = between(page("c1"), '<span class="acts">', "</span></div>");
    expect(acts).toContain('aria-label="New terminal in doq-studio" title="New terminal in doq-studio (Ctrl+Shift+`)"');
    expect(acts).toContain('aria-label="Own page"');
    expect(acts).toContain('aria-label="Hide" title="Hide the terminal (Ctrl+`): its shells keep running"');
    expect(acts).not.toContain("aria-disabled");
  });

  it("shows the shown terminal's folder in mono, and the terminal in the chat's place", () => {
    const html = page("c1");
    expect(html).toContain('<div class="chat-term-where" title="~/Herd/doq-studio"><bdi>~/Herd/doq-studio</bdi></div>');
    expect(between(html, 'class="chat-term"', "</section>")).toContain('class="term-place');
    expect(terminalCss).toMatch(/\.chat-term-where \{[^}]*font-family: var\(--font-mono\)/);
  });

  it("has its left edge as a vertical separator, at 45% the first time", () => {
    const html = page("c1");
    expect(html).toMatch(/class="term-handle" role="separator" aria-orientation="vertical" aria-label="Terminal panel&#x27;s width" aria-valuenow="45"/);
    // the handle comes first: the panel's left edge
    expect(between(html, 'class="chat-term"', 'class="chat-term-in"')).toContain("term-handle");
  });

  it("stays beside the Archive, New chat and another chat", () => {
    for (const [id, archive] of [[undefined, true], [undefined, false], ["c2", false]] as const) {
      const html = page(id, archive);
      expect([id, archive, chatDiv(html)]).toEqual([id, archive, "chat has-term"]);
      expect(html).toMatch(/data-term="t2"[^>]*aria-selected="true"/);
    }
    const archive = page(undefined, true);
    expect(archive.indexOf('class="chat-term"')).toBeGreaterThan(archive.indexOf("chat-archive"));
  });
});

describe("hidden with terminals: the rail", () => {
  it("is a narrow column with the terminal icon, their number and the card's tooltip", () => {
    C.hideChatPanel();
    const html = page("c1");
    expect(chatDiv(html)).toBe("chat has-rail");
    expect(html).not.toContain('class="chat-term"');
    const rail = /<button class="chat-term-rail"[^>]*>[^]*?<\/button>/.exec(html)?.[0] ?? "";
    expect(rail).toContain('title="Show the terminal (Ctrl+`)"');
    expect(rail).toContain('aria-label="Show the terminal (2 terminals)"');
    expect(rail).toContain("lucide-terminal");
    expect(rail).toContain('<span class="n">2</span>');
  });

  it("goes when the last terminal closes, and the chat is as before", () => {
    f.list = [];
    f.subs.forEach((g) => g());
    const html = page("c1");
    expect(chatDiv(html)).toBe("chat");
    expect(html).not.toContain("chat-term");
  });
});

/** The declarations of the rule with exactly this selector list. */
function rule(css: string, selector: string): string {
  const flat = css.replace(/\/\*[\s\S]*?\*\//g, "");
  const at = flat.indexOf(`${selector} {`);
  return at < 0 ? "" : flat.slice(at + selector.length + 2, flat.indexOf("}", at));
}

describe("the layout's CSS", () => {
  it("centres the conversation as before without the panel, and starts it at the left of its column while the panel shows", () => {
    expect(rule(componentsCss, ".chat-col")).toMatch(/max-width: calc\(760px \* var\(--chat-scale, 1\)\);\s*margin: 0 auto;\s*padding: 24px;/);
    expect(rule(appCss, ".chat.has-term .chat-col").trim()).toBe("margin-left: 0;");
  });

  it("gives the panel the share of the space right of the chat list, at least 360px, leaving the conversation 400px", () => {
    const has = rule(appCss, ".chat.has-term");
    expect(has).toContain("clamp(360px, calc((100% - var(--chat-list)) * var(--chat-term-share, .45)), calc(100% - var(--chat-list) - 400px))");
    expect(has).toContain("grid-template-columns: var(--chat-list) minmax(0, 1fr) var(--chat-term-w);");
  });

  it("hides the chat list when it has no room, and draws the rail as a narrow last column", () => {
    expect(rule(appCss, ".chat.has-term.no-list")).toContain("grid-template-columns: minmax(0, 1fr) var(--chat-term-w);");
    expect(rule(appCss, ".chat.no-list > .chat-threads").trim()).toBe("display: none;");
    expect(rule(appCss, ".chat.has-rail")).toContain("grid-template-columns: var(--chat-list) minmax(0, 1fr) auto;");
  });

  it("draws the chat list as wide as chatListWidth says: 260px, 220px up to 1100px, 6px more per pixel of text size", () => {
    expect(rule(appCss, ".chat")).toContain("--chat-list: calc(260px + 6 * var(--ui-grow, 0px));");
    expect(appCss).toMatch(/@media \(max-width: 1100px\) \{ \.chat \{ --chat-list: calc\(220px \+ 6 \* var\(--ui-grow, 0px\)\); \} \}/);
  });
});
