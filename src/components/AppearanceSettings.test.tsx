// GA-42: Settings → Appearance. Five font choices, each shown in its own font (data-font on the choice, which appearance.css
// turns into that font's stacks); a picker per text size with the card's sizes; Theme and Density; and Reset to defaults.
// GA-143's Background section (the mountains along the bottom) went with the islands (GA-290): the gradient took their place.
// GA-292 puts the glass in its place: Background → Glass, the checkbox 'Show the desktop through the background', with the
// hints of design/islands-settings/README.md in its three states (the system's side, lib/glass.ts's useGlass, is a fake here).
// A click changes the appearance at once (src/lib/appearance.ts, tested on its own); here it is checked that each control
// changes the right value. Rendered to HTML on the server, and clicked on the element tree with React's hooks replaced.
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ReactElement, ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";

let direct = false;
vi.mock("react", async (orig) => {
  const R = await orig<typeof import("react")>();
  const useState = ((init: unknown) => (direct ? [typeof init === "function" ? (init as () => unknown)() : init, () => {}] : R.useState(init))) as typeof R.useState;
  const useEffect = ((f: () => void, deps?: unknown[]) => (direct ? undefined : R.useEffect(f, deps))) as typeof R.useEffect;
  return { ...R, default: { ...R, useState, useEffect }, useState, useEffect };
});

const sys = vi.hoisted(() => ({ glass: { support: "compositor", reduced: false } as { support: "vibrancy" | "mica" | "compositor" | "none"; reduced: boolean } }));
vi.mock("../lib/glass", async (orig) => ({ ...(await orig<typeof import("../lib/glass")>()), useGlass: () => sys.glass }));

const { AppearanceSettings } = await import("./AppearanceSettings");
const { DEFAULTS, getAppearance, resetAppearance, setAppearance } = await import("../lib/appearance");

