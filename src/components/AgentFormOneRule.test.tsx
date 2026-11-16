// GA-262 QA: an allowed command that isn't one rule, like `Bash(ls:*) Bash(*)` (Claude Code would read it as two rules, the
// second any command). The agent form shows each such entry, also one saved before Hersir checked this, with the sentence
// saving refuses it with. Rendered to HTML on the server; the form's draft is a saved agent's list (draftFrom mocked).
import { describe, expect, it, vi } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import type { Member } from "../types";

const saved = vi.hoisted(() => ({ tools: [] as string[] }));
vi.mock("../lib/agents", async (orig) => {
  const m = await orig<typeof import("../lib/agents")>();
  return { ...m, draftFrom: (...a: Parameters<typeof m.draftFrom>) => ({ ...m.draftFrom(...a), tools: saved.tools.join("\n") }) };
});

const { AgentDrawer } = await import("./AgentForm");
const real = await vi.importActual<typeof import("../lib/agents")>("../lib/agents");

const SENTENCE = "isn't one rule: write each command as its own entry, like Bash(npm test:*).";
const html = (s: string) => s.replace(/'/g, "&#x27;");
const allowedField = (page: string) => {
  const at = page.indexOf('<label for="a-tools">');
  const next = page.indexOf('<div class="field', at);
  return page.slice(at, next < 0 ? undefined : next);
};

describe("Allowed commands that aren't one rule in the agent form (GA-262)", () => {
  it("shows each one with the sentence, and keeps them in the list to fix", () => {
    saved.tools = ["Bash(git status:*)", "Bash(ls:*) Bash(*)", "Bash(npm test:*)", "Bash(a,b)"];
    const page = renderToStaticMarkup(<AgentDrawer teamId="t1" preset={{ role: "backend" }} onClose={() => {}} />);
    // where the chat's refusal sends the user: Permissions → Allowed commands
    const at = page.indexOf('<label for="a-tools">Allowed commands</label>');
    expect(page.lastIndexOf("<h3>", at)).toBe(page.indexOf("<h3>Permissions</h3>"));
    const field = allowedField(page);
    expect(field).toContain(html(`<span class="warn">Bash(ls:*) Bash(*) ${SENTENCE} Bash(a,b) ${SENTENCE}</span>`));
    expect(field).not.toContain(`Bash(git status:*) ${html(SENTENCE)}`);
    expect(field).toContain("Bash(git status:*)\nBash(ls:*) Bash(*)\nBash(npm test:*)\nBash(a,b)</textarea>");
  });

  it("says nothing more when each entry is one rule", () => {
    saved.tools = ["Bash(git status:*)", "Bash(npm test:*)", "Bash(./vendor/bin/*)"];
    const field = allowedField(renderToStaticMarkup(<AgentDrawer teamId="t1" preset={{ role: "backend" }} onClose={() => {}} />));
    expect(field).not.toContain('class="warn"');
    expect(field).not.toContain(html(SENTENCE));
    expect(field).toContain("Everything else is refused in acceptEdits mode.");
  });
});

describe("oneRule and notOneRule (src/lib/agents.ts), as team.rs's one_rule", () => {
  it("takes a tool name with at most one ( … ) holding no (, ) or comma, and nothing after it", () => {
    for (const ok of ["Bash(git status:*)", "Bash(npm test:*)", "Bash(./vendor/bin/*)", "Read", "mcp__otus__get_card", "Read(//srv/docs/**)",
      " Bash(npm test:*) "]) {
      expect(real.oneRule(ok), ok).toBe(true);
    }
    for (const bad of ["Bash(ls:*) Bash(*)", "Bash(a) b", "Bash(a,b)", "Bash(a(b))", "Bash(ls:*),Bash(*)", "Read Bash", "Bash(a)b", "Bash(a",
      "(a)", "Bash a", "Bäsh(a)"]) {
      expect(real.oneRule(bad), bad).toBe(false);
    }
    expect(real.notOneRule(" Bash(a) b ")).toBe(`Bash(a) b ${SENTENCE}`);
  });

  it("finds every entry of Hersir's default list one rule, and loads a saved agent's old entry so the form can show it", () => {
    expect(real.DEFAULT_TOOLS.filter((t) => !real.oneRule(t))).toEqual([]);
    const m = { actorId: "a1", name: "Backend Agent", roleKey: "backend", kind: "agent", isLead: false,
      allowedTools: ["Bash(git status:*)", "Bash(ls:*) Bash(*)"] } as unknown as Member;
    expect(real.parseTools(real.draftFrom(m).tools).filter((t) => !real.oneRule(t))).toEqual(["Bash(ls:*) Bash(*)"]);
  });
});
