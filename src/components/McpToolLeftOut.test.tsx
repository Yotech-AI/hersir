// GA-262 QA: an MCP tool whose name isn't a plain tool name (like `x Bash`) is never allowed. Settings → MCP servers and
// the agent form show it as left out, and its switch stays off. Rendered to HTML on the server.
import { describe, expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { McpTool } from "./McpTools";
import type { McpToolView } from "../types";

const LEFT_OUT = "left out: its name isn't a plain tool name";
const tool = (name: string, leftOut?: string | null): McpToolView => ({
  name, description: "", params: [], hints: { readOnly: false, destructive: false, idempotent: false, openWorld: false }, hintsSent: [],
  risk: "medium", summary: `${name} changes things`, notes: [], leftOut,
});
const esc = (s: string) => s.replace(/'/g, "&#x27;");

describe("an MCP tool runs leave out (GA-262)", () => {
  it("says it is left out, in Settings (no switch)", () => {
    const html = renderToStaticMarkup(<McpTool t={tool("x Bash", LEFT_OUT)} />);
    expect(html).toContain(esc(`<span class="mt-summary warn">${LEFT_OUT}</span>`));
    expect(html).toContain('<span class="mono">x Bash</span>');
  });

  it("shows its switch off and disabled in the agent form, even with the server's tools all on", () => {
    const html = renderToStaticMarkup(<McpTool t={tool("x Bash", LEFT_OUT)} on onSwitch={() => {}} />);
    expect(html).toMatch(/<input type="checkbox" class="mt-switch" aria-label="Use x Bash" disabled=""\/>/);
    expect(html).toContain(esc(LEFT_OUT));
  });

  it("says nothing more about a tool with a plain name, whose switch works as before", () => {
    for (const leftOut of [null, undefined]) {
      const html = renderToStaticMarkup(<McpTool t={tool("search", leftOut)} on onSwitch={() => {}} />);
      expect(html).not.toContain("left out");
      expect(html).toMatch(/<input type="checkbox" class="mt-switch" aria-label="Use search" checked=""\/>/);
    }
  });
});
