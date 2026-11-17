// GA-292 QA: the glass behind the islands (src/lib/glass.ts; design/islands/README.md → The glass, and
// design/islands-settings/README.md → Appearance: the glass switch). html.glass is on while Settings → Appearance's switch is
// on, the system gives a blur and it doesn't ask for reduced transparency; the window is asked for vibrancy on macOS and Mica
// (dark or light with the theme) on Windows 11, and its effects are cleared when the glass is off; on Hyprland the compositor
// blurs by its own rules, so the window is never asked. Each change shows at once, and the switch is kept on this computer.
// The api (glass_support, setEffects) is a fake; a fake <html>, localStorage, matchMedia and user agent stand in for the
// WebView's, and each "start" imports the modules afresh, as a restart of Hersir would.
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Effect, EffectState } from "@tauri-apps/api/window";
import { effectsFor, glassHint, glassOn, guessSupport, type Glass } from "./glass";

const h = vi.hoisted(() => ({
  /** What glass_support answers, or an Error when it can't be asked (the browser mode). */
  support: "none" as string | Error,
  /** Each setWindowEffects call: the effects asked for, or null to clear them. */
  asked: [] as unknown[],
  /** setWindowEffects is refused. */
  refuse: false,
}));
vi.mock("../api", () => ({
  glassSupport: () => (h.support instanceof Error ? Promise.reject(h.support) : Promise.resolve(h.support)),
  setWindowEffects: (e: unknown) => { h.asked.push(e); return h.refuse ? Promise.reject(new Error("not allowed")) : Promise.resolve(); },
}));

// The user agents of the three web views: WKWebView, WebView2 and WebKitGTK.
const MAC = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko)";
const WINDOWS = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36 Edg/130.0.0.0";
const LINUX = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/605.1.15 (KHTML, like Gecko)";
const VIBRANCY = { effects: [Effect.UnderWindowBackground], state: EffectState.FollowsWindowActiveState };
const HINT_ON = "Blurred and tinted, on macOS, Windows 11 and Hyprland. Off gives a plain background.";
const HINT_NONE = "Your system doesn't blur windows behind Hersir, so the background stays plain.";
const HINT_REDUCED = "Plain while your system reduces transparency.";

let store: Map<string, string>;
const storage = {
  getItem: (k: string) => store.get(k) ?? null,
  setItem: (k: string, v: string) => { store.set(k, String(v)); },
  removeItem: (k: string) => { store.delete(k); },
};
/** prefers-reduced-transparency: reduce, and who listens for its change. */
let reduced = false;
let onReduced: ((e: { matches: boolean }) => void)[] = [];
const media = (q: string) => ({
  media: q, matches: q === "(prefers-reduced-transparency: reduce)" && reduced,
  addEventListener: (_t: string, f: (e: { matches: boolean }) => void) => { onReduced.push(f); },
});
function fakeHtml() {
  const classes = new Set<string>();
  const props = new Map<string, string>();
  return {
    classes,
    classList: { toggle: (c: string, on?: boolean) => { if (on ?? !classes.has(c)) classes.add(c); else classes.delete(c); return classes.has(c); }, contains: (c: string) => classes.has(c) },
    dataset: {} as Record<string, string | undefined>,
    style: { setProperty: (k: string, v: string) => { props.set(k, v); }, removeProperty: (k: string) => { props.delete(k); }, getPropertyValue: (k: string) => props.get(k) ?? "" },
  };
}
let html: ReturnType<typeof fakeHtml>;
const settle = () => new Promise((r) => setTimeout(r, 0));
const glassy = () => html.classes.has("glass");

/** A start of Hersir on a system: main.tsx's applyAppearance, then startGlass, and the system's answer about its blur. */
async function start(ua: string, support: string | Error) {
  vi.resetModules();
  h.support = support;
  h.asked = [];
  onReduced = [];
  html = fakeHtml();
  vi.stubGlobal("navigator", { userAgent: ua });
  vi.stubGlobal("window", { localStorage: storage, matchMedia: media });
  vi.stubGlobal("document", { documentElement: html });
  const a = await import("./appearance");
  const g = await import("./glass");
  a.applyAppearance(html as unknown as HTMLElement, a.getAppearance());
  g.startGlass();
  await settle();
  return { a, g };
}

beforeEach(() => { store = new Map(); reduced = false; h.refuse = false; });
afterEach(() => { vi.unstubAllGlobals(); });

