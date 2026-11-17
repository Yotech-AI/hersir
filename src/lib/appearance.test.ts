// GA-42: Settings → Appearance's values (src/lib/appearance.ts). What is kept in localStorage and read back at the next start,
// what goes on <html> (data-font, data-theme, data-density and the size variables appearance.css grows the text by), and that
// the defaults put no size variable on <html> at all, so the app looks as it did. GA-143's mountains along the bottom went with
// the islands (GA-290): no data-band, and the gizai-band an older Hersir kept is taken out at the first read. A fake localStorage
// and <html> stand in for the WebView's; each "start" imports the module afresh, as a restart of Gizai would. GA-292 adds the
// glass behind the islands (glass, kept as gizai-glass only while it differs from the system's default): on by default on
// macOS and Windows, off on Linux, told apart by the user agent. Node's user agent ("Node.js/…") is neither, so here its
// default is off, as on Linux; lib/glass.test.ts has what it does to the window.
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  CHAT_SIZES, DEFAULTS, DOCS_SIZES, FONTS, GONE_KEYS, KEYS, UI_SIZES, appearanceOf, glassByDefault, growth, isDefault, parseAppearance, sizeVars,
  type Appearance,
} from "./appearance";

let store: Map<string, string>;
const storage = {
  getItem: (k: string) => store.get(k) ?? null,
  setItem: (k: string, v: string) => { store.set(k, String(v)); },
  removeItem: (k: string) => { store.delete(k); },
};
function fakeHtml() {
  const props = new Map<string, string>();
  return {
    dataset: {} as Record<string, string | undefined>,
    style: {
      setProperty: (k: string, v: string) => { props.set(k, v); },
      removeProperty: (k: string) => { props.delete(k); },
      getPropertyValue: (k: string) => props.get(k) ?? "",
    },
    props,
  };
}
let html: ReturnType<typeof fakeHtml>;

/** A start of Gizai: a fresh module (nothing read yet) on the same localStorage, and main.tsx's applyAppearance on <html>. */
async function start() {
  vi.resetModules();
  html = fakeHtml();
  vi.stubGlobal("window", { localStorage: storage });
  vi.stubGlobal("document", { documentElement: html });
  const m = await import("./appearance");
  m.applyAppearance(html as unknown as HTMLElement, m.getAppearance());
  return m;
}

beforeEach(() => { store = new Map(); });
afterEach(() => { vi.unstubAllGlobals(); });

describe("the choices", () => {
  it("offers the card's five fonts, Atkinson Hyperlegible first and the default", () => {
    expect(FONTS.map((f) => f.key)).toEqual(["atkinson", "jetbrains-mono", "inter", "geist", "hack"]);
    expect(FONTS.map((f) => f.name)).toEqual(["Atkinson Hyperlegible", "JetBrains Mono", "Inter", "Geist", "Hack"]);
    expect(DEFAULTS.font).toBe("atkinson");
  });
  it("offers the card's sizes, with today's sizes as the defaults", () => {
    expect(CHAT_SIZES).toEqual([14, 15, 16, 17, 18, 20]);
    expect(UI_SIZES).toEqual([12.5, 13.5, 14.5, 15.5, 16.5]);
    expect(DOCS_SIZES).toEqual([14, 15, 16, 17, 18, 20]);
    expect(DEFAULTS).toEqual({ font: "atkinson", chat: 15, ui: 13.5, docs: 15, theme: "dark", density: "comfortable", glass: false });
  });
  it("has the glass on by default on macOS and Windows, and off on Linux (GA-292), read from the user agent each time", () => {
    const at = (userAgent: string) => { vi.stubGlobal("navigator", { userAgent }); return [glassByDefault(), DEFAULTS.glass]; };
    expect(at("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko)")).toEqual([true, true]);
    expect(at("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36 Edg/130.0.0.0")).toEqual([true, true]);
    expect(at("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/605.1.15 (KHTML, like Gecko)")).toEqual([false, false]);
    expect(at("")).toEqual([false, false]);
    vi.stubGlobal("navigator", undefined);
    expect(glassByDefault()).toBe(false);
  });
  it("keeps the theme and density under the t and d keys' own names, from before", () => {
    expect(KEYS.theme).toBe("gizai-theme");
    expect(KEYS.density).toBe("gizai-density");
    expect(Object.values(KEYS)).not.toContain("gizai-band");
    expect(KEYS.glass).toBe("gizai-glass");
    expect(new Set(Object.values(KEYS)).size).toBe(7);
  });
});

