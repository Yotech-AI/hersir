// GA-232: the backend's chat note that the Chief's tools didn't connect says Hersir now; the chat still shows it as a
// failure (red, role alert), as it does an older chat's note with Gizai (and with the Team Lead, before GA-255). Rendered
// to HTML on the server.
import { describe, expect, it, vi } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import type { ChatMessage } from "../../types";
import chatRs from "../../../src-tauri/src/chat.rs?raw";

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

describe("the chat's note that the Chief's tools didn't connect (GA-232)", () => {
  const why = "didn't connect (failed), so the Team Lead can't look anything up or change anything in this answer.";

  // GA-255: the backend names the Chief now.
  it("is the backend's text, which says Hersir and the Chief", () => {
    expect(chatRs).toContain(`format!("Hersir's tools didn't connect ({s}), so the Chief can't look anything up or change anything in this answer.")`);
  });

  it("shows as a failure with the Chief's name, as the backend says it since GA-255", () => {
    expect(bad(render("Hersir's tools didn't connect (failed), so the Chief can't look anything up or change anything in this answer."))).toBe(true);
  });

  it("shows as a failure with Hersir, and with Gizai in a chat from before the rename", () => {
    expect(bad(render(`Hersir's tools ${why}`))).toBe(true);
    expect(bad(render(`Gizai's tools ${why}`))).toBe(true);
  });

  it("leaves other notes plain", () => {
    expect(bad(render("Hersir kept 2 screenshots on the card."))).toBe(false);
    expect(bad(render(`Otus's tools ${why}`))).toBe(false);
  });
});
