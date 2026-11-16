// GA-231 QA: the name in the app's screens is Hersir. No text a person reads in src/ (headings, labels, buttons, hints,
// toasts, dialogs, aria-labels and titles) says Gizai or GizAi any more: what is left is in comments, the readers that take
// the backend's texts with either name (until GA-232, and older runs and chats), the in-app self-test and the browser
// mode's fixtures (the backend's texts, GA-232's). The code's names stay: storage keys and events (settings, drafts and
// open tabs survive an update), the MCP server name gizai, gizai: links, GIZAI_RESULT and the logo's file. GA-249 gave the
// template file (<team>.hersir-team.json) and the GIZAI_* variables their hersir names; the GIZAI_* ones are set too.
// Files are read from disk; node:fs comes in through a computed specifier, as in band.test.ts.
import { describe, expect, it } from "vitest";
import { KEYS } from "./lib/appearance";
import { nameProblem, TAKEN_NAMES } from "./lib/mcp";
import { itemLink } from "./lib/itemLinks";
import { templateFileName } from "./components/TeamTemplate";

const fs = await import(/* @vite-ignore */ ["node", "fs"].join(":"));
const read = (path: string): string => fs.readFileSync(new URL(`../${path}`, import.meta.url), "utf8");

// Every file of the app's screens, but the tests, the self-test and the browser mode's fixtures.
const files = (fs.readdirSync(new URL("./", import.meta.url), { recursive: true }) as string[])
  .map((f) => `src/${f.replace(/\\/g, "/")}`)
  .filter((f) => /\.(tsx?|svg|css|html)$/.test(f) && !/\.test\.tsx?$/.test(f) && f !== "src/selftest.ts");

// The comments out (block, line and HTML ones), keeping the line numbers; a // after a colon is a URL's.
const withoutComments = (code: string) => code
  .replace(/\/\*[\s\S]*?\*\/|<!--[\s\S]*?-->/g, (c) => c.replace(/[^\n]/g, " "))
  .replace(/(^|[^:\\])\/\/.*$/gm, "$1");

// The three places that read a backend text with either name (GA-231's hand-over): a run's push note, Changes' error
// and the chat's tools note.
const EITHER = /\((\?:)?Hersir\|Gizai\)/;

describe("no text in the app's screens says Gizai or GizAi (GA-231)", () => {
  it("finds the files: the pages, the components, the lib, the logo and the browser mode", () => {
    expect(files.length).toBeGreaterThan(150);
    for (const f of ["src/pages/OnboardingPage.tsx", "src/components/QuitSettings.tsx", "src/lib/onboarding.ts", "src/assets/gizai-logo.svg", "src/browser/mock.ts"])
      expect(files).toContain(f);
  });

  it("each Gizai or GizAi left is in a comment, or a reader that takes both names", () => {
    const left: string[] = [];
    for (const f of files) {
      withoutComments(read(f)).split("\n").forEach((line, i) => {
        if (/\bGiz[Aa][Ii]\b/.test(line) && !EITHER.test(line)) left.push(`${f}:${i + 1}: ${line.trim().slice(0, 140)}`);
      });
    }
    expect(left).toEqual([]);
    // and the texts themselves are still there with the comments out: GA-231 changed 92
    const hersir = files.reduce((n, f) => n + (withoutComments(read(f)).match(/\bHersir\b/g)?.length ?? 0), 0);
    expect(hersir).toBeGreaterThan(90);
  });

  it("the readers that take both names are the three of the hand-over", () => {
    const readers = files.filter((f) => EITHER.test(withoutComments(read(f))));
    expect(readers.sort()).toEqual(["src/components/changes/ChangesView.tsx", "src/components/chat/ChatMessage.tsx", "src/lib/runs.ts"]);
  });
});