describe("the pieces", () => {
  it("guesses the system's blur from the user agent until the system answers", () => {
    expect(guessSupport(MAC)).toBe("vibrancy");
    expect(guessSupport(WINDOWS)).toBe("mica");
    expect(guessSupport(LINUX)).toBe("compositor");
  });
  it("shows the glass only with the switch on, a blur from the system and no reduced transparency", () => {
    for (const support of ["vibrancy", "mica", "compositor"] as const) {
      expect(glassOn(true, { support, reduced: false })).toBe(true);
      expect(glassOn(false, { support, reduced: false })).toBe(false);
      expect(glassOn(true, { support, reduced: true })).toBe(false);
    }
    expect(glassOn(true, { support: "none", reduced: false })).toBe(false);
  });
  it("asks for vibrancy under the window's background on macOS, Mica by the theme on Windows 11, nothing on Hyprland", () => {
    expect(effectsFor(true, "vibrancy", "dark")).toEqual(VIBRANCY);
    expect(effectsFor(true, "vibrancy", "light")).toEqual(VIBRANCY);
    expect(effectsFor(true, "mica", "dark")).toEqual({ effects: ["micaDark"] });
    expect(effectsFor(true, "mica", "light")).toEqual({ effects: ["micaLight"] });
    expect(effectsFor(true, "compositor", "dark")).toBeNull();
    for (const support of ["vibrancy", "mica", "compositor", "none"] as const) expect(effectsFor(false, support, "dark")).toBeNull();
  });
  it("has the notes' three hints, word for word", () => {
    const g = (support: Glass["support"], reduced = false): Glass => ({ support, reduced });
    expect(glassHint(g("vibrancy"))).toBe(HINT_ON);
    expect(glassHint(g("mica"))).toBe(HINT_ON);
    expect(glassHint(g("compositor"))).toBe(HINT_ON);
    expect(glassHint(g("none"))).toBe(HINT_NONE);
    expect(glassHint(g("none", true))).toBe(HINT_NONE);
    expect(glassHint(g("vibrancy", true))).toBe(HINT_REDUCED);
  });
});

describe("macOS", () => {
  it("is on by default: html.glass and vibrancy, and nothing is kept for it", async () => {
    const { a } = await start(MAC, "vibrancy");
    expect(a.getAppearance().glass).toBe(true);
    expect(glassy()).toBe(true);
    expect(h.asked).toEqual([VIBRANCY]);
    expect(store.size).toBe(0);
  });
  it("turning it off shows at once: the plain background and the effect cleared; kept after a restart", async () => {
    const { a } = await start(MAC, "vibrancy");
    a.setAppearance({ glass: false });
    expect(glassy()).toBe(false);
    expect(h.asked).toEqual([VIBRANCY, null]);
    expect(Object.fromEntries(store)).toEqual({ "gizai-glass": "false" });
    await start(MAC, "vibrancy");
    expect(glassy()).toBe(false);
    expect(h.asked).toEqual([null]);
    // on again: at once, and nothing is kept, as it is the default here
    const { a: again } = await start(MAC, "vibrancy");
    again.setAppearance({ glass: true });
    expect(glassy()).toBe(true);
    expect(h.asked).toEqual([null, VIBRANCY]);
    expect(store.size).toBe(0);
  });
  it("Reset to defaults turns it on again", async () => {
    const { a } = await start(MAC, "vibrancy");
    a.setAppearance({ glass: false, theme: "light" });
    a.resetAppearance();
    expect(a.getAppearance().glass).toBe(true);
    expect(glassy()).toBe(true);
    expect(store.size).toBe(0);
  });
  it("doesn't ask the window again for a change of font, size or theme (vibrancy is the same in both)", async () => {
    const { a } = await start(MAC, "vibrancy");
    a.setAppearance({ font: "inter" });
    a.setAppearance({ chat: 18 });
    a.setAppearance({ theme: "light" });
    expect(h.asked).toEqual([VIBRANCY]);
  });
  it("asks again at the next change when the effect was refused, and the page's side is right anyway", async () => {
    h.refuse = true;
    const { a } = await start(MAC, "vibrancy");
    expect(glassy()).toBe(true);
    h.refuse = false;
    a.setAppearance({ font: "geist" });
    await settle();
    a.setAppearance({ font: "hack" });
    expect(h.asked).toEqual([VIBRANCY, VIBRANCY]);
  });
});

