// The sidebar's Terminal (GA-136), Editor (GA-113), Agents, Docs and Memory sections fold (GA-90). Which are folded is kept on this computer in
// localStorage as a comma list ("agents,memory"), the way the Memory page keeps its panel's closed sections; all start
// open.
import { useCallback, useState } from "react";

export const FOLDED_KEY = "gizai.sidebar.folded";
/** The sections that fold. */
export type Fold = "terminal" | "editor" | "agents" | "docs" | "memory";

/** The folded sections in what localStorage keeps (null: nothing kept, so none). */
export function foldedOf(kept: string | null): string[] {
  return (kept ?? "").split(",").map((x) => x.trim()).filter(Boolean);
}

/** What to keep after a click on a section's caret: that section folded if it was open and open if it was folded, the
 *  others as they were. */
export function toggleFold(kept: string | null, section: Fold): string {
  const list = foldedOf(kept);
  return (list.includes(section) ? list.filter((x) => x !== section) : [...list, section]).join(",");
}

/** The live runs of the agents the sidebar lists, together: folded Agents shows them on its heading. */
export function liveTotal(live: { agentId: string }[], agents: { actorId: string }[]): number {
  return live.filter((r) => agents.some((a) => a.actorId === r.agentId)).length;
}

function read(): string | null { try { return localStorage.getItem(FOLDED_KEY); } catch { return null; } }

/** Whether a section is folded, and the caret's click: it folds or opens the section and keeps that. */
export function useFolded(): [(section: Fold) => boolean, (section: Fold) => void] {
  const [kept, setKept] = useState<string | null>(read);
  const toggle = (section: Fold) => {
    const next = toggleFold(kept, section);
    setKept(next);
    try { localStorage.setItem(FOLDED_KEY, next); } catch { /* private mode */ }
  };
  return [(section) => foldedOf(kept).includes(section), toggle];
}

// ---------- the sidebar closes (GA-291; design/islands/README.md → A sidebar that closes) ----------

/** Kept on this computer like the folded sections: "1" while the sidebar is closed; nothing kept means open. */
export const CLOSED_KEY = "gizai.sidebar.closed";

/** Whether what localStorage keeps says the sidebar is closed (null: nothing kept, so open). */
export function closedOf(kept: string | null): boolean {
  return kept === "1";
}

/** The key that closes and opens the sidebar: Ctrl+\ (Cmd+\ on macOS), with no Shift or Alt. Read by the key's place
 *  (Backslash) or by what it types, so it works on layouts where \ sits elsewhere. */
export function isSidebarKey(e: { key: string; code: string; ctrlKey: boolean; metaKey: boolean; shiftKey: boolean; altKey: boolean }): boolean {
  return (e.ctrlKey || e.metaKey) && !e.shiftKey && !e.altKey && (e.code === "Backslash" || e.key === "\\");
}

/** The key as a tooltip names it: "Ctrl+\" (or "Cmd+\" on macOS, `mod` from lib/keys' modKey). */
export const sidebarKeyName = (mod: "Cmd" | "Ctrl") => `${mod}+\\`;

/** The Inbox button's tooltip and its name for screen readers in the closed sidebar's toolbar: "Inbox (3 need you)" and
 *  "Inbox, 3 need you", or "Inbox" with nothing waiting. */
export function inboxTip(count: number): { title: string; label: string } {
  return count > 0 ? { title: `Inbox (${count} need you)`, label: `Inbox, ${count} need you` } : { title: "Inbox", label: "Inbox" };
}

function readClosed(): boolean { try { return closedOf(localStorage.getItem(CLOSED_KEY)); } catch { return false; } }

function keepClosed(closed: boolean) {
  try { if (closed) localStorage.setItem(CLOSED_KEY, "1"); else localStorage.removeItem(CLOSED_KEY); } catch { /* private mode */ }
}

/** Whether the sidebar is closed, and how to close or open it (a value, or a function of the last one): kept at once, so
 *  it stays so after a restart. */
export function useSidebarClosed(): [boolean, (next: boolean | ((was: boolean) => boolean)) => void] {
  const [closed, setClosed] = useState<boolean>(readClosed);
  const set = useCallback((next: boolean | ((was: boolean) => boolean)) => {
    setClosed((was) => {
      const now = typeof next === "function" ? next(was) : next;
      keepClosed(now);
      return now;
    });
  }, []);
  return [closed, set];
}
