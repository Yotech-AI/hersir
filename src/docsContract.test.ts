// Docs → Commits: the UI's calls and types match the Rust side, read as text (Vite's ?raw) as contract C1's are
// (changesContract.test.ts), so a change on one side without the other fails here.
import { describe, expect, it, vi } from "vitest";
import type { ChangelogEntry, CommitFiles, DecisionNote, DocsCommits, LoggedCommit } from "./types";
import libRs from "../src-tauri/src/lib.rs?raw";
import changesRs from "../src-tauri/src/changes.rs?raw";
import devdocsAppRs from "../src-tauri/src/devdocs.rs?raw";
import devdocsRs from "../crates/gizai-core/src/devdocs.rs?raw";
import worktreeRs from "../crates/gizai-agents/src/worktree.rs?raw";

const invoked: [string, unknown][] = [];
vi.mock("@tauri-apps/api/core", () => ({ invoke: (cmd: string, args?: unknown) => { invoked.push([cmd, args]); return Promise.resolve(null); } }));
vi.mock("@tauri-apps/api/event", () => ({ listen: () => Promise.resolve(() => {}) }));

const api = await import("./api");

/** The fields of a Rust struct, camelCased as serde's rename_all = "camelCase" sends them. */
function serdeFields(rs: string, name: string): string[] {
  const body = new RegExp(`#\\[serde\\(rename_all = "camelCase"\\)\\]\\s*pub struct ${name} \\{([\\s\\S]*?)\\n\\}`).exec(rs);
  if (!body) throw new Error(`no camelCase struct ${name}`);
  return [...body[1].matchAll(/^\s*pub (\w+):/gm)].map((m) => m[1].replace(/_(\w)/g, (_, c: string) => c.toUpperCase())).sort();
}

describe("Docs → Commits' commands", () => {
  it("are invoked with camelCase arguments and registered with Tauri", async () => {
    invoked.length = 0;
    await api.docsCommits("p1");
    await api.docsCommitFiles("p1", "abc1234");
    await api.docsFileDiff("p1", "src/a.ts", null, "a1b2c3d", "e4f5a6b");
    expect(invoked).toEqual([
      ["docs_commits", { projectId: "p1" }],
      ["docs_commit_files", { projectId: "p1", sha: "abc1234" }],
      ["docs_file_diff", { projectId: "p1", path: "src/a.ts", oldPath: null, from: "a1b2c3d", to: "e4f5a6b" }],
    ]);
    for (const cmd of ["docs_commits", "docs_commit_files", "docs_file_diff"]) {
      expect(libRs).toContain(`changes::${cmd}`);
      expect(changesRs).toMatch(new RegExp(`pub async fn ${cmd}\\(`));
    }
  });

  it("send the fields the UI's types name", () => {
    const commit: Record<keyof LoggedCommit, true> = { sha: true, parent: true, author: true, at: true, subject: true };
    expect(serdeFields(worktreeRs, "LoggedCommit")).toEqual(Object.keys(commit).sort());
    const list: Record<keyof DocsCommits, true> = { label: true, commits: true };
    expect(serdeFields(changesRs, "DocsCommits")).toEqual(Object.keys(list).sort());
    const files: Record<keyof CommitFiles, true> = { from: true, to: true, files: true };
    expect(serdeFields(changesRs, "CommitFiles")).toEqual(Object.keys(files).sort());
  });
});

describe("Docs → Changelog's and Decisions' commands", () => {
  it("are invoked with camelCase arguments and registered with Tauri", async () => {
    invoked.length = 0;
    await api.docsChangelog("p1");
    await api.docsDecisions("p1");
    expect(invoked).toEqual([["docs_changelog", { projectId: "p1" }], ["docs_decisions", { projectId: "p1" }]]);
    for (const cmd of ["docs_changelog", "docs_decisions"]) {
      expect(libRs).toContain(`devdocs::${cmd}`);
      expect(devdocsAppRs).toMatch(new RegExp(`pub async fn ${cmd}\\(`));
    }
  });

  it("send the fields the UI's types name", () => {
    const entry: Record<keyof ChangelogEntry, true> = { taskId: true, identifier: true, title: true, at: true };
    expect(serdeFields(devdocsRs, "ChangelogEntry")).toEqual(Object.keys(entry).sort());
    const note: Record<keyof DecisionNote, true> = { id: true, path: true, title: true, bodyMd: true, updatedAt: true };
    expect(serdeFields(devdocsRs, "DecisionNote")).toEqual(Object.keys(note).sort());
  });
});
