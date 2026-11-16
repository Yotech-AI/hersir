// GA-200 QA: Settings → Updates for a Hersir from a package manager (AUR, Homebrew, Scoop or another word), against the
// card's texts word for word and GA-199's contract (package, updateCommand, cannotInstall, installTo null). Rendered to
// HTML on the server with the update status given (useUpdate is replaced), so no data loads and nothing calls Hersir.
// Copy is pressed by calling UpdateSettings as a function with a plain stand-in for useState, as RunForMe.test.tsx does,
// and the clipboard is a fake that records what it was given. Without a package it looks as before (GA-1, GA-253 in
// UpdateNotice.test.tsx), also with the browser fixtures' update status.
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import type { Release, UpdateStatus } from "../types";

const { fake } = vi.hoisted(() => ({ fake: { on: false } }));
vi.mock("react", async (orig) => {
  const R = await orig<typeof import("react")>();
  const useState = ((init: unknown) => (fake.on ? [typeof init === "function" ? (init as () => unknown)() : init, () => {}] : R.useState(init))) as typeof R.useState;
  return { ...R, useState, default: { ...R, useState } };
});
let current: UpdateStatus | null = null;
vi.mock("../lib/useUpdate", () => ({ useUpdate: () => [current, () => {}] }));

const { UpdateSettings } = await import("./UpdateSettings");
const fixtures = (await import("../browser/fixtures.json")).default as unknown as { commands: { update_status: UpdateStatus } };