const text = (html: string) => html.replace(/<[^>]+>/g, " ").replace(/&#x27;/g, "'").replace(/\s+/g, " ").trim();
// node:fs through a computed specifier, as in lib/islands.test.ts (Vitest hands .css back empty)
const fs = await import(/* @vite-ignore */ ["node", "fs"].join(":"));
const read = (path: string): string => fs.readFileSync(new URL(`../../${path}`, import.meta.url), "utf8");
const render = () => renderToStaticMarkup(<AppearanceSettings />);
function expand(node: ReactNode): ReactNode {
  if (Array.isArray(node)) return node.map(expand);
  if (node && typeof node === "object" && "props" in node) {
    const el = node as ReactElement<Record<string, unknown>>;
    if (typeof el.type === "function") return expand((el.type as (p: unknown) => ReactNode)(el.props));
    return { ...el, props: { ...el.props, children: expand(el.props.children as ReactNode) } } as ReactNode;
  }
  return node;
}
function findAll(node: ReactNode, pred: (p: Record<string, unknown>) => boolean, out: Record<string, unknown>[] = []) {
  if (Array.isArray(node)) node.forEach((n) => findAll(n, pred, out));
  else if (node && typeof node === "object" && "props" in node) {
    const p = (node as ReactElement<Record<string, unknown>>).props;
    if (pred(p)) out.push(p);
    findAll(p.children as ReactNode, pred, out);
  }
  return out;
}
function tree() {
  direct = true;
  try { return expand(AppearanceSettings()); } finally { direct = false; }
}
/** The buttons of a group (by its aria-label), as [label, pressed]. */
function group(html: string, label: string): [string, boolean][] {
  const at = html.indexOf(`aria-label="${label}"`);
  const body = html.slice(at, html.indexOf("</div>", at));
  return [...body.matchAll(/<button aria-pressed="(true|false)"[^>]*>(.*?)<\/button>/g)].map((m) => [text(m[2] ?? ""), m[1] === "true"]);
}
/** Clicks the button labelled `label` in the group `groupLabel`. */
function click(groupLabel: string, label: string) {
  const t = tree();
  const g = findAll(t, (p) => p["aria-label"] === groupLabel)[0];
  const b = findAll(g?.children as ReactNode, (p) => typeof p.onClick === "function" && text(renderToStaticMarkup(<>{p.children as ReactNode}</>)) === label)[0] as { onClick: () => void } | undefined;
  if (!b) throw new Error(`no ${label} in ${groupLabel}`);
  b.onClick();
}

beforeEach(() => { resetAppearance(); sys.glass = { support: "compositor", reduced: false }; });

describe("Settings → Appearance", () => {
  it("says changes show at once, without Save settings", () => {
    expect(text(render())).toContain("Changes show at once, without Save settings, and are kept on this computer.");
  });
  it("offers the five fonts, each a choice shown in its own font, the default picked", () => {
    const html = render();
    expect(html).toContain('role="radiogroup" aria-label="Font"');
    const choices = [...html.matchAll(/<label class="font-choice( on)?" data-font="([^"]+)"><input type="radio" name="gizai-font"( checked="")?\/><span class="fc-name">([^<]+)<\/span><span class="fc-sample">[^<]*<span class="mono">NWS-41<\/span>/g)];
    expect(choices.map((m) => [m[2], m[4], !!m[1], !!m[3]])).toEqual([
      ["atkinson", "Atkinson Hyperlegible", true, true],
      ["jetbrains-mono", "JetBrains Mono", false, false],
      ["inter", "Inter", false, false],
      ["geist", "Geist", false, false],
      ["hack", "Hack", false, false],
    ]);
    // Inter says it is like the apps the card names
    expect(text(html)).toContain("Used by many apps, like Figma and Linear");
  });
  it.each(["dark", "light"] as const)("its font and docs samples use the tour's made-up names, in %s (GA-240)", (theme) => {
    setAppearance({ theme });
    const html = render();
    expect(html.match(/<span class="fc-sample">Export invoices as CSV <span class="mono">NWS-41<\/span><\/span>/g)).toHaveLength(5);
    expect(text(html)).toContain("The portal replaces the Excel lists Northwind's office keeps today.");
    expect(html).not.toContain("KADE-41");
    expect(text(html)).not.toContain("Kade's");
  });
  it("has a picker per text size with the card's sizes, today's size pressed", () => {
    const html = render();
    expect(group(html, "Chat size")).toEqual([14, 15, 16, 17, 18, 20].map((n) => [String(n), n === 15]));
    expect(group(html, "Interface size")).toEqual([12.5, 13.5, 14.5, 15.5, 16.5].map((n) => [String(n), n === 13.5]));
    expect(group(html, "Tasks and docs size")).toEqual([14, 15, 16, 17, 18, 20].map((n) => [String(n), n === 15]));
    expect(html).toContain('title="15 px, the default"');
    expect(html).toContain('title="13.5 px, the default"');
  });
  it("has Theme (Dark, Light) and Density (Comfortable, Compact)", () => {
    const html = render();
    expect(group(html, "Theme")).toEqual([["Dark", true], ["Light", false]]);
    expect(group(html, "Density")).toEqual([["Comfortable", true], ["Compact", false]]);
  });
  it("shows what is set now", () => {
    setAppearance({ font: "geist", chat: 20, ui: 16.5, docs: 18, theme: "light", density: "compact" });
    const html = render();
    expect(html).toContain('<label class="font-choice on" data-font="geist"><input type="radio" name="gizai-font" checked=""/>');
    expect(group(html, "Chat size").filter(([, on]) => on)).toEqual([["20", true]]);
    expect(group(html, "Interface size").filter(([, on]) => on)).toEqual([["16.5", true]]);
    expect(group(html, "Tasks and docs size").filter(([, on]) => on)).toEqual([["18", true]]);
    expect(group(html, "Theme")).toEqual([["Dark", false], ["Light", true]]);
    expect(group(html, "Density")).toEqual([["Comfortable", false], ["Compact", true]]);
  });
  it("ends with Background → Glass (GA-292): the mountains' checkbox is gone (GA-290), the glass has its place", () => {
    const html = render();
    const sections = [...html.matchAll(/<h3[^>]*>([^<]+)<\/h3>/g)].map((m) => m[1]);
    // (the docs sample's own h3, Acceptance criteria, sits in Text size)
    expect(sections.filter((h) => h !== "Acceptance criteria")).toEqual(["Font", "Text size", "Theme and density", "Background"]);
    expect(html.match(/type="checkbox"/g)).toHaveLength(1);
    const background = html.slice(html.indexOf("<h3>Background</h3>"));
    expect(background).toContain("<p>Behind the islands: a soft gradient, and glass where your system blurs windows.</p>");
    expect(background).toMatch(/<div class="field wide"><label>Glass<\/label><label class="check glass-check"><input type="checkbox"\/>Show the desktop through the background<\/label><span class="hint">Blurred and tinted, on macOS, Windows 11 and Hyprland\. Off gives a plain background\.<\/span><\/div>/);
    const t = text(html);
    for (const gone of ["Mountains", "Show mountains along the bottom", "A landscape of wireframe mountains along the bottom of the window. It stays behind text and controls.",
      "Under the sidebar and the page, in the theme's colours. Off gives the plain background back."]) expect(t).not.toContain(gone);
    expect(findAll(tree(), (p) => p.type === "checkbox")).toHaveLength(1);
  });
  it("the glass's checkbox in its three states (Figma 318:1588): what it does, no blur, reduced transparency", () => {
    const glass = () => { const html = render(); return html.slice(html.indexOf("<label>Glass</label>")); };
    const box = (html: string) => /<input type="checkbox"[^>]*\/>/.exec(html)?.[0];
    const hint = (html: string) => /<span class="hint">([^<]*)<\/span>/.exec(html)?.[1]?.replace(/&#x27;/g, "'");
    // the system blurs (macOS, Windows 11, Hyprland): it shows what is set, and says what it does
    for (const support of ["vibrancy", "mica", "compositor"] as const) {
      sys.glass = { support, reduced: false };
      setAppearance({ glass: true });
      expect([support, box(glass()), hint(glass())]).toEqual([support, '<input type="checkbox" checked=""/>', "Blurred and tinted, on macOS, Windows 11 and Hyprland. Off gives a plain background."]);
      setAppearance({ glass: false });
      expect([support, box(glass())]).toEqual([support, '<input type="checkbox"/>']);
    }
    // no blur (Windows 10, Linux other than Hyprland): off and disabled, its words faint, even with the switch kept on
    sys.glass = { support: "none", reduced: false };
    setAppearance({ glass: true });
    expect(box(glass())).toBe('<input type="checkbox" disabled=""/>');
    expect(hint(glass())).toBe("Your system doesn't blur windows behind Hersir, so the background stays plain.");
    expect(read("src/styles/appearance.css")).toMatch(/\.glass-check:has\(input:disabled\) \{ color: var\(--text-3\);/);
    // reduced transparency: it keeps its value, and says why the background is plain
    sys.glass = { support: "vibrancy", reduced: true };
    expect(box(glass())).toBe('<input type="checkbox" checked=""/>');
    expect(hint(glass())).toBe("Plain while your system reduces transparency.");
    setAppearance({ glass: false });
    expect(box(glass())).toBe('<input type="checkbox"/>');
  });
  it("Reset to defaults is off at the defaults and on as soon as anything differs", () => {
    expect(render()).toMatch(/<button class="link" disabled="" title="Everything is at its default">Reset to defaults<\/button>/);
    for (const change of [{ chat: 16 }, { font: "hack" as const }, { theme: "light" as const }, { density: "compact" as const }, { ui: 12.5 }, { docs: 20 }, { glass: !DEFAULTS.glass }]) {
      resetAppearance();
      setAppearance(change);
      expect(render()).toContain('<button class="link">Reset to defaults</button>');
    }
  });
});

describe("a click", () => {
  it("on a size sets that part's size, and only that part", () => {
    click("Chat size", "20");
    expect(getAppearance()).toEqual({ ...DEFAULTS, chat: 20 });
    click("Interface size", "16.5");
    click("Tasks and docs size", "14");
    expect(getAppearance()).toEqual({ ...DEFAULTS, chat: 20, ui: 16.5, docs: 14 });
  });
  it("on a font picks it", () => {
    const radios = findAll(tree(), (p) => p.type === "radio") as unknown as { onChange: () => void }[];
    expect(radios.length).toBe(5);
    radios[2]?.onChange();
    expect(getAppearance().font).toBe("inter");
    radios[4]?.onChange();
    expect(getAppearance().font).toBe("hack");
  });
  it("on Light, Compact and back", () => {
    click("Theme", "Light");
    click("Density", "Compact");
    expect(getAppearance()).toEqual({ ...DEFAULTS, theme: "light", density: "compact" });
    click("Theme", "Dark");
    click("Density", "Comfortable");
    expect(getAppearance()).toEqual(DEFAULTS);
  });
  it("on the glass's checkbox turns it on and off at once (GA-292)", () => {
    const box = () => findAll(tree(), (p) => p.type === "checkbox")[0] as { onChange: (e: unknown) => void; checked: boolean; disabled: boolean };
    expect(box().checked).toBe(false);
    box().onChange({ target: { checked: true } });
    expect(getAppearance()).toEqual({ ...DEFAULTS, glass: true });
    expect(box().checked).toBe(true);
    box().onChange({ target: { checked: false } });
    expect(getAppearance()).toEqual({ ...DEFAULTS, glass: false });
    sys.glass = { support: "none", reduced: false };
    expect(box().disabled).toBe(true);
  });
  it("on Reset to defaults brings everything back", () => {
    setAppearance({ font: "geist", chat: 20, ui: 16.5, docs: 18, theme: "light", density: "compact", glass: !DEFAULTS.glass });
    const reset = findAll(tree(), (p) => p.className === "link")[0] as { onClick: () => void; children: unknown };
    expect(reset.children).toBe("Reset to defaults");
    reset.onClick();
    expect(getAppearance()).toEqual(DEFAULTS);
  });
});
