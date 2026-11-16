// GA-257 QA: what Chat → Pinned counts on from the backend, read as text (Vite's ?raw), so a change on one side without the
// other fails here. The UI's pinnedChats, pinChat and unpinChat call Tauri commands that are registered and take what the UI
// sends; a pin or an unpin sends the notice "chat_pins", which reloads Pinned (and not Recent or the Archive's search); the
// pins are a setting, so they add no migration (the schema is at 22 for GA-261, not for the pins).
import { describe, expect, it, vi } from "vitest";
import { READS } from "./lib/tables";
import libRs from "../src-tauri/src/lib.rs?raw";
import commandsRs from "../src-tauri/src/commands.rs?raw";
import chatRs from "../crates/gizai-core/src/chat.rs?raw";
import dbRs from "../crates/gizai-core/src/db.rs?raw";
import archiveTsx from "./components/chat/ChatArchive.tsx?raw";

const invoked: [string, unknown][] = [];
vi.mock("@tauri-apps/api/core", () => ({ invoke: (cmd: string, args?: unknown) => { invoked.push([cmd, args]); return Promise.resolve(null); } }));
vi.mock("@tauri-apps/api/event", () => ({ listen: () => Promise.resolve(() => {}) }));

const api = await import("./api");
const migrations = import.meta.glob("../crates/gizai-core/migrations/*.sql", { query: "?raw", import: "default", eager: true });

/** A command's body in src-tauri/src/commands.rs, up to the next one. */
const body = (cmd: string) => {
  const at = commandsRs.indexOf(`pub async fn ${cmd}(`);
  return at < 0 ? "" : commandsRs.slice(at, commandsRs.indexOf("#[tauri::command]", at));
};

describe("Chat → Pinned's commands", () => {
  it("are registered with Tauri, and the UI sends what they take", async () => {
    invoked.length = 0;
    await api.pinnedChats();
    await api.pinChat("c1");
    await api.unpinChat("c2");
    expect(invoked).toEqual([["pinned_chats", undefined], ["pin_chat", { threadId: "c1" }], ["unpin_chat", { threadId: "c2" }]]);
    const handler = /generate_handler!\[([\s\S]*?)\]\)/.exec(libRs)?.[1] ?? "";
    for (const [cmd] of invoked) expect(handler, cmd).toMatch(new RegExp(`commands::${cmd}\\b`));
    const args = (cmd: string) => new RegExp(`pub async fn ${cmd}\\(([^)]*)\\)`).exec(commandsRs)?.[1].replace(/^(app: AppHandle, )?st: State<'_, AppState>,?/, "").trim();
    expect(args("pinned_chats")).toBe("");
    expect(args("pin_chat")).toBe("thread_id: String");
    expect(args("unpin_chat")).toBe("thread_id: String");
  });

  it("pin and unpin write through set_pinned and send chat_pins; the list is read with pinned_threads", () => {
    expect(body("pinned_chats")).toContain("gizai_core::chat::pinned_threads(&st.db)");
    expect(body("pin_chat")).toContain("set_pinned(&st.db, &thread_id, true)");
    expect(body("unpin_chat")).toContain("set_pinned(&st.db, &thread_id, false)");
    for (const cmd of ["pin_chat", "unpin_chat"]) expect(body(cmd), cmd).toContain('changed(&app, "chat_pins")');
    expect(body("pinned_chats")).not.toContain("changed(");
    expect(chatRs).toContain('pub const PINS: &str = "chat_pins";');
  });
});

describe("the notice chat_pins", () => {
  it("reloads Pinned, which also follows the chats' own changes", () => {
    expect(READS.pinned_chats).toContain("chat_pins");
    for (const t of READS.list_chat_threads) expect(READS.pinned_chats).toContain(t);
  });

  it("doesn't reload Recent or a chat read on its own", () => {
    expect(READS.list_chat_threads).not.toContain("chat_pins");
    expect(READS.get_chat_thread).not.toContain("chat_pins");
  });

  it("doesn't run the Archive's search again", () => {
    expect(archiveTsx).toMatch(/table\.startsWith\("chat"\) && table !== "chat_pins"/);
  });
});

describe("no schema change for the pins", () => {
  // The schema is at 22 since GA-261's 0022_outside_marks.sql (a chat's outside mark, GA-101's hold kind); the pins add no
  // migration of their own.
  it("keeps SCHEMA_VERSION at 22 with GA-261's migration last, and no migration for the pins", () => {
    expect(dbRs).toMatch(/pub const SCHEMA_VERSION: i64 = 22;/);
    const names = Object.keys(migrations).map((p) => p.split("/").pop()).sort();
    expect(names).toHaveLength(22);
    expect(names.at(-1)).toBe("0022_outside_marks.sql");
    for (const sql of Object.values(migrations)) expect(String(sql)).not.toContain("chat_pins");
  });
});
