// GA-309 QA: what Rename on a chat counts on from the backend, read as text (Vite's ?raw), so a change on one side without
// the other fails here. The UI's renameChat calls the Tauri command rename_chat, registered beside pin_chat, with the
// chat's id and the name; it renames through chat::rename_thread off the main thread and sends the notice "chat_threads",
// which reloads Recent, Pinned, a chat read on its own (the top bar's crumbs), the Inbox and the Archive's search. The
// UI's longest name is the backend's; a rename adds no migration.
import { describe, expect, it, vi } from "vitest";
import { READS } from "./lib/tables";
import libRs from "../src-tauri/src/lib.rs?raw";
import commandsRs from "../src-tauri/src/commands.rs?raw";
import chatRs from "../crates/gizai-core/src/chat.rs?raw";
import archiveTsx from "./components/chat/ChatArchive.tsx?raw";
import tasksPageTsx from "./pages/TasksPage.tsx?raw";

const invoked: [string, unknown][] = [];
vi.mock("@tauri-apps/api/core", () => ({ invoke: (cmd: string, args?: unknown) => { invoked.push([cmd, args]); return Promise.resolve(null); } }));
vi.mock("@tauri-apps/api/event", () => ({ listen: () => Promise.resolve(() => {}) }));

const api = await import("./api");
const { NAME_MAX } = await import("./components/chat/useRenames");
const migrations = import.meta.glob("../crates/gizai-core/migrations/*.sql", { query: "?raw", import: "default", eager: true });

/** A command's body in src-tauri/src/commands.rs, up to the next one. */
const body = (cmd: string) => {
  const at = commandsRs.indexOf(`pub async fn ${cmd}(`);
  return at < 0 ? "" : commandsRs.slice(at, commandsRs.indexOf("#[tauri::command]", at));
};

describe("Rename's command", () => {
  it("is registered with Tauri beside pin_chat, and the UI sends what it takes", async () => {
    invoked.length = 0;
    await api.renameChat("c1", "Hersir sidebar");
    expect(invoked).toEqual([["rename_chat", { threadId: "c1", title: "Hersir sidebar" }]]);
    const handler = /generate_handler!\[([\s\S]*?)\]\)/.exec(libRs)?.[1] ?? "";
    expect(handler).toMatch(/commands::rename_chat\b/);
    expect(handler).toMatch(/commands::pin_chat, commands::unpin_chat, commands::rename_chat\b/);
    const args = new RegExp(`pub async fn rename_chat\\(([^)]*)\\)`).exec(commandsRs)?.[1].replace(/^app: AppHandle, st: State<'_, AppState>,?/, "").trim();
    expect(args).toBe("thread_id: String, title: String");
  });

  it("answers the renamed chat", () => {
    expect(commandsRs).toMatch(/pub async fn rename_chat\([^)]*\) -> R<gizai_core::chat::ChatThread>/);
  });

  it("renames off the main thread through chat::rename_thread as you, then sends chat_threads (and only that)", () => {
    const b = body("rename_chat");
    expect(b).toContain("off(&st, move |st| gizai_core::chat::rename_thread(&st.db, &st.you_id, &thread_id, &title)");
    expect(b).toContain('changed(&app, "chat_threads")');
    expect(b.match(/changed\(/g)).toHaveLength(1);
    // the notice goes after the write succeeded: a refused rename sends none
    expect(b.indexOf(".await?")).toBeLessThan(b.indexOf('changed(&app, "chat_threads")'));
  });
});

describe("the notice chat_threads", () => {
  it("reloads Recent, Pinned and a chat read on its own (the top bar's crumbs)", () => {
    expect(READS.list_chat_threads).toContain("chat_threads");
    expect(READS.pinned_chats).toContain("chat_threads");
    expect(READS.get_chat_thread).toContain("chat_threads");
  });

  it("runs the Archive's search again", () => {
    expect(archiveTsx).toMatch(/table\.startsWith\("chat"\) && table !== "chat_pins"/);
  });

  it("reaches the Inbox, which lists the chats with listChatThreads", () => {
    expect(tasksPageTsx).toMatch(/inbox \? listChatThreads\(\)/);
  });
});

describe("the longest name", () => {
  it("is 80 in the UI and in the backend", () => {
    expect(NAME_MAX).toBe(80);
    expect(chatRs).toContain("pub const NAME_MAX: usize = 80;");
  });

  it("is longer than a first message's title (60 and …), so editing that one never hits it", () => {
    const titleMax = Number(/const TITLE_MAX: usize = (\d+);/.exec(chatRs)?.[1]);
    expect(titleMax).toBe(60);
    expect(NAME_MAX).toBeGreaterThan(titleMax + 1);
  });
});

describe("no schema change for a rename", () => {
  it("adds no migration: the title column is the one a chat already has", () => {
    const names = Object.keys(migrations).map((p) => p.split("/").pop()!);
    for (const n of names) expect(n).not.toMatch(/rename|chat_name/i);
    for (const sql of Object.values(migrations)) expect(String(sql)).not.toMatch(/chat_threads\s+ADD\s+COLUMN\s+(name|custom_title)/i);
    expect(Object.values(migrations).some((sql) => /CREATE TABLE[^;]*chat_threads[\s\S]*?\btitle\b/i.test(String(sql)))).toBe(true);
  });
});