describe("reading what was kept", () => {
  const read = (kept: Record<string, string>) => parseAppearance((k) => kept[k] ?? null);
  it("is the defaults when nothing is kept", () => {
    expect(read({})).toEqual(DEFAULTS);
    expect(isDefault(read({}))).toBe(true);
  });
  it("reads each value", () => {
    expect(read({ "gizai-font": "geist", "gizai-size-chat": "20", "gizai-size-ui": "16.5", "gizai-size-docs": "14", "gizai-theme": "light", "gizai-density": "compact", "gizai-glass": "true" }))
      .toEqual({ font: "geist", chat: 20, ui: 16.5, docs: 14, theme: "light", density: "compact", glass: true });
    expect(read({ "gizai-glass": "false" }).glass).toBe(false);
  });
  it("puts anything it doesn't know back at its default, so a bad value can't break the look", () => {
    expect(read({ "gizai-font": "comic-sans", "gizai-size-chat": "19", "gizai-size-ui": "13", "gizai-size-docs": "big", "gizai-theme": "blue", "gizai-density": "tiny", "gizai-glass": "yes" }))
      .toEqual(DEFAULTS);
  });
  it("reads what the t and d keys kept before this card: 'dark' and an empty density are the defaults", () => {
    expect(read({ "gizai-theme": "dark", "gizai-density": "" })).toEqual(DEFAULTS);
    expect(read({ "gizai-theme": "light", "gizai-density": "compact" })).toMatchObject({ theme: "light", density: "compact" });
  });
});

describe("what grows", () => {
  it("grows reading text fully and headings about half", () => {
    expect(growth(20, 15)).toMatchObject({ text: 5, head: 2.5 });
    expect(growth(16.5, 13.5)).toMatchObject({ text: 3, head: 1.5 });
    expect(growth(14, 15)).toMatchObject({ text: -1, head: -0.5 });
  });
  it("keeps small things (IDs, labels, times, badges) the same, or at most 1px bigger at the largest sizes", () => {
    for (const [sizes, base] of [[CHAT_SIZES, 15], [UI_SIZES, 13.5], [DOCS_SIZES, 15]] as const) {
      for (const n of sizes) {
        const { meta } = growth(n, base);
        expect(meta).toBeGreaterThanOrEqual(0);
        expect(meta).toBeLessThanOrEqual(1);
        if (n - base < 2) expect(meta).toBe(0);
      }
    }
  });
});

