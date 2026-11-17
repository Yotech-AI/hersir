// GA-296 QA (was GA-278's ChatTerminal.test.tsx, the terminal panel and its rail): the Chat page in islands with the side
// island and the tool strip, rendered to HTML on the server as ChatPage.test.tsx does (the data hooks answer at once): one
// Open button with a menu right before New chat; nothing open: no island and no strip; with terminals opened in the chat
// (openChatTerminal, terminalOpen faked) the side island right of the conversation with its tabs (the running program,
// ended), + with its menu, Own page and Hide with the notes' tooltips, the folder in mono, the left edge as a vertical
// separator at 45%, the terminal in the chat's place, and the strip after it: Terminal with its count, selected, its
// tooltip; hidden, the strip alone, nothing selected; another chat, New chat and the Archive show their own (none). The
// layout's CSS (the islands, the columns with the strip, the limits, the list hiding) is read from the style sheets. The
// interaction itself (clicks, menus, drag, keys, focus, a real shell) is the headless UI test's.
// useChatSideLayout tells chatSide which chat shows in a layout effect, which a server render doesn't run: page() does it.
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
// TF0's store, faked: the list of terminals and where each shows
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
const S = await import("../../lib/chatSide");

const lead = { actorId: "lead", name: "Chief", kind: "agent", roleKey: "lead", handle: "lead", status: "active", isLead: false, allowedTools: [],
  chatEnabled: true } as Member;
const atOnce = <T,>(v: T) => ({ then: (g: (x: T) => unknown) => g(v) });
const chat = (n: number): ChatThread => ({ id: `c${n}`, agentId: "lead", title: `Chat ${n}`, createdAt: n, updatedAt: n, costUsdMicros: 0, inputTokens: 0, outputTokens: 0 });
const HERD = "/home/you/Herd";
const session = (id: string, name: string, more: Partial<TerminalSession> = {}): TerminalSession => ({
  id, name, folder: `${HERD}/doq-studio`, folderPath: "~/Herd/doq-studio", taskId: null, shell: "sh", busy: null, ended: null,
  cols: 80, rows: 24, openedAt: 1, ...more,
});
/** The Chat page of chat `id` (New chat without, the Archive with `archive`), as the page tells chatSide it shows. */
const page = (id?: string, archive = false) => {
  S.showChat(S.chatSideKey(id, archive));
  return renderToStaticMarkup(<ChatPage id={id} archive={archive} youId="you" />);
};
const chatDiv = (html: string) => /<div class="(chat[^"]*)"/.exec(html)?.[1];
const between = (html: string, from: string, to: string) => html.slice(html.indexOf(from), html.indexOf(to, html.indexOf(from)));
/** The strip's markup. */
const strip = (html: string) => (html.includes('class="chat-strip"') ? html.slice(html.indexOf('<div class="chat-strip"')) : "");
const open = (s: TerminalSession) => { f.next.push(s); return C.openChatTerminal(s.folder, false); };

beforeEach(() => {
  answers.chatAgent = () => atOnce(lead);
  answers.listChatThreads = () => [chat(1), chat(2)];
  answers.listChatPins = () => atOnce([]);
});