describe("Quit: the same words as the tray (GA-232)", () => {
  it("Settings → General says Quit Hersir completely, and its text names Open Hersir", () => {
    const quit = read("src/components/QuitSettings.tsx");
    expect(quit).toContain('"Quit Hersir completely"');
    expect(quit).toContain("Open Hersir in the tray, or starting Hersir again, brings the window back.");
    expect(quit).toContain('label="Quit Hersir"');
  });

  it("the tray's items are Open Hersir and Quit Hersir completely since GA-232, the same words", () => {
    const lib = read("src-tauri/src/lib.rs");
    const open = /MenuItem::with_id\(app, "open", "([^"]+)"/.exec(lib)?.[1];
    const quit = /MenuItem::with_id\(app, "quit", "([^"]+)"/.exec(lib)?.[1];
    expect(open).toBe("Open Hersir");
    expect(quit).toBe("Quit Hersir completely");
  });
});

describe("the code's names stay (GA-231): settings, drafts and open tabs survive an update", () => {
  it("the appearance's keys", () => {
    expect(KEYS).toEqual({ font: "gizai-font", chat: "gizai-size-chat", ui: "gizai-size-ui", docs: "gizai-size-docs", theme: "gizai-theme", density: "gizai-density", band: "gizai-band" });
  });

  it("the other storage keys and events, each where it was", () => {
    const where: Record<string, string[]> = {
      "src/lib/hand.ts": ['"gizai.cards.seen"'],
      "src/lib/editor.ts": ['"gizai.editor.opened"', '"gizai.editor.openedAt"', '"gizai.editor.tabs"', '"gizai-editor-opened"'],
      "src/lib/behind.ts": ['"gizai-editor-main"'],
      "src/lib/terminalPanel.ts": ['"gizai.editor.panel"', '"gizai.editor.terminal"'],
      "src/lib/sidebar.ts": ['"gizai.sidebar.folded"'],
      "src/lib/team.ts": ['"gizai-team"', '"gizai-team-changed"'],
      "src/pages/MemoryPage.tsx": ['"gizai.memory.open"', '"gizai.memory.folders"', '"gizai.memory.panel"', '"gizai.memory.graph"', '"gizai.memory.local"'],
      "src/components/memory/NotePane.tsx": ['"gizai.memory.mode"', '"gizai.memory.closed"'],
      "src/pages/DocPage.tsx": ['"gizai.doc.mode"'],
      "src/pages/TasksPage.tsx": ['"gizai-task-filter"', '"gizai-task-project"', '"gizai-tasks-view"', '"gizai-task-sort"', '"gizai-task-group"'],
      "src/pages/TaskPage.tsx": ['"gizai-props"'],
    };
    for (const [file, keys] of Object.entries(where)) for (const k of keys) expect(read(file), `${file} ${k}`).toContain(k);
    // no key was renamed to hersir
    expect(files.filter((f) => /["'`]hersir[.-]/i.test(read(f)))).toEqual([]);
  });

  it("the MCP server name gizai is Hersir's own and gizai: links stay; Export offers <team>.hersir-team.json (GA-249)", () => {
    expect(TAKEN_NAMES).toEqual(["gizai", "chrome-devtools"]);
    expect(nameProblem("gizai", [])).toBe("gizai is Hersir's own: give the server another name");
    expect(itemLink({ kind: "task", key: "GA-12", text: "GA-12 - Fix the login" } as Parameters<typeof itemLink>[0])).toBe("[GA-12 - Fix the login](gizai:task/GA-12)");
    expect(templateFileName("Software team")).toBe("Software team.hersir-team.json");
  });

  it("texts that show a name of the code keep it: GIZAI_RESULT, the logo's file; the preview's hint names HERSIR_PREVIEW_* and says GIZAI_PREVIEW_* are set too (GA-249)", () => {
    expect(read("src/components/CliSettings.tsx")).toContain("Hersir reads its output as plain text and looks for the GIZAI_RESULT line at the end.");
    const project = read("src/components/ProjectForm.tsx");
    expect(project).toContain("on a free port in HERSIR_PREVIEW_PORT (and PORT), with HERSIR_PREVIEW_DIR for a throwaway database. The GIZAI_PREVIEW_* names are set too. Serve demo data, never your .env database. Hersir ends it with the run.");
    expect(project).toContain(`placeholder={'npx astro dev --host 127.0.0.1 --port "$HERSIR_PREVIEW_PORT" --ignore-lock'}`);
    expect(read("src/pages/OnboardingPage.tsx")).toContain('import logo from "../assets/gizai-logo.svg?raw";');
  });
});
