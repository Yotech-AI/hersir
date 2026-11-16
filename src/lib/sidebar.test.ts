// GA-90 QA: which of the sidebar's Agents and Memory are folded, as kept in localStorage, and the live runs folded
// Agents shows on its heading.
import { describe, expect, it } from "vitest";
import { CLOSED_KEY, closedOf, FOLDED_KEY, foldedOf, inboxTip, isSidebarKey, liveTotal, sidebarKeyName, toggleFold } from "./sidebar";

describe("the sidebar's folded sections in localStorage (GA-90)", () => {
  it("is kept under gizai.sidebar.folded", () => {
    expect(FOLDED_KEY).toBe("gizai.sidebar.folded");
  });
  it("starts with nothing folded on a computer that never folded them", () => {
    expect(foldedOf(null)).toEqual([]);
    expect(foldedOf("")).toEqual([]);
  });
  it("reads the comma list, leaving out blanks and spaces", () => {
    expect(foldedOf("agents,memory")).toEqual(["agents", "memory"]);
    expect(foldedOf(" memory , ,agents,")).toEqual(["memory", "agents"]);
  });
  it("folds an open section and opens a folded one", () => {
    expect(toggleFold(null, "agents")).toBe("agents");
    expect(toggleFold("agents", "agents")).toBe("");
    expect(toggleFold("", "memory")).toBe("memory");
  });
  it("leaves the other section as it was", () => {
    expect(toggleFold("agents", "memory")).toBe("agents,memory");
    expect(toggleFold("agents,memory", "agents")).toBe("memory");
    expect(toggleFold("agents,memory", "memory")).toBe("agents");
    expect(foldedOf(toggleFold(toggleFold(null, "memory"), "agents"))).toEqual(["memory", "agents"]);
  });
  it("folds a section only once, even when it was kept twice", () => {
    expect(toggleFold("agents,agents", "agents")).toBe("");
  });
});

describe("the live runs on folded Agents' heading (GA-90)", () => {
  const agents = [{ actorId: "a1" }, { actorId: "a2" }, { actorId: "a3" }];
  it("adds up every live run of the agents the sidebar lists", () => {
    expect(liveTotal([{ agentId: "a1" }, { agentId: "a1" }, { agentId: "a3" }], agents)).toBe(3);
  });
  it("is 0 with nothing running", () => {
    expect(liveTotal([], agents)).toBe(0);
    expect(liveTotal([], [])).toBe(0);
  });
  it("leaves out runs of agents the sidebar doesn't list (another team's), like the per-agent tags", () => {
    expect(liveTotal([{ agentId: "a2" }, { agentId: "elsewhere" }], agents)).toBe(1);
  });
});

// GA-291 QA: the sidebar closes (design/islands/README.md → A sidebar that closes). Kept as "1" under gizai.sidebar.closed
// while closed, nothing while open; the key is Ctrl+\ (Cmd+\ on macOS) without Shift or Alt; the tooltips name it.
describe("the closed sidebar, as kept on this computer (GA-291)", () => {
  it("is kept under gizai.sidebar.closed, next to the folded sections", () => {
    expect(CLOSED_KEY).toBe("gizai.sidebar.closed");
  });
  it("is open on a computer that never closed it, and closed only for '1'", () => {
    expect(closedOf(null)).toBe(false);
    expect(closedOf("")).toBe(false);
    expect(closedOf("1")).toBe(true);
    expect(closedOf("0")).toBe(false);
    expect(closedOf("true")).toBe(false);
  });
});

describe("Ctrl+\\ closes and opens the sidebar (GA-291)", () => {
  const key = (more: Partial<{ key: string; code: string; ctrlKey: boolean; metaKey: boolean; shiftKey: boolean; altKey: boolean }>) =>
    isSidebarKey({ key: "\\", code: "Backslash", ctrlKey: false, metaKey: false, shiftKey: false, altKey: false, ...more });
  it("is Ctrl+\\, and Cmd+\\ on macOS", () => {
    expect(key({ ctrlKey: true })).toBe(true);
    expect(key({ metaKey: true })).toBe(true);
  });
  it("needs Ctrl or Cmd: \\ alone types a backslash", () => {
    expect(key({})).toBe(false);
  });
  it("is not Ctrl+Shift+\\ or Ctrl+Alt+\\ (CodeMirror's), nor Cmd with them", () => {
    expect(key({ ctrlKey: true, shiftKey: true, key: "|" })).toBe(false);
    expect(key({ ctrlKey: true, altKey: true })).toBe(false);
    expect(key({ metaKey: true, shiftKey: true })).toBe(false);
    expect(key({ metaKey: true, altKey: true })).toBe(false);
  });
  it("is read by the key's place or by what it types, so other layouts have it too", () => {
    // the key's place on a US layout, whatever it types (a German layout types # there)
    expect(key({ ctrlKey: true, key: "#" })).toBe(true);
    // a layout with \ elsewhere: what it types
    expect(key({ ctrlKey: true, code: "IntlBackslash" })).toBe(true);
  });
  it("is no other key with Ctrl: B (the board and the Editor's), K (Search), ]", () => {
    expect(key({ ctrlKey: true, key: "b", code: "KeyB" })).toBe(false);
    expect(key({ ctrlKey: true, key: "k", code: "KeyK" })).toBe(false);
    expect(key({ ctrlKey: true, key: "]", code: "BracketRight" })).toBe(false);
  });
  it("is named Ctrl+\\ in a tooltip, Cmd+\\ on macOS", () => {
    expect(sidebarKeyName("Ctrl")).toBe("Ctrl+\\");
    expect(sidebarKeyName("Cmd")).toBe("Cmd+\\");
  });
});

describe("the closed sidebar's Inbox tooltip (GA-291)", () => {
  it("says how many need you, as the notes' Inbox (3 need you)", () => {
    expect(inboxTip(3)).toEqual({ title: "Inbox (3 need you)", label: "Inbox, 3 need you" });
    expect(inboxTip(1).title).toMatch(/^Inbox \(1 needs? you\)$/);
  });
  it("is just Inbox with nothing waiting", () => {
    expect(inboxTip(0)).toEqual({ title: "Inbox", label: "Inbox" });
  });
});