// The chats' tabs are kept in the module while Hersir runs, so these tests follow each other: none, then two in chat 1, hidden, …
describe("the Chat page's top bar", () => {
  it("has one Open button with a menu right before New chat: panel-right, Open, chevron-down, and its tooltip", () => {
    const html = page();
    const bar = between(html, 'class="topbar"', "</div></div>");
    expect(bar).not.toContain("Open terminal");
    const button = /<button type="button" class="btn sm chat-open"([^>]*)>([^]*?)<\/button>/.exec(bar);
    expect(button, "the Open button").not.toBeNull();
    expect(button![1]).toBe(' aria-haspopup="menu" aria-expanded="false" title="Open a terminal, a file or the board beside the chat"');
    const inside = button![2]!;
    expect(inside.indexOf("lucide-panel-right"), "the panel-right icon first").toBeGreaterThan(0);
    expect(inside.indexOf(">Open<")).toBeGreaterThan(inside.indexOf("lucide-panel-right"));
    expect(inside.indexOf("lucide-chevron-down"), "the chevron after the word").toBeGreaterThan(inside.indexOf(">Open<"));
    // nothing between it and New chat
    const after = bar.slice(bar.indexOf(button![0]) + button![0].length);
    expect(after).toMatch(/^<button class="btn sm">(<svg[^]*?<\/svg>)?New chat<\/button>/);
    expect([...bar.matchAll(/>Open</g)], "one Open").toHaveLength(1);
  });

  it("has no Open while the Chief isn't set up (as New chat)", () => {
    answers.chatAgent = () => atOnce(null);
    const html = page();
    expect(html).not.toContain("chat-open");
    expect(html).not.toContain("New chat</button>");
  });
});

describe("nothing open beside the chat", () => {
  it("draws the chat as two islands: no side island, no strip", () => {
    const html = page("c1");
    expect(chatDiv(html)).toBe("chat");
    expect(html).not.toContain("chat-side");
    expect(html).not.toContain("chat-strip");
  });
});

describe("the side island, with terminals opened in the chat", () => {
  it("is a column right of the conversation, with the strip after it", async () => {
    S.showChat("c1");
    expect(await open(session("t1", "doq-studio"))).toBeNull();
    expect(await open(session("t2", "doq-studio 2"))).toBeNull();
    const html = page("c1");
    expect(chatDiv(html)).toBe("chat has-side has-strip");
    const main = html.indexOf('data-conversation="c1"');
    const island = html.indexOf('<div class="chat-side">');
    const bar = html.indexOf('<div class="chat-strip"');
    expect(main).toBeGreaterThan(0);
    expect(island, "after the conversation").toBeGreaterThan(main);
    expect(bar, "the strip after the island").toBeGreaterThan(island);
  });

  it("has the chat's terminals as tabs in the order they opened, with what runs in each or ended, the shown one selected", () => {
    f.list = f.list.map((s) => (s.id === "t1" ? { ...s, busy: "sleep" } : s));
    let html = page("c1");
    const tabs = between(html, 'role="tablist" aria-label="Beside the chat"', '<span class="acts">');
    expect(tabs, "the tab list").not.toBe("");
    expect([...tabs.matchAll(/data-term="(t\d)"/g)].map((m) => m[1])).toEqual(["t1", "t2"]);
    expect(tabs).toMatch(/data-term="t1"[^>]*aria-selected="false"/);
    expect(tabs).toMatch(/data-term="t2"[^>]*aria-selected="true"/);
    expect(tabs, "the Terminal kind's icon").toContain("lucide-terminal");
    expect(tabs).toContain('<span class="name">doq-studio</span><span class="prog">sleep</span>');
    expect(tabs).toContain('aria-label="Close doq-studio"');
    f.list = f.list.map((s) => (s.id === "t1" ? { ...s, busy: null, ended: { code: 0, at: 2, why: null } } : s));
    html = page("c1");
    expect(html).toContain('<span class="name">doq-studio</span><span class="state">ended</span>');
    f.list = f.list.map((s) => (s.id === "t1" ? { ...s, ended: null } : s));
  });

  it("has +, Own page and Hide with the notes' tooltips", () => {
    const acts = between(page("c1"), '<span class="acts">', "</span></div>");
    expect(acts).toMatch(/aria-label="Open beside the chat" title="Open beside the chat" aria-haspopup="menu" aria-expanded="false"><svg[^>]*lucide-plus/);
    expect(acts).toContain('aria-label="Own page" title="Open doq-studio 2 on its own page"');
    expect(acts).toMatch(/aria-label="Own page"[^>]*><svg[^>]*lucide-maximize-2/);
    expect(acts).toContain('aria-label="Hide" title="Hide (Ctrl+`): terminals keep running"');
    expect(acts).toMatch(/aria-label="Hide"[^>]*><svg[^>]*lucide-chevron-right/);
    expect(acts).not.toContain("disabled");
    // the order: +, Own page, Hide
    expect(acts.indexOf("Open beside the chat")).toBeLessThan(acts.indexOf("Own page"));
    expect(acts.indexOf("Own page")).toBeLessThan(acts.indexOf('aria-label="Hide"'));
  });

  it("shows the shown terminal's folder in mono, and the terminal in the chat's place", () => {
    const html = page("c1");
    expect(html).toContain('<div class="chat-term-where" title="~/Herd/doq-studio"><bdi>~/Herd/doq-studio</bdi></div>');
    expect(between(html, 'class="chat-side"', '<div class="chat-strip"')).toContain('class="term-place');
    expect(terminalCss).toMatch(/\.chat-term-where \{[^}]*font-family: var\(--font-mono\)/);
  });

  it("has its left edge as a vertical separator, at 45% the first time", () => {
    const html = page("c1");
    expect(html).toMatch(/class="term-handle" role="separator" aria-orientation="vertical" aria-label="Side island&#x27;s width" aria-valuenow="45"/);
    // the handle comes first: the island's left edge
    expect(between(html, 'class="chat-side"', 'class="chat-side-in"')).toContain("term-handle");
  });
});