describe("the size variables on <html>", () => {
  const at = (a: Partial<Appearance>) => sizeVars({ ...DEFAULTS, ...a });
  it("are none at all at the defaults, so the app looks as it did", () => {
    expect(at({})).toEqual({});
    expect(at({ font: "inter", theme: "light" })).toEqual({});
  });
  it("at the largest sizes: the interface 3px, rows 5px, the chat 5px and its column a third wider, tasks and docs 5px", () => {
    expect(at({ chat: 20, ui: 16.5, docs: 20 })).toEqual({
      "--ui-grow": "3px", "--ui-grow-head": "1.5px", "--ui-grow-meta": "1px", "--ui-box": "5px",
      "--chat-grow": "5px", "--chat-grow-head": "2.5px", "--chat-scale": "1.333",
      "--docs-grow": "5px", "--docs-grow-head": "2.5px",
    });
  });
  it("at the smallest sizes: text 1px smaller, the chat column narrower, small things unchanged", () => {
    const v = at({ chat: 14, ui: 12.5, docs: 14 });
    expect(v["--ui-grow"]).toBe("-1px");
    expect(v["--ui-grow-meta"]).toBeUndefined();
    expect(v["--chat-grow"]).toBe("-1px");
    expect(v["--chat-scale"]).toBe("0.933");
    expect(v["--docs-grow"]).toBe("-1px");
  });
  it("sets only the part that changed", () => {
    expect(Object.keys(at({ chat: 18 })).sort()).toEqual(["--chat-grow", "--chat-grow-head", "--chat-scale"]);
    expect(Object.keys(at({ docs: 17 })).sort()).toEqual(["--docs-grow", "--docs-grow-head"]);
    expect(Object.keys(at({ ui: 14.5 })).sort()).toEqual(["--ui-box", "--ui-grow", "--ui-grow-head"]);
  });
  it("makes rows 4px shorter when compact, at any interface size", () => {
    expect(at({ density: "compact" })).toEqual({ "--ui-box": "-4px" });
    expect(at({ density: "compact", ui: 16.5 })["--ui-box"]).toBe("1px");
  });
});

describe("Settings → Appearance on <html>", () => {
  it("puts nothing but the dark theme on <html> at the defaults", async () => {
    await start();
    expect(html.dataset).toEqual({ theme: "dark" });
    expect(html.props.size).toBe(0);
  });
  it("shows a change at once, keeps it, and starts with it next time", async () => {
    const m = await start();
    m.setAppearance({ font: "inter", chat: 20, ui: 16.5, docs: 18, theme: "light", density: "compact", glass: true });
    expect(html.dataset).toEqual({ font: "inter", theme: "light", density: "compact" });
    expect(html.style.getPropertyValue("--chat-grow")).toBe("5px");
    expect(html.style.getPropertyValue("--ui-grow")).toBe("3px");
    expect(html.style.getPropertyValue("--docs-grow")).toBe("3px");
    expect(Object.fromEntries(store)).toEqual({
      "gizai-font": "inter", "gizai-size-chat": "20", "gizai-size-ui": "16.5", "gizai-size-docs": "18", "gizai-theme": "light", "gizai-density": "compact",
      "gizai-glass": "true",
    });
    // a restart
    const again = await start();
    expect(again.getAppearance()).toEqual({ font: "inter", chat: 20, ui: 16.5, docs: 18, theme: "light", density: "compact", glass: true });
    expect(html.dataset).toEqual({ font: "inter", theme: "light", density: "compact" });
    expect(html.style.getPropertyValue("--chat-scale")).toBe("1.333");
  });
  it("changes one thing without touching the others", async () => {
    const m = await start();
    m.setAppearance({ chat: 18 });
    m.setAppearance({ font: "hack" });
    expect(m.getAppearance()).toEqual({ ...DEFAULTS, chat: 18, font: "hack" });
    expect(Object.fromEntries(store)).toEqual({ "gizai-size-chat": "18", "gizai-font": "hack" });
  });
  it("Reset to defaults forgets everything kept and puts the default look back", async () => {
    const m = await start();
    m.setAppearance({ font: "geist", chat: 14, ui: 12.5, docs: 20, theme: "light", density: "compact", glass: true });
    m.resetAppearance();
    expect(m.getAppearance()).toEqual(DEFAULTS);
    expect(store.size).toBe(0);
    expect(html.dataset).toEqual({ theme: "dark" });
    expect(html.props.size).toBe(0);
    await start();
    expect(html.props.size).toBe(0);
  });
  it("the t and d keys switch the theme and density the same way, and are kept", async () => {
    const m = await start();
    m.toggleTheme();
    m.toggleDensity();
    expect(html.dataset).toMatchObject({ theme: "light", density: "compact" });
    expect(Object.fromEntries(store)).toEqual({ "gizai-theme": "light", "gizai-density": "compact" });
    m.toggleTheme();
    m.toggleDensity();
    expect(html.dataset).toEqual({ theme: "dark" });
    expect(store.size).toBe(0);
  });
  it("starts with what the t and d keys kept before this card", async () => {
    store.set("gizai-theme", "light");
    store.set("gizai-density", "compact");
    await start();
    expect(html.dataset).toEqual({ theme: "light", density: "compact" });
  });
  it("the dev hook's appearance (screenshots) shows but isn't kept", async () => {
    const m = await start();
    m.setAppearance(m.appearanceOf("chat=20,ui=16.5,docs=20,font=inter,theme=light"), false);
    expect(html.dataset).toMatchObject({ font: "inter", theme: "light" });
    expect(store.size).toBe(0);
  });
  it("still works where localStorage can't be used: the change shows until a restart", async () => {
    const m = await start();
    vi.stubGlobal("window", { get localStorage(): Storage { throw new Error("private mode"); } });
    m.setAppearance({ chat: 17 });
    expect(html.style.getPropertyValue("--chat-grow")).toBe("2px");
    expect(store.size).toBe(0);
  });
});

