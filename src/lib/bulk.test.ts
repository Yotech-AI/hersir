// GA-306 QA: the texts of the board's Move all and Archive all (their question and button), the toast after a move or an
// archive of several cards (with the cards the backend skipped), and where moved cards show before the backend answers.
import { describe, expect, it } from "vitest";
import type { Task } from "../types";
import { archivedText, askArchiveAll, askMoveAll, movedText, skippedIds, skippedNote, toBottom } from "./bulk";

const WORKING = "An agent is working on this card";
const ids = (n: number) => Array.from({ length: n }, (_, i) => `GA-${i + 1}`);
const skip = (identifier: string, reason = WORKING) => ({ identifier, reason });

describe("Move all's and Archive all's question", () => {
  it("names the number of cards and both columns, and the button says the number", () => {
    expect(askMoveAll(ids(140), "Deploy", "Done")).toEqual({ question: "Move all 140 cards in Deploy to Done?", go: "Move 140 cards" });
    expect(askArchiveAll(ids(544), "Done")).toEqual({
      question: "Archive all 544 cards in Done? The bin on the Tasks page keeps them, and Restore puts one back.",
      go: "Archive 544 cards",
    });
  });

  it("names the card when there is one", () => {
    expect(askMoveAll(["GA-12"], "Deploy", "Done").question).toBe("Move GA-12 to Done?");
    expect(askArchiveAll(["GA-12"], "Done").question).toBe("Archive GA-12?");
  });

  it("follows the columns' own names", () => {
    expect(askMoveAll(ids(2), "Ship", "Shipped").question).toBe("Move all 2 cards in Ship to Shipped?");
    expect(askArchiveAll(ids(3), "Finished").question).toMatch(/^Archive all 3 cards in Finished\? /);
  });
});

describe("the toast after a move or an archive", () => {
  it("says how many, or names the one card", () => {
    expect(movedText(ids(12), "Done")).toBe("Moved 12 cards to Done");
    expect(movedText(["GA-12"], "Done")).toBe("Moved GA-12 to Done");
    expect(archivedText(ids(12))).toBe("Archived 12 cards");
    expect(archivedText(["GA-12"])).toBe("GA-12 archived");
  });

  it("names a skipped card after the count, with its reason", () => {
    expect(archivedText(ids(682), [skip("GA-303")])).toBe("Archived 682 cards. GA-303 wasn't: An agent is working on this card.");
    expect(movedText(ids(12), "Done", [skip("GA-303")])).toBe("Moved 12 cards to Done. GA-303 wasn't: An agent is working on this card.");
  });

  it("names up to three skipped cards, then says how many more", () => {
    expect(skippedNote([skip("GA-1"), skip("GA-2")])).toBe("GA-1 and GA-2 weren't: An agent is working on this card.");
    expect(skippedNote([skip("GA-1"), skip("GA-2"), skip("GA-3")])).toBe("GA-1, GA-2 and GA-3 weren't: An agent is working on this card.");
    const seven = ids(7).map((x) => skip(x));
    const note = skippedNote(seven);
    expect(note).toContain("GA-1, GA-2, GA-3 and 4 more");
    expect(note).not.toContain("GA-4");
    expect(archivedText(ids(10), seven)).toBe("Archived 10 cards. GA-1, GA-2, GA-3 and 4 more weren't: An agent is working on this card.");
  });

  it("gives each card its own reason when they differ, and never more than three", () => {
    const mixed = [skip("GA-303"), skip("GA-12", "Only a card in Done can be archived, and GA-12 is in Review"), skip("GA-13"), skip("GA-14"), skip("GA-15")];
    const note = skippedNote(mixed);
    expect(note).toContain("GA-303 wasn't: An agent is working on this card");
    expect(note).toContain("GA-12 wasn't: Only a card in Done can be archived, and GA-12 is in Review");
    expect(note).toContain("and 2 more");
    expect(note).not.toContain("GA-14");
    expect(note.endsWith(".")).toBe(true);
    expect(note).not.toContain("..");
  });

  it("says nothing of skipped cards when none were", () => {
    expect(skippedNote([])).toBe("");
    expect(movedText(ids(2), "Done", [])).toBe("Moved 2 cards to Done");
  });
});

const card = (n: number, stateId: string, sortKey: string): Task => ({
  id: `t${n}`, identifier: `GA-${n}`, projectId: "p1", title: `Card ${n}`, descriptionMd: "", stateId, stateName: stateId === "s-done" ? "Done" : "Deploy",
  stateCategory: stateId === "s-done" ? "done" : "deploy", priority: 0, labels: [], bounceCount: 0, failCount: 0, sortKey, testing: true, createdAt: 0,
  updatedAt: 0, byHand: false, screenshots: null,
} as Task);
const done = { id: "s-done", name: "Done", category: "done" };
const deploy = { id: "s-deploy", name: "Deploy", category: "deploy" };
/** The column's ids top to bottom, as the board orders them (by sort key). */
const column = (ts: Task[], stateId: string) => ts.filter((t) => t.stateId === stateId).sort((a, b) => (a.sortKey < b.sortKey ? -1 : 1)).map((t) => t.id);

describe("where moved cards show before the backend answers", () => {
  const before = [card(1, "s-deploy", "a0"), card(2, "s-deploy", "a1"), card(3, "s-deploy", "a2"), card(4, "s-done", "a0"), card(5, "s-done", "a5")];

  it("at the bottom of the column, in the order given, under the cards already there", () => {
    const after = toBottom(before, ["t3", "t1"], done);
    expect(column(after, "s-done")).toEqual(["t4", "t5", "t3", "t1"]);
    expect(column(after, "s-deploy")).toEqual(["t2"]);
    const moved = after.find((t) => t.id === "t3")!;
    expect([moved.stateId, moved.stateName, moved.stateCategory]).toEqual(["s-done", "Done", "done"]);
    expect(after.find((t) => t.id === "t2")).toBe(before[1]);
  });

  it("Undo puts them back at the bottom of Deploy in their order", () => {
    const moved = toBottom(before, ["t1", "t2", "t3"], done);
    const back = toBottom(moved, ["t1", "t2", "t3"], deploy);
    expect(column(back, "s-deploy")).toEqual(["t1", "t2", "t3"]);
    expect(column(back, "s-done")).toEqual(["t4", "t5"]);
  });

  it("into an empty column", () => {
    expect(column(toBottom(before, ["t2", "t1"], { id: "s-empty", name: "Empty", category: "done" }), "s-empty")).toEqual(["t2", "t1"]);
  });

  it("finds the cards a command skipped by their identifier", () => {
    expect(skippedIds(before, [skip("GA-2"), skip("GA-9")])).toEqual(new Set(["t2"]));
    expect(skippedIds(before, [])).toEqual(new Set());
  });
});
