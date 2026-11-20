// GA-312 QA: a tool line says what happened. Each Hersir tool has a done verb and a base form: a call that worked reads as
// before ("Changed task DU-37"), a refused one reads "Couldn't change task DU-37" with the reason under it, and one Hersir
// kept for your OK reads "Waiting for your OK: add to note Chief/Notes", without a reason. No Hersir tool shows its raw
// name (mcp__gizai__…). The tools are read from the app's tool list (src-tauri/src/tools/mod.rs), so a new one is covered.
import { describe, expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { toolCard } from "./chat";
import { ToolCard } from "../components/chat/ChatMessage";
import type { ChatMessage } from "../types";

const fs = await import(/* @vite-ignore */ ["node", "fs"].join(":"));
const read = (path: string): string => fs.readFileSync(new URL(`../../${path}`, import.meta.url), "utf8");

const base: ChatMessage = { id: "m", threadId: "t", role: "tool", authorId: "a", authorName: "Chief", bodyMd: null, runId: "r", toolName: null, tool: null, createdAt: 1_000_000 };
const call = (name: string, tool: Record<string, unknown>): ChatMessage => ({ ...base, toolName: `mcp__gizai__${name}`, tool });

/** The app's texts, word for word (src-tauri/src/tools/mod.rs: not_run, refused_after and ONLY_THE_USER). */
const kept = (name: string, tool = "mcp__chrome-devtools__new_page") =>
  `${name} wasn't run: this chat used ${tool}, a tool from outside Hersir. Hersir keeps the call for the user to approve with one click. Tell them in a sentence what it does.`;
const notKept = (name: string, why: string) =>
  `${name} wasn't run: this chat used mcp__otus__search, a tool from outside Hersir, and Hersir couldn't keep the call for the user to approve (${why}). Tell the user what you would do, so they can do it themselves in Hersir.`;
const refusedAfter = (name: string) =>
  `${name} is refused in this chat: it used mcp__otus__search, a tool from outside Hersir, and what that returned could be trying to steer you. Tell the user what you would do, so they can do it themselves in Hersir.`;
const ONLY_THE_USER = "Only the user takes this hold off, on the card: it waits for their OK after a tool from outside Hersir.";

/** Every tool the Chief's MCP server offers, from its list in the app. */
const TOOLS = [...read("src-tauri/src/tools/mod.rs").matchAll(/\btool\("([a-z_]+)"/g)].map((m) => m[1]);
/** The tools with a line of their own in src/lib/chat.ts (`name: ["Done", "base"]`). */
const OWN = new Set([...read("src/lib/chat.ts").matchAll(/\b([a-z_]+): \["[A-Z]/g)].map((m) => m[1]));

describe("the three forms of a tool line (the card's examples)", () => {
  it("a refused update_task reads Couldn't change task DU-37, with its reason under it", () => {
    const m = call("update_task", { input: { task: "DU-37", clear_hold: true }, result: ONLY_THE_USER, isError: true });
    expect(toolCard(m)).toEqual({ verb: "Couldn't change task", label: "DU-37", state: "error", detail: ONLY_THE_USER });
    const html = renderToStaticMarkup(<ToolCard m={m} />);
    expect(html).toContain('<div class="tool-card error">');
    expect(html).toContain('<span class="tc-verb">Couldn&#x27;t change task</span><span class="tc-label">DU-37</span>');
    expect(html).toContain(`<div class="tc-error">${ONLY_THE_USER}</div>`);
    expect(html).not.toContain("Changed task");
  });

  it("a kept memory_append reads Waiting for your OK: add to note Chief/Notes, with no reason under it", () => {
    const m = call("memory_append", { input: { note: "Chief/Notes", text: "- 17 Nov: DU-37 to DU-42 wait for an OK." }, result: kept("memory_append"), isError: true });
    expect(toolCard(m)).toEqual({ verb: "Waiting for your OK: add to note", label: "Chief/Notes", state: "waiting" });
    const html = renderToStaticMarkup(<ToolCard m={m} />);
    expect(html).toContain('<div class="tool-card waiting">');
    expect(html).toContain('<span class="tc-verb">Waiting for your OK: add to note</span><span class="tc-label">Chief/Notes</span>');
    expect(html).toContain("lucide-clock");
    expect(html).not.toContain("tc-error");
  });

  it("an update_task that worked still reads Changed task DU-37, linked to the card", () => {
    const m = call("update_task", { input: { task: "DU-37", priority: 2 },
      result: JSON.stringify({ ok: true, done: "updated", task: { id: "t37", identifier: "DU-37" }, link: { page: "task", id: "t37", label: "DU-37 Docx import" } }), isError: false });
    expect(toolCard(m)).toEqual({ verb: "Changed task", label: "DU-37 Docx import", href: "#/task/t37", state: "ok" });
    const html = renderToStaticMarkup(<ToolCard m={m} />);
    expect(html).toContain('<div class="tool-card ok">');
    expect(html).toContain('<span class="tc-verb">Changed task</span>');
  });

  it("one that held its card still reads as done (the card waits under the answer, not the call)", () => {
    const m = call("move_task", { input: { task: "DU-38", column: "To do" }, isError: false,
      result: JSON.stringify({ ok: true, task: { id: "t38", identifier: "DU-38" }, link: { page: "task", id: "t38", label: "DU-38 Export" }, on_hold: "DU-38 waits on hold…" }) });
    expect(toolCard(m)).toMatchObject({ verb: "Moved task", label: "DU-38 Export", state: "ok" });
  });

  it("a call still running shows its done verb, as before", () => {
    expect(toolCard(call("update_task", { input: { task: "DU-37" } }))).toEqual({ verb: "Changed task", label: "DU-37", state: "running" });
  });
});

describe("which refusals are kept calls", () => {
  it("only the 'keeps the call' text reads as waiting; a call Hersir couldn't keep, or refused outright, reads Couldn't", () => {
    const why = "the chat has 50 waiting calls";
    expect(toolCard(call("memory_append", { input: { note: "Chief/Notes" }, result: notKept("memory_append", why), isError: true })))
      .toEqual({ verb: "Couldn't add to note", label: "Chief/Notes", state: "error", detail: notKept("memory_append", why) });
    expect(toolCard(call("start_agent_run", { input: { task: "DU-37" }, result: refusedAfter("start_agent_run"), isError: true })))
      .toMatchObject({ verb: "Couldn't start an agent on", label: "DU-37", state: "error" });
  });

  it("GA-312 fix round: a call GA-102 kept (2 to 9 Nov: 'this answer used') reads as waiting too; one it couldn't keep reads Couldn't", () => {
    const old = (name: string) => `${name} wasn't run: this answer used mcp__otus__search, a tool from outside Hersir. Hersir keeps the call for the user to approve with one click. Tell them in a sentence what it does.`;
    const oldNotKept = "memory_append wasn't run: this answer used mcp__otus__search, a tool from outside Hersir, and Hersir couldn't keep the call for the user to approve (full). Tell the user what you would do and ask them to confirm it in a new message.";
    expect(toolCard(call("memory_append", { input: { note: "Chief/Notes" }, result: old("memory_append"), isError: true })))
      .toEqual({ verb: "Waiting for your OK: add to note", label: "Chief/Notes", state: "waiting" });
    expect(toolCard(call("memory_append", { input: { note: "Chief/Notes" }, result: oldNotKept, isError: true })))
      .toMatchObject({ verb: "Couldn't add to note", label: "Chief/Notes", state: "error" });
  });

  it("each kept call names what it would do, with the note's whole path for memory", () => {
    const cases: [string, Record<string, unknown>, string, string][] = [
      ["memory_write", { path: "Chief/Notes", body_md: "x" }, "Waiting for your OK: save note", "Chief/Notes"],
      ["memory_move", { note: "Agents/QA Agent/Notes", to: "Lessons/" }, "Waiting for your OK: move note", "Agents/QA Agent/Notes"],
      ["comment_on_task", { task: "DU-37", body_md: "Go." }, "Waiting for your OK: comment on", "DU-37"],
      ["start_agent_run", { task: "DU-37" }, "Waiting for your OK: start an agent on", "DU-37"],
      ["merge_pull_request", { task: "DU-37" }, "Waiting for your OK: merge the pull request of", "DU-37"],
      ["set_column", { column: "Deploy", auto: true }, "Waiting for your OK: change column", "Deploy"],
    ];
    for (const [name, input, verb, label] of cases)
      expect(toolCard(call(name, { input, result: kept(name, "WebFetch"), isError: true })), name).toEqual({ verb, label, state: "waiting" });
  });

  it("another server's refused tool keeps its own name (no base form to say what failed)", () => {
    const m: ChatMessage = { ...base, toolName: "mcp__otus__get_card", tool: { input: {}, result: "Not found", isError: true } };
    expect(toolCard(m)).toMatchObject({ verb: "mcp__otus__get_card", state: "error", detail: "Not found" });
    // and Claude Code's own: its done verb, with the reason
    expect(toolCard({ ...base, toolName: "Read", tool: { input: { file_path: "/x/a.md" }, result: "No such file", isError: true } }))
      .toMatchObject({ verb: "Couldn't read a file", label: "a.md", state: "error", detail: "No such file" });
  });
});

describe("every Hersir tool shows a plain verb, never mcp__gizai__…", () => {
  it("finds the app's tools, the ones the card names among them", () => {
    expect(TOOLS.length).toBeGreaterThanOrEqual(46);
    for (const t of ["memory_append", "check_board", "merge_pull_request", "start_chat", "continue_agent_run", "update_checkout", "save_label",
      "set_column", "memory_list", "memory_search", "memory_read", "memory_write", "memory_move"]) expect(TOOLS).toContain(t);
  });

  it("the card's words for memory_append and check_board", () => {
    const ok = (name: string) => toolCard(call(name, { input: {}, result: "{}", isError: false })).verb;
    const no = (name: string) => toolCard(call(name, { input: {}, result: "No.", isError: true })).verb;
    expect([ok("memory_append"), no("memory_append")]).toEqual(["Added to note", "Couldn't add to note"]);
    expect([ok("check_board"), no("check_board")]).toEqual(["Checked the board", "Couldn't check the board"]);
    expect([ok("update_task"), no("update_task")]).toEqual(["Changed task", "Couldn't change task"]);
  });

  it("each one, done, refused, kept and running, in words of its own", () => {
    const bad: string[] = [];
    for (const name of TOOLS) {
      const done = toolCard(call(name, { input: {}, result: "{}", isError: false })).verb;
      const running = toolCard(call(name, { input: {} })).verb;
      const refused = toolCard(call(name, { input: {}, result: "No.", isError: true })).verb;
      const waiting = toolCard(call(name, { input: {}, result: kept(name), isError: true }));
      const plain = (s: string) => !/mcp__|_|gizai/i.test(s);
      // its own line in VERBS, not the fallback made from its name ("Memory append")
      if (!OWN.has(name)) bad.push(`${name} has no line of its own`);
      if (!plain(done) || !/^[A-Z]/.test(done)) bad.push(`${name} done: ${done}`);
      if (running !== done) bad.push(`${name} running: ${running}`);
      if (!plain(refused) || !/^Couldn't [a-z]/.test(refused)) bad.push(`${name} refused: ${refused}`);
      if (waiting.state !== "waiting" || !/^Waiting for your OK: [a-z]/.test(waiting.verb) || waiting.verb.slice(21) !== refused.slice(9))
        bad.push(`${name} kept: ${waiting.verb} (${waiting.state})`);
    }
    expect(bad).toEqual([]);
  });
});

// GA-307 QA: the Chief's archive_tasks and restore_tasks read as Archived tasks and Restored tasks (the card's words),
// and an archive_tasks kept after a tool from outside Hersir reads as waiting for your OK.
describe("archive_tasks and restore_tasks (GA-307)", () => {
  const answer = JSON.stringify({ ok: true, done: "archived", archived: 2, tasks: ["GA-12", "GA-13"], skipped: [] });
  it("done, they read Archived tasks and Restored tasks", () => {
    expect(toolCard(call("archive_tasks", { input: { tasks: ["GA-12", "GA-13"] }, result: answer, isError: false })))
      .toEqual({ verb: "Archived tasks", label: undefined, state: "ok" });
    expect(toolCard(call("archive_tasks", { input: { all_done: true, project: "GA" } })))
      .toEqual({ verb: "Archived tasks", label: "GA", state: "running" });
    const restored = JSON.stringify({ ok: true, done: "restored", restored: 1, tasks: ["GA-12"], skipped: [] });
    expect(toolCard(call("restore_tasks", { input: { tasks: ["GA-12"] }, result: restored, isError: false })).verb).toBe("Restored tasks");
  });

  it("refused, they read Couldn't archive tasks and Couldn't restore tasks, with the reason under it", () => {
    const why = "A board check never archives cards: ask the user in a chat (start_chat) instead.";
    expect(toolCard(call("archive_tasks", { input: { all_done: true }, result: why, isError: true })))
      .toEqual({ verb: "Couldn't archive tasks", label: undefined, state: "error", detail: why });
    expect(toolCard(call("restore_tasks", { input: { tasks: ["GA-12"] }, result: "No.", isError: true })).verb).toBe("Couldn't restore tasks");
  });

  it("kept after a tool from outside Hersir, archive_tasks reads Waiting for your OK: archive tasks", () => {
    const m = call("archive_tasks", { input: { all_done: true }, result: kept("archive_tasks", "mcp__otus__search"), isError: true });
    expect(toolCard(m)).toEqual({ verb: "Waiting for your OK: archive tasks", label: undefined, state: "waiting" });
    expect(renderToStaticMarkup(<ToolCard m={m} />)).toContain('<span class="tc-verb">Waiting for your OK: archive tasks</span>');
  });

  it("both are in the app's tool list, each with a line of its own", () => {
    expect(TOOLS).toContain("archive_tasks");
    expect(TOOLS).toContain("restore_tasks");
    expect(OWN.has("archive_tasks") && OWN.has("restore_tasks")).toBe(true);
  });
});