describe("Windows", () => {
  it("11: on by default, Mica dark or light with the theme, changed at once", async () => {
    const { a } = await start(WINDOWS, "mica");
    expect(a.getAppearance().glass).toBe(true);
    expect(glassy()).toBe(true);
    expect(h.asked).toEqual([{ effects: ["micaDark"] }]);
    a.setAppearance({ theme: "light" });
    expect(h.asked).toEqual([{ effects: ["micaDark"] }, { effects: ["micaLight"] }]);
    a.setAppearance({ glass: false });
    expect(glassy()).toBe(false);
    expect(h.asked.at(-1)).toBeNull();
  });
  it("10 has no Mica: once the system answers, the background is plain and the effect cleared, whatever the switch says", async () => {
    const { a, g } = await start(WINDOWS, "none");
    expect(g.getGlass()).toEqual({ support: "none", reduced: false });
    expect(a.getAppearance().glass).toBe(true);
    expect(glassy()).toBe(false);
    expect(h.asked).toEqual([{ effects: ["micaDark"] }, null]);
    expect(glassHint(g.getGlass())).toBe(HINT_NONE);
  });
});

describe("Linux", () => {
  it("is off by default, also on Hyprland, and the window's effects are never asked for", async () => {
    const { a, g } = await start(LINUX, "compositor");
    expect(a.getAppearance().glass).toBe(false);
    expect(glassy()).toBe(false);
    expect(g.getGlass().support).toBe("compositor");
    expect(glassHint(g.getGlass())).toBe(HINT_ON);
    expect(h.asked).toEqual([]);
  });
  it("on Hyprland it can be turned on: html.glass at once, the compositor blurs; kept after a restart", async () => {
    const { a } = await start(LINUX, "compositor");
    a.setAppearance({ glass: true });
    expect(glassy()).toBe(true);
    expect(Object.fromEntries(store)).toEqual({ "gizai-glass": "true" });
    const { a: again } = await start(LINUX, "compositor");
    expect(again.getAppearance().glass).toBe(true);
    expect(glassy()).toBe(true);
    again.setAppearance({ glass: false });
    expect(glassy()).toBe(false);
    expect(store.size).toBe(0);
    expect(h.asked).toEqual([]);
  });
  it("other desktops give no blur: plain, even with the switch kept on, and the no-blur hint", async () => {
    store.set("gizai-glass", "true");
    const { a, g } = await start(LINUX, "none");
    expect(a.getAppearance().glass).toBe(true);
    expect(glassy()).toBe(false);
    expect(glassHint(g.getGlass())).toBe(HINT_NONE);
  });
  it("the dev hook's glass=true (screenshots) shows the glass but isn't kept", async () => {
    const { a } = await start(LINUX, "compositor");
    a.setAppearance(a.appearanceOf("glass=true"), false);
    expect(glassy()).toBe(true);
    expect(store.size).toBe(0);
  });
});

describe("reduced transparency (prefers-reduced-transparency)", () => {
  it("keeps the background plain and clears the effect, while the switch keeps its value", async () => {
    reduced = true;
    const { a, g } = await start(MAC, "vibrancy");
    expect(a.getAppearance().glass).toBe(true);
    expect(glassy()).toBe(false);
    expect(h.asked).toEqual([null]);
    expect(glassHint(g.getGlass())).toBe(HINT_REDUCED);
    expect(store.size).toBe(0);
  });
  it("follows the system's change at once, without a restart", async () => {
    const { g } = await start(MAC, "vibrancy");
    expect(glassy()).toBe(true);
    onReduced.forEach((f) => f({ matches: true }));
    expect(glassy()).toBe(false);
    expect(g.getGlass().reduced).toBe(true);
    onReduced.forEach((f) => f({ matches: false }));
    expect(glassy()).toBe(true);
    expect(h.asked).toEqual([VIBRANCY, null, VIBRANCY]);
  });
});

describe("without an answer from the system (the browser mode)", () => {
  it("keeps the guess and still starts", async () => {
    const { a, g } = await start(MAC, new Error("no such command"));
    expect(g.getGlass().support).toBe("vibrancy");
    expect(a.getAppearance().glass).toBe(true);
    expect(glassy()).toBe(true);
  });
  it("starts once only, however often startGlass is called", async () => {
    const { a, g } = await start(MAC, "vibrancy");
    g.startGlass();
    g.startGlass();
    a.setAppearance({ glass: false });
    expect(h.asked).toEqual([VIBRANCY, null]);
    expect(onReduced.length).toBe(1);
  });
});
