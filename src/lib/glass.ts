// The glass behind the islands (GA-292; design/islands/README.md → The glass, and design/islands-settings/README.md →
// Appearance: the glass switch). The window is see-through (src-tauri/tauri.conf.json: transparent). With the glass on,
// the page paints the gradient at 92% in dark and 86% in light (glass-from, glass-to, glass-glow) and the system blurs the
// desktop behind it: vibrancy on macOS, Mica on Windows 11, and on Hyprland the compositor's own blur (when it is on). The islands and what floats stay solid. Otherwise the background is GA-290's opaque gradient and the window's
// effects are cleared.
//
// The glass is on when Settings → Appearance's switch is on (lib/appearance.ts: glass), the system gives a blur (not
// Windows 10, not Linux other than Hyprland), and the system doesn't ask for reduced transparency (prefers-reduced-transparency, where the web view knows
// it). html.glass is the page's side (app.css). A change of any of the three shows at once, without a restart.
import { useEffect, useState } from "react";
import { Effect, EffectState, type Effects } from "@tauri-apps/api/window";
import { glassSupport, setWindowEffects } from "../api";
import { getAppearance, onAppearance, type Theme } from "./appearance";

/** What blurs the desktop behind the window (src-tauri/src/glass.rs): vibrancy on macOS, Mica on Windows 11, Hyprland's
 *  compositor by its own rules (Hersir can't tell whether its blur is on), and nothing on an older Windows or on Linux
 *  other than Hyprland. */
export type GlassSupport = "vibrancy" | "mica" | "compositor" | "none";
const SUPPORTS: readonly unknown[] = ["vibrancy", "mica", "compositor", "none"];
/** The system's side: its blur, and whether it asks for reduced transparency. */
export type Glass = { support: GlassSupport; reduced: boolean };

/** Until the system answers (at start): macOS has vibrancy, Windows is taken to be 11 and Linux to be Hyprland (an older
 *  Windows or another desktop says "none" a moment later; the glass is off by default on Linux anyway). */
export function guessSupport(userAgent: string): GlassSupport {
  return /Macintosh|Mac OS X/.test(userAgent) ? "vibrancy" : /Windows/.test(userAgent) ? "mica" : "compositor";
}

/** Whether the glass shows: the switch on, a blur from the system, and no reduced transparency. */
export function glassOn(want: boolean, g: Glass): boolean {
  return want && g.support !== "none" && !g.reduced;
}

/** The effect to ask the window for, or null to clear it. macOS: vibrancy under the window's background, active only
 *  while the window is (as the system's own windows); Windows 11: Mica, dark or light with the theme. Linux has no such
 *  call: the compositor blurs the see-through window or doesn't. */
export function effectsFor(on: boolean, support: GlassSupport, theme: Theme): Effects | null {
  if (!on) return null;
  if (support === "vibrancy") return { effects: [Effect.UnderWindowBackground], state: EffectState.FollowsWindowActiveState };
  // Tauri has micaDark and micaLight in Rust (WindowEffect, camelCase) but not in the TypeScript enum
  if (support === "mica") return { effects: [(theme === "dark" ? "micaDark" : "micaLight") as unknown as Effect] };
  return null;
}

/** Settings → Appearance → Background → Glass: its hint, by the system's side: no blur (Windows 10, Linux other than
 *  Hyprland), reduced transparency, or what it does (design/islands-settings/README.md, word for word). */
export function glassHint(g: Glass): string {
  if (g.support === "none") return "Your system doesn't blur windows behind Hersir, so the background stays plain.";
  if (g.reduced) return "Plain while your system reduces transparency.";
  return "Blurred and tinted, on macOS, Windows 11 and Hyprland. Off gives a plain background.";
}

const REDUCED = "(prefers-reduced-transparency: reduce)";
function reducedQuery(): MediaQueryList | null {
  try { return typeof window !== "undefined" && typeof window.matchMedia === "function" ? window.matchMedia(REDUCED) : null; } catch { return null; }
}

let state: Glass | null = null;
const listeners = new Set<(g: Glass) => void>();
/** The effects last asked for (as JSON), so a change of font or size doesn't ask the window again. */
let asked: string | undefined;
let started = false;

/** The system's side now: guessed from the user agent until the system answers. */
export function getGlass(): Glass {
  if (!state) {
    const ua = typeof navigator === "undefined" ? "" : navigator.userAgent || "";
    state = { support: guessSupport(ua), reduced: reducedQuery()?.matches ?? false };
  }
  return state;
}

function update(patch: Partial<Glass>) {
  state = { ...getGlass(), ...patch };
  applyGlass();
  const g = state;
  listeners.forEach((f) => f(g));
}

/** Puts the glass on the window, or takes it off: html.glass for the page, and the system's effect. */
export function applyGlass() {
  const g = getGlass();
  const a = getAppearance();
  const on = glassOn(a.glass, g);
  if (typeof document !== "undefined") document.documentElement.classList.toggle("glass", on);
  if (g.support === "compositor") return;
  const effects = effectsFor(on, g.support, a.theme);
  const key = JSON.stringify(effects);
  if (key === asked) return;
  asked = key;
  // refused or failed: the page's side is right anyway; the next change asks again
  setWindowEffects(effects).catch(() => { asked = undefined; });
}

/** At start (main.tsx), right after the appearance and before the first paint: puts the glass on, then follows the switch
 *  and the theme (lib/appearance.ts), reduced transparency, and the system's answer about its blur. */
export function startGlass() {
  if (started) return;
  started = true;
  applyGlass();
  onAppearance(() => applyGlass());
  const q = reducedQuery();
  const onReduced = (e: MediaQueryListEvent) => update({ reduced: e.matches });
  if (q?.addEventListener) q.addEventListener("change", onReduced);
  else q?.addListener?.(onReduced);
  glassSupport()
    .then((support) => { if (SUPPORTS.includes(support) && support !== getGlass().support) update({ support }); })
    .catch(() => { /* no answer (the browser mode's fake backend, a test): the guess stays */ });
}

/** The system's side, kept up to date (Settings → Appearance's hints). */
export function useGlass(): Glass {
  const [g, setG] = useState(getGlass);
  useEffect(() => {
    listeners.add(setG);
    setG(getGlass());
    return () => { listeners.delete(setG); };
  }, []);
  return g;
}