describe("the tool strip", () => {
  it("is a vertical toolbar Beside the chat with Terminal, its count and tooltip, selected while the island shows a terminal", () => {
    const bar = strip(page("c1"));
    expect(bar).toMatch(/^<div class="chat-strip" role="toolbar" aria-label="Beside the chat" aria-orientation="vertical">/);
    const btns = [...bar.matchAll(/<button [^>]*class="strip-btn"[^>]*>[^]*?<\/button>/g)].map((m) => m[0]);
    expect(btns).toHaveLength(1);
    // GA-300: the tooltip is drawn as the notes' Islands/Tooltip (.strip-tip, on hover or keyboard focus), so the button has
    // no title for the browser to draw its own
    expect(btns[0]).toMatch(/data-kind="terminal" aria-pressed="true" aria-label="Terminal, 2 open">/);
    expect(btns[0]).not.toContain("title=");
    expect(btns[0]).toContain("lucide-terminal");
    expect(btns[0], "the count under the icon").toMatch(/<\/svg><span class="n">2<\/span><\/button>$/);
  });

  it("has an icon per kind open, in the order Terminal, Editor, Board, Card, each with its count; the shown kind selected", () => {
    // Card, Board and Editor tabs come with cards 8 and 9; the store takes them already, and the strip draws them
    S.showChat("c9");
    S.openSideTab({ kind: "card", id: "GA-1" });
    S.openSideTab({ kind: "board", id: "hersir" });
    S.openSideTab({ kind: "card", id: "GA-2" });
    S.openSideTab({ kind: "editor", id: "src/a.ts" });
    S.openSideTab({ kind: "card", id: "GA-3" });
    const bar = strip(page("c9"));
    const btns = [...bar.matchAll(/data-kind="(\w+)" aria-pressed="(\w+)" aria-label="([^"]+)">/g)].map((m) => [m[1], m[2], m[3]]);
    expect(btns).toEqual([
      ["editor", "false", "Editor, 1 open"],
      ["board", "false", "Board, 1 open"],
      ["card", "true", "Card, 3 open"],
    ]);
    expect(bar, "no browser tooltips: the strip draws its own (GA-300)").not.toContain("title=");
    expect(bar.indexOf("lucide-file-code")).toBeLessThan(bar.indexOf("lucide-columns-3"));
    expect(bar.indexOf("lucide-columns-3")).toBeLessThan(bar.indexOf("lucide-square-kanban"));
    S.closeSideTabs(["card:GA-1", "card:GA-2", "card:GA-3", "board:hersir", "editor:src/a.ts"], "c9");
    expect(chatDiv(page("c9")), "with all closed, nothing").toBe("chat");
  });

  it("stays while the island is hidden, with nothing selected; the conversation then has the room", () => {
    S.showChat("c1");
    S.hideSide();
    const html = page("c1");
    expect(chatDiv(html)).toBe("chat has-strip");
    expect(html).not.toContain('<div class="chat-side">');
    const bar = strip(html);
    expect(bar).toMatch(/data-kind="terminal" aria-pressed="false" aria-label="Terminal, 2 open">/);
    expect(bar).toContain('<span class="n">2</span>');
    expect(f.list.map((s) => s.id), "the terminals keep running").toEqual(["t1", "t2"]);
    S.showSide();
    expect(chatDiv(page("c1")), "shown again").toBe("chat has-side has-strip");
  });
});

