// GA-351 QA: Hersir's chat shows a message from Claude Desktop (its ask_chief, meta {from: "claude-desktop"}) with a small
// "From Claude Desktop" line over it, sent or queued; a message you typed, or one with other details, has none. Rendered to
// HTML on the server.
import { describe, expect, it, vi } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import type { ChatMessage, ChatNoteMeta } from "../../types";
import { FROM_CLAUDE_DESKTOP, fromClaudeDesktop } from "../../lib/chat";

vi.mock("../../api", async (orig) => {
  const real = await orig<Record<string, unknown>>();
  const out: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(real)) out[k] = typeof v !== "function" ? v : () => new Promise(() => {});
  return out;
});

const { MessageGroup, UserBubble } = await import("./ChatMessage");

const LINE = '<div class="msg-from">From Claude Desktop</div>';
const yours = (id: string, bodyMd: string, meta?: ChatNoteMeta | null): ChatMessage =>
  ({ id, threadId: "c1", role: "user", bodyMd, createdAt: 0, meta });
const group = (items: ChatMessage[]) => renderToStaticMarkup(<MessageGroup g={{ side: "user", key: items[0].id, at: 0, items }} />);

describe("fromClaudeDesktop", () => {
  it("is true only for from: claude-desktop", () => {
    expect(FROM_CLAUDE_DESKTOP).toBe("From Claude Desktop");
    expect(fromClaudeDesktop({ from: "claude-desktop" })).toBe(true);
    for (const meta of [null, undefined, {}, { from: "claude" }, { from: "Claude-Desktop" }, { kind: "switch", cli: "claude-desktop" }] as (ChatNoteMeta | null | undefined)[]) {
      expect(fromClaudeDesktop(meta), JSON.stringify(meta)).toBe(false);
    }
  });
});

describe("a message from Claude Desktop in Hersir's chat", () => {
  it("shows the small line over its bubble", () => {
    const html = group([yours("m1", "Which cards are stuck?", { from: "claude-desktop" })]);
    expect(html).toContain(`<div class="chat-msg user">${LINE}<div class="bubble">`);
    expect(html).toContain("Which cards are stuck?");
  });

  it("a message you typed has no line, also beside one from Claude Desktop in the same group", () => {
    expect(group([yours("m1", "And KADE-1?")])).not.toContain("msg-from");
    expect(group([yours("m1", "And KADE-1?", null)])).not.toContain("From Claude Desktop");
    const html = group([yours("m1", "Which cards are stuck?", { from: "claude-desktop" }), yours("m2", "And KADE-1?")]);
    expect(html.split(LINE).length - 1).toBe(1);
    expect(html.indexOf(LINE)).toBeLessThan(html.indexOf("Which cards are stuck?"));
    expect(html.indexOf("And KADE-1?")).toBeGreaterThan(html.lastIndexOf(LINE));
  });

  it("a message with other details (a note's kind) has no line", () => {
    expect(group([yours("m1", "Hi", { kind: "switch", cli: "claude", cliName: "Claude Code" })])).not.toContain("msg-from");
  });

  it("the queued message's bubble (UserBubble with its meta) shows the line too, and one you typed doesn't", () => {
    expect(renderToStaticMarkup(<UserBubble text="Queued from Claude" meta={{ from: "claude-desktop" }} />)).toBe(
      `${LINE}<div class="bubble">Queued from Claude</div>`);
    expect(renderToStaticMarkup(<UserBubble text="Queued by you" />)).toBe('<div class="bubble">Queued by you</div>');
  });

  it("a message of files only from Claude Desktop still says where it came from", () => {
    expect(renderToStaticMarkup(<UserBubble text="  " meta={{ from: "claude-desktop" }} />)).toBe(LINE);
  });
});
