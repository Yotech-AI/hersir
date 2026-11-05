// GA-231 QA: a note in the chat (side "system") that says the Team Lead's tools didn't connect shows as a failure (red,
// role alert), whether it names Hersir (the backend since GA-232) or Gizai (older chats, and the backend before GA-232).
// Other notes stay plain. Rendered to HTML on the server.
import { describe, expect, it, vi } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import type { ChatMessage } from "../../types";

vi.mock("../../api", async (orig) => {
  const real = await orig<Record<string, unknown>>();
  const out: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(real)) out[k] = typeof v !== "function" ? v : () => new Promise(() => {});
  return out;
});

const { MessageGroup } = await import("./ChatMessage");

const note = (bodyMd: string): ChatMessage => ({ id: "m1", threadId: "c1", role: "system", bodyMd, createdAt: 0 });
const render = (bodyMd: string) => renderToStaticMarkup(<MessageGroup g={{ side: "system", key: "m1", at: 0, items: [note(bodyMd)] }} />);
const bad = (html: string) => html.startsWith('<div class="chat-note bad" role="alert">');

describe("the chat's note that the tools didn't connect", () => {
  const why = "didn't connect (failed), so the Team Lead can't look anything up or change anything in this answer.";

  it("is a failure with Hersir, as the backend says it since GA-232", () => {
    const html = render(`Hersir's tools ${why}`);
    expect(bad(html)).toBe(true);
    expect(html).toContain(`<span>Hersir&#x27;s tools ${why.replaceAll("'", "&#x27;")}</span>`);
  });

  it("is a failure with Gizai too, in chats from before GA-232", () => {
    expect(bad(render(`Gizai's tools ${why}`))).toBe(true);
  });

  it("another note stays plain: the name alone, or the words further on, don't make it a failure", () => {
    for (const text of ["Hersir moved this chat to Claude Code 2.", "The answer says Hersir's tools didn't connect.", `Otus's tools ${why}`]) {
      const html = render(text);
      expect(bad(html), text).toBe(false);
      expect(html.startsWith('<div class="chat-note">')).toBe(true);
    }
  });

  it("The Team Lead couldn't … stays a failure, as before", () => {
    expect(bad(render("The Team Lead couldn't answer: Claude Code exited."))).toBe(true);
  });
});
