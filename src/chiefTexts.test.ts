// GA-254 QA: no text a person reads in the app says Team Lead for the lead agent, as `grep -rn "Team Lead" src` checks by
// hand: every string, template, JSX text and regular expression in src's code (not its tests, and not the UI self-test
// src/selftest.ts) is read with TypeScript's own parser, so comments are left out. What may still say it, by the card's
// rule: the line that still shows an older chat's "The Team Lead couldn't …" as a failure. The Memory folder, which kept
// the name until GA-258, is Chief/ now (LEAD in lib/memory.ts, and the paths in the browser mode's fixtures).
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
  /^components\/chat\/ChatMessage\.tsx:\d+: \/\^The \(Chief\|Team Lead\) couldn't\/$/, // an older chat's failure note
];

describe("the app's texts (GA-254)", () => {
  const all = files.flatMap(teamLeads);

  it("are read from the whole app, the older chat's failure note among them", () => {
    expect(files.length).toBeGreaterThan(100);
    expect(files).toContain("components/Sidebar.tsx");
    expect(all.some((f) => ALLOWED[0].test(f))).toBe(true);
  });

  it("say Team Lead only for an older chat's failure note", () => {
    expect(all.filter((f) => !ALLOWED.some((a) => a.test(f)))).toEqual([]);
  });

  it("name the Memory folder Chief/ (GA-258)", () => {
    expect(LEAD).toBe("Chief");
  });
});

describe("the browser mode's fixtures (GA-254)", () => {
  const json = read("browser/fixtures.json");

  it("say Team Lead nowhere, and Chief/ for the Memory folder (GA-258)", () => {
    expect([...json.matchAll(/.{0,40}Team Lead.{0,12}/g)].map((m) => m[0])).toEqual([]);
    expect(json).toContain("memory_append to Chief/Notes");
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