describe("tabs belong to each chat", () => {
  it("another chat, New chat and the Archive show neither chat 1's island nor its strip", () => {
    for (const [id, archive] of [["c2", false], [undefined, false], [undefined, true]] as const) {
      const html = page(id, archive);
      expect([id, archive, chatDiv(html)]).toEqual([id, archive, "chat"]);
      expect(html).not.toContain("data-term=");
      expect(html).not.toContain("chat-strip");
    }
    expect(f.list.map((s) => s.id), "chat 1's terminals still run").toEqual(["t1", "t2"]);
  });

  it("chat 2 gets its own terminal, and chat 1 still has its own two, the one it showed still shown", async () => {
    S.showChat("c2");
    await open(session("t3", "doq-studio 3"));
    const two = page("c2");
    expect([...two.matchAll(/data-term="(t\d)"/g)].map((m) => m[1])).toEqual(["t3"]);
    expect(strip(two)).toContain('aria-label="Terminal, 1 open"');
    const one = page("c1");
    expect(chatDiv(one)).toBe("chat has-side has-strip");
    expect([...one.matchAll(/data-term="(t\d)"/g)].map((m) => m[1])).toEqual(["t1", "t2"]);
    expect(one).toMatch(/data-term="t2"[^>]*aria-selected="true"/);
    expect(strip(one)).toContain('aria-label="Terminal, 2 open"');
  });

  it("the strip goes when a chat's last terminal closes; the other chat keeps its own", () => {
    f.list = f.list.filter((s) => s.id !== "t3");
    f.subs.forEach((g) => g());
    expect(chatDiv(page("c2"))).toBe("chat");
    expect(chatDiv(page("c1"))).toBe("chat has-side has-strip");
    f.list = [];
    f.subs.forEach((g) => g());
    const html = page("c1");
    expect(chatDiv(html)).toBe("chat");
    expect(html).not.toContain("chat-side");
    expect(html).not.toContain("chat-strip");
  });
});