const release: Release = { version: "0.16.0", tag: "v0.16.0", url: "https://github.com/Yotech-AI/hersir/releases/tag/v0.16.0", notes: "- Packages" };
const status = (more: Partial<UpdateStatus> = {}): UpdateStatus => ({
  current: "0.15.0", autoCheck: true, checking: false, repo: "https://github.com/Yotech-AI/hersir.git", checkedAt: Date.now(),
  installTo: "/home/you/.local", ...more,
});
const text = (html: string) => html.replace(/<[^>]+>/g, " ").replace(/&#x27;/g, "'").replace(/&quot;/g, '"').replace(/&lt;/g, "<")
  .replace(/&gt;/g, ">").replace(/&amp;/g, "&").replace(/\s+/g, " ").trim();
const html = () => renderToStaticMarkup(<UpdateSettings />);
/** The section's text: the <p> under the Updates title. */
const sectionText = (h: string) => text(/<header><h3>Updates<\/h3><p>(.*?)<\/p><\/header>/.exec(h)?.[1] ?? "");

const STILL_ASKS = "Hersir still asks GitHub for new releases and tells you when one is out.";
const PACKAGES = [
  { package: "aur", updateCommand: "yay -Syu",
    cannotInstall: "Hersir came from the AUR, so it doesn't update itself. Update it with yay -Syu, then restart Hersir.",
    section: `Hersir came from the AUR, so yay updates it, not Hersir itself. ${STILL_ASKS}`,
    hint: "The AUR package follows each release, usually on the same day." },
  { package: "homebrew", updateCommand: "brew update && brew upgrade hersir",
    cannotInstall: "Hersir came from Homebrew, so it doesn't update itself. Update it with brew update && brew upgrade hersir, then restart Hersir.",
    section: `Hersir came from Homebrew, so brew updates it, not Hersir itself. ${STILL_ASKS}`,
    hint: "The formula in yotech-ai/tap follows each release, usually on the same day." },
  { package: "scoop", updateCommand: "scoop update; scoop update hersir",
    cannotInstall: "Hersir came from Scoop, so it doesn't update itself. Update it with scoop update; scoop update hersir, then restart Hersir.",
    section: `Hersir came from Scoop, so scoop updates it, not Hersir itself. ${STILL_ASKS}`,
    hint: "The Scoop bucket follows each release, usually on the same day." },
];
const OTHER_CANNOT = "Hersir came from a package, so it doesn't update itself. Update it with the package manager that installed it, then restart Hersir.";
const OTHER_SECTION = `Hersir came from a package, so its package manager updates it, not Hersir itself. ${STILL_ASKS}`;
const TODAY = "Hersir updates from its signed releases on GitHub. An update checks the release's signature, downloads its ready-made build for your system in the background (or builds it from source when there is none), backs up your data, installs it and offers a restart. If a step fails, the installed version keeps working.";
const ALL_HINTS = PACKAGES.map((p) => p.hint);
const pkgStatus = (p: (typeof PACKAGES)[number], more: Partial<UpdateStatus> = {}) =>
  status({ latest: release, available: release, installTo: null, package: p.package, updateCommand: p.updateCommand, cannotInstall: p.cannotInstall, ...more });

// A rendered tree, as elements: what UpdateSettings returned, with the elements in its props (children) too.
type El = { type: unknown; props: Record<string, unknown> };
const isEl = (x: unknown): x is El => !!x && typeof x === "object" && "type" in x && "props" in x;
function elements(node: unknown, out: El[] = []): El[] {
  if (Array.isArray(node)) node.forEach((n) => elements(n, out));
  else if (isEl(node)) { out.push(node); Object.values(node.props).forEach((v) => elements(v, out)); }
  return out;
}
function call(): El[] {
  fake.on = true;
  try { return elements(UpdateSettings()); } finally { fake.on = false; }
}
const copyButtons = () => call().filter((e) => e.type === "button" && e.props.title === "Copy the command");

beforeEach(() => { current = null; });
afterEach(() => vi.unstubAllGlobals());

describe("Settings → Updates for a Hersir from the AUR, Homebrew or Scoop", () => {
  it("says its package updates it, then cannotInstall, the command in mono with Copy, and the package's hint, word for word", () => {
    for (const p of PACKAGES) {
      current = pkgStatus(p);
      const h = html();
      expect(sectionText(h)).toBe(p.section);
      expect(h).not.toContain("Hersir updates from its signed releases on GitHub.");
      const t = text(h);
      expect(t).toContain("Hersir 0.15.0. Version 0.16.0 is out");
      expect(t).toContain(`${p.cannotInstall} ${p.updateCommand} Copy ${p.hint}`);
      // the cannotInstall line with its minus icon, then the command row, then the hint, in that order inside Update
      const line = /<div class="gh-line skipped"><svg[^>]*lucide-circle-minus[^>]*>.*?<\/svg><span>(.*?)<\/span><\/div>/.exec(h);
      expect(text(line?.[1] ?? "")).toBe(p.cannotInstall);
      const row = /<div class="update-command"><code class="mono">(.*?)<\/code><button class="btn ghost sm" title="Copy the command"><svg[^>]*lucide-copy[^>]*>.*?<\/svg>Copy<\/button><\/div>/.exec(h);
      expect(row).not.toBeNull();
      expect(text(row![1]!)).toBe(p.updateCommand);
      expect(h.indexOf(`<span class="hint">${p.hint}</span>`)).toBeGreaterThan(h.indexOf('class="update-command"'));
      expect(h.indexOf('class="update-command"')).toBeGreaterThan(h.indexOf("lucide-circle-minus"));
      // it doesn't update itself: no Update button and no Installs into, and the other packages' hints aren't there
      expect(h).not.toContain("btn primary");
      expect(t).not.toContain("Update to 0.16.0");
      expect(t).not.toContain("Installs into");
      for (const other of ALL_HINTS.filter((x) => x !== p.hint)) expect(t).not.toContain(other);
    }
  });
  it("shows the command as text, so && isn't markup", () => {
    current = pkgStatus(PACKAGES[1]!);
    expect(html()).toContain('<code class="mono">brew update &amp;&amp; brew upgrade hersir</code>');
  });
  it("Copy puts exactly updateCommand on the clipboard", async () => {
    for (const p of PACKAGES) {
      const writeText = vi.fn(async (_t: string) => {});
      vi.stubGlobal("navigator", { clipboard: { writeText } });
      current = pkgStatus(p);
      const copies = copyButtons();
      expect(copies).toHaveLength(1);
      expect(copies[0]!.props.className).toBe("btn ghost sm");
      (copies[0]!.props.onClick as () => void)();
      expect(writeText.mock.calls).toEqual([[p.updateCommand]]);
      vi.unstubAllGlobals();
    }
  });
  it("Copy doesn't throw when the clipboard refuses, or there is none", async () => {
    current = pkgStatus(PACKAGES[0]!);
    vi.stubGlobal("navigator", { clipboard: { writeText: () => Promise.reject(new Error("not allowed")) } });
    expect(() => (copyButtons()[0]!.props.onClick as () => void)()).not.toThrow();
    await new Promise((r) => setTimeout(r, 0));
    vi.stubGlobal("navigator", {});
    expect(() => (copyButtons()[0]!.props.onClick as () => void)()).not.toThrow();
  });
  it("without updateCommand shows the package's text and cannotInstall, with no command, no Copy and no hint", () => {
    current = pkgStatus(PACKAGES[0]!, { updateCommand: null });
    const h = html();
    expect(sectionText(h)).toBe(PACKAGES[0]!.section);
    expect(text(h)).toContain(PACKAGES[0]!.cannotInstall);
    expect(h).not.toContain("update-command");
    expect(h).not.toContain("Copy");
    expect(text(h)).not.toContain(PACKAGES[0]!.hint);
  });
  it("with nothing newer out, or a release that isn't signed yet, says its package updates it and shows no command", () => {
    current = pkgStatus(PACKAGES[0]!, { latest: { ...release, version: "0.15.0", tag: "v0.15.0" }, available: null });
    let h = html();
    expect(sectionText(h)).toBe(PACKAGES[0]!.section);
    expect(text(h)).toContain("Hersir 0.15.0. This is the latest version (checked just now).");
    expect(h).not.toContain("update-command");
    expect(text(h)).not.toContain(PACKAGES[0]!.cannotInstall);
    current = pkgStatus(PACKAGES[0]!, { available: null, unsigned: release });
    h = html();
    expect(sectionText(h)).toBe(PACKAGES[0]!.section);
    expect(text(h)).toContain("Hersir 0.15.0. v0.16.0 is out, but not signed yet.");
    expect(h).not.toContain("update-command");
  });
  it("keeps Check for new releases and Check now", () => {
    current = pkgStatus(PACKAGES[0]!);
    const h = html();
    expect(h).toMatch(/<input type="checkbox" checked=""\/>Check for new releases automatically/);
    expect(text(h)).toContain("Check now");
  });
});

describe("Settings → Updates for a Hersir from another package", () => {
  it("says a package manager updates it and shows cannotInstall, with no command, no Copy and no hint", () => {
    for (const word of ["nix", "flatpak"]) {
      current = status({ latest: release, available: release, installTo: null, package: word, updateCommand: null, cannotInstall: OTHER_CANNOT });
      const h = html();
      expect(sectionText(h)).toBe(OTHER_SECTION);
      expect(text(h)).toContain(OTHER_CANNOT);
      expect(h).not.toContain("update-command");
      expect(h).not.toContain("<code");
      expect(h).not.toContain("Copy");
      expect(h).not.toContain("btn primary");
      for (const hint of ALL_HINTS) expect(text(h)).not.toContain(hint);
      expect(h).not.toContain("follows each release");
    }
  });
});

describe("Settings → Updates without a package looks as before", () => {
  it("an install script's Hersir (package null) has today's text and Update to <version>, with no command", () => {
    current = status({ latest: release, available: release, package: null, updateCommand: null });
    const h = html();
    expect(sectionText(h)).toBe(TODAY);
    const t = text(h);
    expect(t).toContain("Update to 0.16.0");
    expect(t).toContain("Installs into /home/you/.local.");
    expect(h).not.toContain("update-command");
    expect(h).not.toContain("Copy");
  });
  it("a dev build (package null) shows its cannotInstall line with no command and no hint", () => {
    const why = "This Gizai runs from /repo/target/release/gizai, not from an install";
    current = status({ latest: release, available: release, installTo: null, cannotInstall: why, package: null, updateCommand: null });
    const h = html();
    expect(sectionText(h)).toBe(TODAY);
    expect(h).toMatch(new RegExp(`<div class="gh-line skipped"><svg[^>]*lucide-circle-minus[^>]*>.*?</svg><span>${why}</span></div>`));
    expect(h).not.toContain("update-command");
    expect(h).not.toContain("Copy");
    expect(h).not.toContain("btn primary");
    expect(h).not.toContain("follows each release");
  });
  it("shows no command when updateCommand comes without a package", () => {
    current = status({ latest: release, available: release, package: null, updateCommand: "yay -Syu" });
    const h = html();
    expect(sectionText(h)).toBe(TODAY);
    expect(h).not.toContain("yay -Syu");
    expect(text(h)).toContain("Update to 0.16.0");
  });
  it("the browser fixtures' update status has package and updateCommand null, and shows today's Settings → Updates", () => {
    const s = fixtures.commands.update_status;
    expect(s.package).toBeNull();
    expect(s.updateCommand).toBeNull();
    current = s;
    const h = html();
    expect(sectionText(h)).toBe(TODAY);
    expect(h).not.toContain("update-command");
  });
  it("the browser fixtures with package aur and a newer release (the card's screenshot) show the AUR's Settings → Updates", () => {
    const aur = PACKAGES[0]!;
    current = { ...fixtures.commands.update_status, available: release, latest: release, installTo: null,
      package: aur.package, updateCommand: aur.updateCommand, cannotInstall: aur.cannotInstall };
    const t = text(html());
    expect(t).toContain(aur.section);
    expect(t).toContain(`${aur.cannotInstall} yay -Syu Copy ${aur.hint}`);
  });
});
