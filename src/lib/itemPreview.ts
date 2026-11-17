// The @ chip's hover card (GA-287, design/islands/README.md → The chip's hover card): what it shows of an item, and where.
// An item is read only when its card is about to show, never for every chip on the screen, and kept for a minute: resting
// on the same chip again within that minute reads nothing.
import { getAgent, getClient, getDoc, getProject, getTask, itemId, listUsers, liveRuns } from "../api";
import type { Client, Doc, Member, Person, Project, Task } from "../types";
import type { ItemKind } from "./itemLinks";

/** What the card shows of an item. An agent's `workingOn`: the id of the card it runs on now (GA-12). `missing`: deleted, or
 *  a key that matches nothing. */
export type ItemPreview =
  | { kind: "task"; task: Task }
  | { kind: "project"; project: Project }
  | { kind: "client"; client: Client }
  | { kind: "agent"; agent: Member; workingOn: string | undefined }
  | { kind: "person"; person: Person }
  | { kind: "doc"; doc: Doc; project: Project | undefined }
  | { kind: "missing" };

/** How long a read item is kept. */
export const KEEP_MS = 60_000;

type Kept = { at: number; answer: Promise<ItemPreview>; value?: ItemPreview };
const kept = new Map<string, Kept>();
const keyOf = (kind: ItemKind, key: string) => `${kind}\n${key}`;
const MISSING: ItemPreview = { kind: "missing" };

/** The item when it was read less than a minute ago and has arrived; else undefined. */
export function keptPreview(kind: ItemKind, key: string, now = Date.now()): ItemPreview | undefined {
  const k = kept.get(keyOf(kind, key));
  return k && now - k.at < KEEP_MS ? k.value : undefined;
}

/** The item: the one read less than a minute ago (or on its way), else read now. Never fails: one that can't be read is
 *  `missing`. */
export function loadPreview(kind: ItemKind, key: string, now = Date.now()): Promise<ItemPreview> {
  const k = keyOf(kind, key);
  const old = kept.get(k);
  if (old && now - old.at < KEEP_MS) return old.answer;
  for (const [x, v] of kept) if (now - v.at >= KEEP_MS) kept.delete(x);
  const entry: Kept = { at: now, answer: read(kind, key).catch(() => MISSING) };
  void entry.answer.then((v) => { entry.value = v; });
  kept.set(k, entry);
  return entry.answer;
}

/** Forgets every item read (for tests). */
export function forgetPreviews(): void {
  kept.clear();
}

/** A read's answer, or a failure when there is none (the browser mode answers null, or "" for an id, without a fixture). */
function found<T>(v: T | null | undefined | ""): T {
  if (v === null || v === undefined || v === "") throw new Error("not found");
  return v;
}

async function read(kind: ItemKind, key: string): Promise<ItemPreview> {
  switch (kind) {
    case "task": return { kind, task: found(await getTask(found(await itemId("task", key)))) };
    case "project": return { kind, project: found(await getProject(found(await itemId("project", key)))) };
    case "client": return { kind, client: found(await getClient(key)) };
    case "person": return { kind, person: found((await listUsers()).find((p) => p.id === key)) };
    case "doc": {
      const doc = found(await getDoc(key));
      const project = doc.projectId ? await getProject(doc.projectId).then((p) => p ?? undefined, () => undefined) : undefined;
      return { kind, doc, project };
    }
    case "agent": {
      const [agent, runs] = await Promise.all([getAgent(key), liveRuns().catch(() => [])]);
      const run = (runs ?? []).find((r) => r.agentId === key);
      const workingOn = run ? await getTask(run.taskId).then((t) => t?.identifier || undefined, () => undefined) : undefined;
      return { kind, agent: found(agent), workingOn };
    }
  }
}

// ---- Where it shows ------------------------------------------------------------------------------------------------

/** The card's width, the gap between the chip and the card, and the room it keeps from the window's edges. */
export const CARD_WIDTH = 300;
export const CARD_GAP = 6;
export const CARD_EDGE = 8;

type Box = { left: number; top: number; bottom: number };
type Size = { width: number; height: number };

/** The card's top-left corner, for the chip's line `at` (the line of a wrapping chip the pointer is on), the card's size and
 *  the window's. Below the chip, 6px under it, its left edge on the chip's; above it when the card doesn't fit below
 *  (6px gap, its height, 8px from the window's edge) and the room above is larger. Never past the window's edges: 8px
 *  inside left and right, and top and bottom when it fits neither below nor above. */
export function cardPlace(at: Box, card: Size, win: Size): { left: number; top: number } {
  const width = Math.min(card.width, win.width - 2 * CARD_EDGE);
  const left = Math.max(CARD_EDGE, Math.min(at.left, win.width - CARD_EDGE - width));
  const below = win.height - at.bottom;
  const above = at.top;
  const up = below < CARD_GAP + card.height + CARD_EDGE && above > below;
  const top = up ? at.top - CARD_GAP - card.height : at.bottom + CARD_GAP;
  return { left, top: Math.max(CARD_EDGE, Math.min(top, win.height - CARD_EDGE - card.height)) };
}