describe("the mountains' switch is gone (GA-290: the islands' gradient took their place)", () => {
  const read = (kept: Record<string, string>) => parseAppearance((k) => kept[k] ?? null);
  it("has no band in the appearance, and gizai-band is only a key an older Hersir kept", () => {
    expect(Object.keys(DEFAULTS).sort()).toEqual(["chat", "density", "docs", "font", "glass", "theme", "ui"]);
    expect(read({ "gizai-band": "false" })).toEqual(DEFAULTS);
    expect(GONE_KEYS).toEqual(["gizai-band"]);
  });
  it("takes the gizai-band an older Hersir kept out at the first start, and leaves the rest as it was", async () => {
    store.set("gizai-band", "false");
    store.set("gizai-theme", "light");
    const m = await start();
    expect(store.has("gizai-band")).toBe(false);
    expect(Object.fromEntries(store)).toEqual({ "gizai-theme": "light" });
    expect(m.getAppearance()).toEqual({ ...DEFAULTS, theme: "light" });
    expect(html.dataset).toEqual({ theme: "light" });
    expect("band" in html.dataset).toBe(false);
  });
  it("puts no data-band on <html> after any change, a reset or the dev hook's band=false", async () => {
    const m = await start();
    m.setAppearance({ theme: "light", density: "compact" });
    expect("band" in html.dataset).toBe(false);
    m.resetAppearance();
    expect(html.dataset).toEqual({ theme: "dark" });
    m.setAppearance(m.appearanceOf("band=false"), false);
    expect(html.dataset).toEqual({ theme: "dark" });
    expect(appearanceOf("band=false")).toEqual(DEFAULTS);
    expect(store.size).toBe(0);
  });
  it("still starts where localStorage can't be used", async () => {
    vi.resetModules();
    html = fakeHtml();
    vi.stubGlobal("window", { get localStorage(): Storage { throw new Error("private mode"); } });
    vi.stubGlobal("document", { documentElement: html });
    const m = await import("./appearance");
    expect(m.getAppearance()).toEqual(DEFAULTS);
  });
});

describe("appearanceOf (the dev hook)", () => {
  it("reads the given parts and leaves the rest at the default", () => {
    expect(appearanceOf("chat=20,ui=16.5,docs=20,font=inter,theme=light"))
      .toEqual({ font: "inter", chat: 20, ui: 16.5, docs: 20, theme: "light", density: "comfortable", glass: DEFAULTS.glass });
    expect(appearanceOf("glass=true").glass).toBe(true);
    expect(appearanceOf("glass=false").glass).toBe(false);
    expect(appearanceOf("")).toEqual(DEFAULTS);
    expect(appearanceOf("chat=99,nope=1")).toEqual(DEFAULTS);
  });
});