/** The declarations of the rule with exactly this selector list. */
function rule(css: string, selector: string): string {
  const flat = css.replace(/\/\*[\s\S]*?\*\//g, "");
  const at = flat.indexOf(`${selector} {`);
  return at < 0 ? "" : flat.slice(at + selector.length + 2, flat.indexOf("}", at));
}

describe("the layout's CSS", () => {
  it("draws the chat list, the conversation and the side island as islands on the background, 6px apart, with no lines", () => {
    expect(rule(appCss, ".page-island:has(> .chat)")).toMatch(/background: none;\s*border-radius: 0;/);
    expect(rule(appCss, ".chat")).toContain("column-gap: var(--island-gap);");
    for (const [css, sel] of [[appCss, ".chat-threads"], [appCss, ".chat-main"], [terminalCss, ".chat-side-in"]] as const) {
      const r = rule(css, sel);
      expect(r, sel).toContain("border-radius: var(--island-radius);");
      expect(r, sel).toContain("background: var(--island);");
      expect(r, `${sel}: no border`).not.toMatch(/border(-left|-right|-top|-bottom)?:/);
    }
    expect(rule(appCss, ".chat-composer"), "no line above the composer").not.toContain("border");
    expect(rule(terminalCss, ".island-head"), "no line under the island's header").not.toContain("border");
  });

  it("centres the conversation without the island, and starts it at the left of its column while the island shows", () => {
    expect(rule(componentsCss, ".chat-col")).toMatch(/max-width: calc\(760px \* var\(--chat-scale, 1\)\);\s*margin: 0 auto;\s*padding: 24px;/);
    expect(rule(appCss, ".chat.has-side .chat-col").trim()).toBe("margin-left: 0;");
  });

  it("puts the strip in a last column, size-tool wide, while the chat has anything open", () => {
    const has = rule(appCss, ".chat.has-strip");
    expect(has).toContain("--chat-strip: var(--size-tool);");
    expect(has).toContain("grid-template-columns: var(--chat-list) minmax(0, 1fr) var(--chat-strip);");
  });

  it("gives the island its share of the room right of the chat list, at least 360px, leaving the conversation 400px", () => {
    const has = rule(appCss, ".chat.has-side");
    expect(has).toContain("--chat-room: calc(100% - var(--chat-list) - 3 * var(--island-gap) - var(--chat-strip));");
    expect(has).toContain("--chat-side-w: clamp(360px, calc(var(--chat-room) * var(--chat-side-share, .45)), calc(var(--chat-room) - 400px));");
    expect(has).toContain("grid-template-columns: var(--chat-list) minmax(0, 1fr) var(--chat-side-w) var(--chat-strip);");
  });

  it("hides the chat list when it has no room, and the strip stays as the last column", () => {
    expect(rule(appCss, ".chat.no-list > .chat-threads").trim()).toBe("display: none;");
    expect(rule(appCss, ".chat.has-strip.no-list")).toContain("grid-template-columns: minmax(0, 1fr) var(--chat-strip);");
    const side = rule(appCss, ".chat.has-side.no-list");
    expect(side).toContain("grid-template-columns: minmax(0, 1fr) var(--chat-side-w) var(--chat-strip);");
    // the room is then the whole chat less two gaps and the strip: --chat-list makes up for the third gap
    expect(side).toContain("--chat-list: calc(-1 * var(--island-gap));");
  });

  it("draws the chat list as wide as chatListWidth says: 260px, 220px up to 1100px, 6px more per pixel of text size", () => {
    expect(rule(appCss, ".chat")).toContain("--chat-list: calc(260px + 6 * var(--ui-grow, 0px));");
    expect(appCss).toMatch(/@media \(max-width: 1100px\) \{ \.chat \{ --chat-list: calc\(220px \+ 6 \* var\(--ui-grow, 0px\)\); \} \}/);
  });

  it("draws a strip button 28 × 42, radius 6, no fill and text-2; the selected one a pill in the island's colour; focus outside", () => {
    const btn = rule(terminalCss, ".strip-btn");
    expect(btn).toContain("width: var(--size-tool);");
    expect(btn).toContain("height: 42px;");
    expect(btn).toContain("border-radius: var(--radius);");
    expect(btn).toContain("background: none;");
    expect(btn).toContain("color: var(--text-2);");
    expect(rule(terminalCss, ".strip-btn:hover")).toMatch(/background: var\(--canvas-hover\);\s*color: var\(--text\);/);
    expect(rule(terminalCss, '.strip-btn[aria-pressed="true"]')).toMatch(/background: var\(--island\);\s*color: var\(--text\);/);
    expect(rule(terminalCss, ".strip-btn:focus-visible")).toMatch(/outline: 2px solid var\(--focus\);\s*outline-offset: 2px;/);
    expect(rule(terminalCss, ".chat-strip"), "on the background: no fill").not.toContain("background");
    expect(rule(terminalCss, ".chat-strip")).toContain("gap: 4px;");
  });

  it("the strip's menu: its head in text-3, Close all in danger", () => {
    expect(rule(terminalCss, ".pop .pop-head")).toContain("color: var(--text-3);");
    expect(rule(terminalCss, ".pop .opt.danger")).toContain("color: var(--danger);");
  });
});
