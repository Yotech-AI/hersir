// GA-306: moving and archiving several cards of a column at once (the board's Move all, Archive all and a selection of
// ticked cards): the question Move all asks, the toast after it, and where the moved cards show before the backend
// answers.
import type { BulkResult, Task } from "../types";
import { keyBetween } from "./sortKey";

type Skip = BulkResult["skipped"][number];

/** "GA-1", "GA-1 and GA-2", "GA-1, GA-2 and GA-3". */
function andList(names: string[]): string {
  return names.length < 2 ? names.join("") : `${names.slice(0, -1).join(", ")} and ${names[names.length - 1]}`;
}

/** A reason without its full stop, so a sentence can end it. */
const bare = (reason: string) => reason.trim().replace(/\.$/, "");

/** What the toast says of the cards a move or an archive skipped, after its count: "GA-303 wasn't: An agent is working
 *  on this card." With one reason for all: "GA-303, GA-304 and GA-305 weren't: …", and with more than three, the first
 *  three, then "and 4 more". With several reasons, each card with its own: "GA-303 wasn't: …; GA-12 wasn't: …; and 4
 *  more." Nothing skipped: "". */
export function skippedNote(skipped: Skip[]): string {
  if (skipped.length === 0) return "";
  const first = skipped.slice(0, 3);
  const more = skipped.length - first.length;
  if (skipped.every((s) => s.reason === skipped[0].reason)) {
    const ids = first.map((s) => s.identifier);
    const names = more > 0 ? `${ids.join(", ")} and ${more} more` : andList(ids);
    return `${names} ${skipped.length === 1 ? "wasn't" : "weren't"}: ${bare(skipped[0].reason)}.`;
  }
  return `${first.map((s) => `${s.identifier} wasn't: ${bare(s.reason)}`).join("; ")}${more > 0 ? `; and ${more} more` : ""}.`;
}

/** The toast after a move: "Moved 12 cards to Done" ("Moved GA-12 to Done"), then what was skipped. */
export function movedText(moved: string[], to: string, skipped: Skip[] = []): string {
  const head = moved.length === 0 ? "Nothing was moved" : moved.length === 1 ? `Moved ${moved[0]} to ${to}` : `Moved ${moved.length} cards to ${to}`;
  return skipped.length ? `${head}. ${skippedNote(skipped)}` : head;
}

/** The toast after an archive: "Archived 12 cards" ("GA-12 archived"), then what was skipped. */
export function archivedText(archived: string[], skipped: Skip[] = []): string {
  const head = archived.length === 0 ? "Nothing was archived" : archived.length === 1 ? `${archived[0]} archived` : `Archived ${archived.length} cards`;
  return skipped.length ? `${head}. ${skippedNote(skipped)}` : head;
}

/** The question Move all asks (Deploy's): "Move all 140 cards in Deploy to Done?", one card "Move GA-12 to Done?"; and
 *  its button, "Move 140 cards". */
export function askMoveAll(identifiers: string[], from: string, to: string): { question: string; go: string } {
  const n = identifiers.length;
  return n === 1
    ? { question: `Move ${identifiers[0]} to ${to}?`, go: "Move 1 card" }
    : { question: `Move all ${n} cards in ${from} to ${to}?`, go: `Move ${n} cards` };
}

/** The question Archive all asks (Done's): "Archive all 544 cards in Done? The bin …", one card "Archive GA-12?"; and
 *  its button, "Archive 544 cards". */
export function askArchiveAll(identifiers: string[], from: string): { question: string; go: string } {
  const n = identifiers.length;
  return n === 1
    ? { question: `Archive ${identifiers[0]}?`, go: "Archive 1 card" }
    : { question: `Archive all ${n} cards in ${from}? The bin on the Tasks page keeps them, and Restore puts one back.`, go: `Archive ${n} cards` };
}

/** A sort key after `key` (null: the first). A key fractional-indexing can't read gets a character more, still after it. */
function after(key: string | null): string {
  try { return keyBetween(key, null); } catch { return `${key ?? "a0"}V`; }
}

/** `tasks` with the cards `ids` at the bottom of column `to`, one after the other in that order: where a move or an
 *  Undo puts them before the backend answers (it does the same). */
export function toBottom(tasks: Task[], ids: string[], to: { id: string; name: string; category: string }): Task[] {
  const moving = new Set(ids);
  let key: string | null = null;
  for (const t of tasks) if (t.stateId === to.id && !moving.has(t.id) && (key === null || t.sortKey > key)) key = t.sortKey;
  const keys = new Map<string, string>();
  for (const id of ids) { key = after(key); keys.set(id, key); }
  return tasks.map((t) => {
    const k = keys.get(t.id);
    return k === undefined ? t : { ...t, stateId: to.id, stateName: to.name, stateCategory: to.category, sortKey: k };
  });
}

/** The ids of `cards` a bulk command skipped (it names them by identifier). */
export function skippedIds(cards: Task[], skipped: Skip[]): Set<string> {
  const names = new Set(skipped.map((s) => s.identifier));
  return new Set(cards.filter((t) => names.has(t.identifier)).map((t) => t.id));
}
