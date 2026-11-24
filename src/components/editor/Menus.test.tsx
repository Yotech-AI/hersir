// GA-348 QA: a menu item can be one of a choice (the Terminal page's Layout): with `checked` it is a menuitemradio with
// aria-checked, and the current one has a 14px check on the right; without it, it stays a plain menuitem. The menu is
// rendered to markup in Node; its portal renders in place.
import { describe, expect, it, vi } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";

vi.mock("react-dom", async (orig) => ({ ...(await orig<object>()), createPortal: (node: unknown) => node }));
vi.stubGlobal("document", { body: {} });

const { ContextMenu } = await import("./Menus");

const html = (items: Parameters<typeof ContextMenu>[0]["items"]) =>
  renderToStaticMarkup(<ContextMenu at={{ x: 10, y: 20 }} label="Layout" items={items} onClose={() => {}} />);
const buttons = (markup: string) => [...markup.matchAll(/<button[^>]*>.*?<\/button>/g)].map((m) => m[0]);

describe("ContextMenu: one of a choice", () => {
  it("checked items are menuitemradio with aria-checked; only the checked one has the check", () => {
    const out = html([
      { label: "One terminal", checked: true, run: () => {} },
      { label: "2 side by side", checked: false, run: () => {} },
      { label: "3 side by side", checked: false, disabled: true, title: "Why it is off", run: () => {} },
    ]);
    expect(out).toContain('role="menu" aria-label="Layout"');
    const [one, two, three] = buttons(out);
    expect(one).toMatch(/role="menuitemradio"/);
    expect(one).toMatch(/aria-checked="true"/);
    expect(one).toMatch(/<svg[^>]*class="[^"]*icon check[^"]*"[^>]*aria-hidden="true"/);
    expect(two).toMatch(/role="menuitemradio"/);
    expect(two).toMatch(/aria-checked="false"/);
    expect(two).not.toMatch(/icon check/);
    expect(three).toMatch(/disabled=""/);
    expect(three).toMatch(/title="Why it is off"/);
    // the check comes after the label, on the right
    expect(one.indexOf("One terminal")).toBeLessThan(one.indexOf("icon check"));
  });
  it("items without checked stay plain menuitems, with no aria-checked and no check", () => {
    const [b] = buttons(html([{ label: "Rename", kbd: "F2", run: () => {} }]));
    expect(b).toMatch(/role="menuitem"/);
    expect(b).not.toMatch(/aria-checked/);
    expect(b).not.toMatch(/icon check/);
  });
});
