// GA-254 QA: no text a person reads in the app says Team Lead for the lead agent, as `grep -rn "Team Lead" src` checks by
// hand: every string, template, JSX text and regular expression in src's code (not its tests, and not the UI self-test
// src/selftest.ts) is read with TypeScript's own parser, so comments are left out. What may still say it, by the card's
// rule: the Memory folder's name (LEAD in lib/memory.ts, and "Team Lead/" paths in the browser mode's fixtures) and the
// line that still shows an older chat's "The Team Lead couldn't …" as a failure.
import { describe, expect, it } from "vitest";
import ts from "typescript";
import { LEAD } from "./lib/memory";

const fs = await import(/* @vite-ignore */ ["node", "fs"].join(":"));
const SRC = new URL("./", import.meta.url);
const read = (file: string): string => fs.readFileSync(new URL(file, SRC), "utf8");

const files = (fs.readdirSync(SRC, { recursive: true }) as string[]).map((f) => f.split("\\").join("/"))
  .filter((f) => /\.tsx?$/.test(f) && !/\.test\.tsx?$/.test(f) && !f.endsWith(".d.ts") && f !== "selftest.ts");

/** Each piece of text in a file's code that says Team Lead, as "file:line: text". */
function teamLeads(file: string): string[] {
  const src = ts.createSourceFile(file, read(file), ts.ScriptTarget.Latest, true, file.endsWith(".tsx") ? ts.ScriptKind.TSX : ts.ScriptKind.TS);
  const found: string[] = [];
  const visit = (n: ts.Node) => {
    if ((ts.isStringLiteralLike(n) || ts.isTemplateLiteralToken(n) || ts.isJsxText(n) || ts.isRegularExpressionLiteral(n)) && n.getText(src).includes("Team Lead"))
      found.push(`${file}:${src.getLineAndCharacterOfPosition(n.getStart(src)).line + 1}: ${n.getText(src).trim()}`);
    ts.forEachChild(n, visit);
  };
  visit(src);
  return found;
}

const ALLOWED = [
  /^lib\/memory\.ts:\d+: "Team Lead"$/, // LEAD, the Memory folder
  /^components\/chat\/ChatMessage\.tsx:\d+: \/\^The \(Chief\|Team Lead\) couldn't\/$/, // an older chat's failure note
];

describe("the app's texts (GA-254)", () => {
  const all = files.flatMap(teamLeads);

  it("are read from the whole app, the Memory folder's name among them", () => {
    expect(files.length).toBeGreaterThan(100);
    expect(files).toContain("components/Sidebar.tsx");
    expect(all.some((f) => ALLOWED[0].test(f))).toBe(true);
  });

  it("say Team Lead only for the Memory folder and an older chat's failure note", () => {
    expect(all.filter((f) => !ALLOWED.some((a) => a.test(f)))).toEqual([]);
  });

  it("keep the Memory folder Team Lead/", () => {
    expect(LEAD).toBe("Team Lead");
  });
});

describe("the browser mode's fixtures (GA-254)", () => {
  const json = read("browser/fixtures.json");

  it("say Team Lead only as the Memory folder's name", () => {
    const hits = [...json.matchAll(/.{0,40}Team Lead.{0,12}/g)].map((m) => m[0]);
    expect(hits.length).toBeGreaterThan(0);
    expect(hits.filter((h) => !h.includes("Team Lead/"))).toEqual([]);
  });

  it("name the lead agent Chief", () => {
    const leads = new Set<string>();
    const walk = (v: unknown): void => {
      if (Array.isArray(v)) v.forEach(walk);
      else if (v && typeof v === "object") {
        const o = v as Record<string, unknown>;
        if (o.roleKey === "lead" && typeof o.name === "string") leads.add(o.name);
        Object.values(o).forEach(walk);
      }
    };
    walk(JSON.parse(json));
    expect([...leads]).toEqual(["Chief"]);
  });
});
