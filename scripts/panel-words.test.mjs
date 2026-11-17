// GA-305 QA: 'panel' and 'side panel', never 'island', in what people read. hersir.dev copies Hersir's user docs (GW-26),
// and the site must never say island: the README, docs/install.md, settings.md, memory.md, agent-tools.md and every doc
// under docs/features say panel, and the column beside the Chief Chat is the side panel. docs/features/editor-and-terminal.md
// says the conversation stays centred while the side panel shows (GA-302, 0.18.1). The app's own texts (labels, tooltips,
// titles, hints, the words in its markup) say it too. Class names, code identifiers, comments, design/ and the dated notes
// keep 'island': nobody reads them, and the site doesn't copy them.
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const ROOT = fileURLToPath(new URL("..", import.meta.url));
const read = (path) => readFileSync(join(ROOT, path), "utf8");
const FEATURES = readdirSync(join(ROOT, "docs/features")).filter((f) => f.endsWith(".md")).map((f) => `docs/features/${f}`);
// The card's list: the README and the user docs under docs/
const DOCS = ["README.md", "docs/install.md", "docs/settings.md", "docs/memory.md", "docs/agent-tools.md", ...FEATURES];
const ISLAND = /island/i;
/** Each line of `path` that says island, as "path:line: text". */
const islandLines = (path) => read(path).split("\n").flatMap((l, i) => (ISLAND.test(l) ? [`${path}:${i + 1}: ${l.trim()}`] : []));

describe("the user docs say panel, never island", () => {
  it("covers the card's list: the README, install, settings, memory, agent-tools and the features", () => {
    expect(FEATURES).toEqual(expect.arrayContaining(["docs/features/chief.md", "docs/features/editor-and-terminal.md"]));
    expect(DOCS.length).toBeGreaterThanOrEqual(5 + 9);
  });
  it.each(DOCS)("%s has no island or islands", (path) => {
    expect(islandLines(path)).toEqual([]);
  });

  it("docs/settings.md: the Appearance row and Background → Glass say the panels", () => {
    const md = read("docs/settings.md");
    expect(md).toContain("| Appearance | The font, text sizes, theme and density, and the glass behind the panels |");
    const glass = md.slice(md.indexOf("- **Background → Glass:**"), md.indexOf("\n", md.indexOf("- **Background → Glass:**")));
    expect(glass).toContain("The background behind the panels is a soft gradient;");
    expect(glass).toContain("The panels, menus and drawers stay solid.");
  });

  it("docs/features/chief.md calls the column beside the chat the side panel", () => {
    const md = read("docs/features/chief.md");
    for (const s of ["starts a terminal in a folder you pick, in the side panel on the right of the conversation",
      "opens the card in a Card tab in the side panel,", "once you close them or hide the side panel,",
      "**Own page** in the side panel's header opens the card's page"]) expect(md).toContain(s);
  });

  it("docs/features/editor-and-terminal.md: the side panel in the Chat, its tabs, Hide, its width, the strip and its keys", () => {
    const md = read("docs/features/editor-and-terminal.md");
    const chat = md.slice(md.indexOf("### In the Chat"), md.indexOf("### In the sidebar and on its own page"));
    for (const s of ["it starts a terminal beside the chat, in the side panel on the right of the conversation,",
      "The side panel's tabs are what you opened in this chat", "**Hide** hides the side panel.",
      "Drag the gap left of the side panel to change its width", "whether the side panel shows or not",
      "a click on the selected one hides the side panel;", "In the side panel, Ctrl+PgDn and Ctrl+PgUp switch tabs.",
      "in the chat's side panel:"]) expect(chat).toContain(s);
  });

  it("docs/features/editor-and-terminal.md says the conversation stays centred while the side panel shows (GA-302), not that it moves left", () => {
    const md = read("docs/features/editor-and-terminal.md");
    expect(md).toContain("While the side panel shows, the conversation stays centred, with the text box and the Archive, as it is without the side panel.");
    expect(md).not.toMatch(/moves to the left/);
    // what the app does: app.css centres the conversation with the side panel as without it (GA-302)
    expect(read("src/styles/app.css")).toContain("The conversation, the text box and the Archive stay centred in their column,");
  });
});

// The app's sources: every .ts and .tsx under src/, but tests and the headless UI test's probe (selftest*.ts)
function sources(dir = "src", out = []) {
  for (const name of readdirSync(join(ROOT, dir))) {
    const path = `${dir}/${name}`;
    if (statSync(join(ROOT, path)).isDirectory()) sources(path, out);
    else if (/\.tsx?$/.test(name) && !/\.test\.tsx?$/.test(name) && !/^selftest/.test(name)) out.push(path);
  }
  return out;
}
// A text people read: a label, title, tooltip, hint, placeholder, a form section's text or an error, given as a string
const TEXT_PROP = /\b(aria-label|aria-description|title|label|text|hint|placeholder|alt|tooltip|tip|message|error|confirm|lead|empty)\s*[=:]\s*\{?\s*(["'`])((?:(?!\2).)*)\2/g;
// The words between two tags in JSX: no {…} expression or comment, and no code (quotes, = or ;, as in querySelector<T>(":scope > .x"))
const JSX_TEXT = />([^<>{}"`=;]*[A-Za-z][^<>{}"`=;]*)</g;

describe("the app's texts say panel, never island", () => {
  const files = sources();
  it("reads the app's sources (AppearanceSettings.tsx and ChatSide.tsx among them)", () => {
    expect(files).toEqual(expect.arrayContaining(["src/components/AppearanceSettings.tsx", "src/components/chat/ChatSide.tsx"]));
  });
  it("no label, title, tooltip, hint or other text given as a string says island", () => {
    const hits = files.flatMap((path) => [...read(path).matchAll(TEXT_PROP)].filter((m) => ISLAND.test(m[3])).map((m) => `${path}: ${m[0]}`));
    expect(hits).toEqual([]);
  });
  it("no words in the markup say island", () => {
    const hits = files.filter((p) => p.endsWith(".tsx"))
      .flatMap((path) => [...read(path).matchAll(JSX_TEXT)].filter((m) => ISLAND.test(m[1])).map((m) => `${path}: ${m[1].trim()}`));
    expect(hits).toEqual([]);
  });
  it("Settings → Appearance's Background line and the side panel's drag handle use the new words", () => {
    expect(read("src/components/AppearanceSettings.tsx"))
      .toContain('<FormSection title="Background" text="Behind the panels: a soft gradient, and glass where your system blurs windows.">');
    expect(read("src/components/chat/ChatSide.tsx")).toContain('side: "left", label: "Side panel\'s width",');
  });
  it("the names in the code stay: the classes and the side panel's state still say island (GA-305 changes words only)", () => {
    expect(read("src/App.tsx")).toContain('<div className="page-island">');
    expect(read("src/lib/chatSide.ts")).toContain('export function useSideShows(): "island" | "strip" | null {');
    expect(read("src/styles/components.css")).toMatch(/\.island-head\b/);
    expect(read("src/styles/terminal.css")).toMatch(/\.chat-side\b/);
  });
});
