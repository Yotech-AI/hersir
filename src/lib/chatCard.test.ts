// GA-297 QA: the Card kind of the Chief Chat's side island, its pure parts and what opens by itself (src/lib/chatCard.ts,
// src/lib/chatSide.ts's openSideTabs and openCardTab, src/lib/cardPage.ts), against the card and design/islands-chat/README.md:
// - the Card tab's numbers and words: the description folds after 4 lines, a comment after 3, the first 3 criteria with
//   "2 of 6" and "Show all 6", the latest 2 comments and 3 runs, "Run 2 · 1 h ago", the hold's reason 60 characters;
// - the criteria read from a card's checklist, QA's verdict (its newest QA run), a run's number;
// - Run or Continue in the foot as the card's page allows it, never on a card in Review or Done;
// - a chat the Chief started about cards opens them as Card tabs the first time, the first one shown; once per chat, also
//   after a restart (localStorage gizai.chat.sideOpened, the newest 200);
// - a chip's click opens a Card tab or shows the one open (by the card's id, in upper case); Alt+4 shows or hides the cards,
//   and the strip counts them;
// - a run or a comment clicked in the tab opens the card's page there (on Runs for a run, once).
// "A restart" is vi.resetModules with localStorage kept: every module's state goes, as when Hersir starts again.
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { RunOutcome } from "../types";

const f = vi.hoisted(() => ({
  store: new Map<string, string>(),
  went: [] as unknown[],
  revealed: [] as string[],
  broken: false,
}));

vi.mock("react", async (orig) => ({ ...(await orig<typeof import("react")>()), useSyncExternalStore: (_sub: unknown, get: () => unknown) => get() }));
vi.mock("../api", () => ({}));
vi.mock("./terminals", async (orig) => ({
  ...(await orig<typeof import("./terminals")>()),
  terminalsNow: () => [],
  watchTerminals: () => () => {},
}));
vi.mock("../router", async (orig) => ({ ...(await orig<object>()), go: (r: unknown) => { f.went.push(r); } }));
vi.mock("./reveal", () => ({ reveal: (id: string) => { f.revealed.push(id); } }));

vi.stubGlobal("localStorage", {
  getItem: (k: string) => { if (f.broken) throw new Error("no storage"); return f.store.get(k) ?? null; },
  setItem: (k: string, v: string) => { if (f.broken) throw new Error("no storage"); f.store.set(k, String(v)); },
  removeItem: (k: string) => { f.store.delete(k); },
});

let C: typeof import("./chatCard");
let S: typeof import("./chatSide");
let P: typeof import("./cardPage");
/** Hersir starts (again): fresh modules, the same localStorage. */
async function start() {
  vi.resetModules();
  S = await import("./chatSide");
  C = await import("./chatCard");
  P = await import("./cardPage");
}
beforeEach(async () => {
  f.store.clear();
  f.went = [];
  f.revealed = [];
  f.broken = false;
  await start();
});

const keys = (chat?: string) => S.sideNow(chat).tabs.map(S.tabKey);
const kept = () => JSON.parse(f.store.get("gizai.chat.sideOpened") ?? "null") as string[] | null;

describe("what the Card tab shows (design: Card)", () => {
  it("folds and counts as the notes say", () => {
    expect([C.DESCRIPTION_LINES, C.COMMENT_LINES, C.CRITERIA_SHOWN, C.COMMENTS_SHOWN, C.RUNS_SHOWN, C.REASON_CHARS]).toEqual([4, 3, 3, 2, 3, 60]);
    expect(C.showAll(6)).toBe("Show all 6");
    expect(C.showAll(4)).toBe("Show all 4");
    expect(C.runLine(2, "1 h ago")).toBe("Run 2 · 1 h ago");
  });

  it("reads the acceptance criteria from the card's checklist, checked or not, with '2 of 6'", () => {
    const md = [
      "- [x] A Card tab shows the card's id",
      "- [ ] A card in Review shows its pull request",
      "* [X] Comment, Run or Continue",
      "1. [ ] A click on a card chip",
      "2) [x] Chips outside the Chat page",
      "+ [ ] Frame 8 matches in light",
    ].join("\r\n");
    const items = C.criteriaOf(md)!;
    expect(items.map((c) => c.done)).toEqual([true, false, true, false, true, false]);
    expect(items[0]!.text).toBe("A Card tab shows the card's id");
    expect(items[3]!.text).toBe("A click on a card chip");
    expect(C.criteriaCount(items)).toBe("3 of 6");
    expect(C.criteriaCount([{ done: true, text: "a" }, { done: true, text: "b" }, ...Array.from({ length: 4 }, () => ({ done: false, text: "c" }))])).toBe("2 of 6");
  });

  it("joins a line indented under a criterion to it, and an unindented line ends it", () => {
    const items = C.criteriaOf("Intro text\n\n- [ ] Opening a chat\n  the Chief started about GA-259\n\tand GA-283\nNot part of it\n- [x] Alt+4")!;
    expect(items).toEqual([
      { done: false, text: "Opening a chat the Chief started about GA-259 and GA-283" },
      { done: true, text: "Alt+4" },
    ]);
  });

  it("has no criteria for plain text, an ordinary list, nothing or null (the text shows as the description does)", () => {
    expect(C.criteriaOf("The export works.\n- It has a header")).toBeNull();
    expect(C.criteriaOf("")).toBeNull();
    expect(C.criteriaOf(null)).toBeNull();
    expect(C.criteriaOf(undefined)).toBeNull();
  });

  it("numbers runs from the oldest, Run 1, as list_runs gives them newest first", () => {
    const runs = [{ id: "r3" }, { id: "r2" }, { id: "r1" }];
    expect(runs.map((r) => C.runNumber(runs, r.id))).toEqual([3, 2, 1]);
    expect(C.runNumber(runs, "gone")).toBe(0);
  });

  it("takes QA's verdict from the newest run that ended QA passed or QA failed", () => {
    const runs: { id: string; outcome: RunOutcome | null }[] = [
      { id: "r4", outcome: "needs_decision" },
      { id: "r3", outcome: "qa_fail" },
      { id: "r2", outcome: "qa_pass" },
      { id: "r1", outcome: "ready_for_testing" },
    ];
    expect(C.qaVerdict(runs)?.id).toBe("r3");
    expect(C.qaVerdict(runs.slice(2))?.id).toBe("r2");
    expect(C.qaVerdict(runs.slice(3).concat({ id: "r0", outcome: null }))).toBeNull();
    expect(C.qaVerdict([])).toBeNull();
  });
});

describe("Run or Continue in the foot (as the card's page allows it)", () => {
  const card = (o: Partial<Parameters<typeof C.footRun>[0]> = {}) =>
    ({ category: "in_progress", held: false, archived: false, live: false, continuable: false, canRun: true, ...o });

  it("Run when the card isn't held and an agent can run it", () => {
    for (const category of ["backlog", "ready", "in_progress", "testing", "deploy"]) expect(C.footRun(card({ category })), category).toBe("run");
  });
  it("Continue when its last run can go on, also on a held card (it clears the hold)", () => {
    expect(C.footRun(card({ continuable: true }))).toBe("continue");
    expect(C.footRun(card({ continuable: true, held: true, canRun: false }))).toBe("continue");
  });
  it("neither on a card in Review or Done, whatever else is true", () => {
    for (const category of ["review", "done"]) {
      expect(C.footRun(card({ category })), category).toBeNull();
      expect(C.footRun(card({ category, continuable: true })), category).toBeNull();
    }
  });
  it("neither while an agent works on it, on an archived card, on a held card that can't go on, or with no agent to run it", () => {
    expect(C.footRun(card({ live: true, continuable: true }))).toBeNull();
    expect(C.footRun(card({ archived: true, continuable: true }))).toBeNull();
    expect(C.footRun(card({ held: true }))).toBeNull();
    expect(C.footRun(card({ canRun: false }))).toBeNull();
  });
});

describe("a chat the Chief started about cards (design: What opens, and when)", () => {
  it("opens them as Card tabs the first time, the first one shown, the island open, with their ids in upper case", () => {
    expect(C.openChatCardsOnce("chat-1", ["GA-259", "ga-283"])).toBe(true);
    expect(keys("chat-1")).toEqual(["card:GA-259", "card:GA-283"]);
    const side = S.sideNow("chat-1");
    expect(side.current).toBe("card:GA-259");
    expect(side.open).toBe(true);
    expect(S.kindsOpen(side)).toEqual([{ kind: "card", count: 2 }]);
    expect(S.shownKind(side)).toBe("card");
    expect(kept()).toEqual(["chat-1"]);
  });

  it("goes to that chat, not to the chat on show (the page opens it as the chat loads)", () => {
    S.showChat("other");
    C.openChatCardsOnce("chat-1", ["GA-259"]);
    expect(keys("chat-1")).toEqual(["card:GA-259"]);
    expect(keys("other")).toEqual([]);
    expect(S.useChatSide().side.tabs).toEqual([]);
  });

  it("doesn't take the focus from the conversation (a chip's click does)", () => {
    S.showChat("chat-1");
    const before = S.useChatSide().focus;
    C.openChatCardsOnce("chat-1", ["GA-259"]);
    expect(S.useChatSide().focus).toBe(before);
    S.openCardTab("GA-283");
    expect(S.useChatSide().focus).toBe(before + 1);
  });

  it("happens once per chat: closed tabs stay closed when the chat opens again", () => {
    S.showChat("chat-1");
    C.openChatCardsOnce("chat-1", ["GA-259", "GA-283"]);
    S.closeSideTabs(["card:GA-259", "card:GA-283"]);
    expect(keys("chat-1")).toEqual([]);
    S.showChat("chat-2");
    S.showChat("chat-1");
    expect(C.openChatCardsOnce("chat-1", ["GA-259", "GA-283"])).toBe(false);
    expect(keys("chat-1")).toEqual([]);
  });

  it("a hidden island stays hidden, and a closed tab stays closed, when the chat opens again", () => {
    S.showChat("chat-1");
    C.openChatCardsOnce("chat-1", ["GA-259", "GA-283"]);
    S.hideSide();
    S.closeSideTabs(["card:GA-283"]);
    expect(C.openChatCardsOnce("chat-1", ["GA-259", "GA-283"])).toBe(false);
    expect(keys("chat-1")).toEqual(["card:GA-259"]);
    expect(S.sideNow("chat-1").open).toBe(false);
  });

  it("also after a restart: nothing opens again", async () => {
    C.openChatCardsOnce("chat-1", ["GA-259", "GA-283"]);
    await start();
    expect(keys("chat-1")).toEqual([]); // tabs are kept while Hersir runs, not across a restart
    expect(C.openChatCardsOnce("chat-1", ["GA-259", "GA-283"])).toBe(false);
    expect(keys("chat-1")).toEqual([]);
    // another chat about cards still opens its own the first time
    expect(C.openChatCardsOnce("chat-2", ["GA-12"])).toBe(true);
    expect(keys("chat-2")).toEqual(["card:GA-12"]);
    expect(kept()).toEqual(["chat-1", "chat-2"]);
  });

  it("a chat about no cards opens nothing and isn't kept, so cards it gets later still open once", () => {
    for (const none of [[], null, undefined, ["", "  "]]) expect(C.openChatCardsOnce("chat-1", none)).toBe(false);
    expect(keys("chat-1")).toEqual([]);
    expect(kept()).toBeNull();
    expect(C.openChatCardsOnce("chat-1", ["GA-259"])).toBe(true);
  });

  it("opens a card named twice once", () => {
    C.openChatCardsOnce("chat-1", ["GA-259", "ga-259 ", "GA-283", "GA-259"]);
    expect(keys("chat-1")).toEqual(["card:GA-259", "card:GA-283"]);
  });

  it("adds them after tabs the chat has, and shows the first card", () => {
    S.showChat("chat-1");
    S.openCardTab("GA-1");
    S.openSideTab({ kind: "terminal", id: "t1" });
    C.openChatCardsOnce("chat-1", ["GA-259", "GA-1"]);
    expect(keys("chat-1")).toEqual(["card:GA-1", "terminal:t1", "card:GA-259"]);
    expect(S.sideNow("chat-1").current).toBe("card:GA-259");
  });

  it("keeps the newest 200 chats, the one opened last at the end", () => {
    const many = Array.from({ length: 200 }, (_, i) => `c${i}`);
    f.store.set("gizai.chat.sideOpened", JSON.stringify(many));
    expect(C.openChatCardsOnce("new-one", ["GA-1"])).toBe(true);
    const list = kept()!;
    expect(list).toHaveLength(200);
    expect(list[0]).toBe("c1");
    expect(list[199]).toBe("new-one");
    expect(C.openChatCardsOnce("c0", ["GA-1"])).toBe(true); // the oldest was let go, so it counts as new again
    expect(C.withOpenedChat(["a", "b", "c"], "a")).toEqual(["b", "c", "a"]);
    expect(C.withOpenedChat(["a", "b", "c"], "d", 2)).toEqual(["c", "d"]);
    expect(C.SIDE_OPENED_KEY).toBe("gizai.chat.sideOpened");
    expect(C.SIDE_OPENED_MAX).toBe(200);
  });

  it("reads nothing kept, or something unreadable, as none", () => {
    expect(C.openedChats(null)).toEqual([]);
    expect(C.openedChats("not json")).toEqual([]);
    expect(C.openedChats('{"a":1}')).toEqual([]);
    expect(C.openedChats('["a", 2, null, "b"]')).toEqual(["a", "b"]);
    f.store.set("gizai.chat.sideOpened", "{broken");
    expect(C.openChatCardsOnce("chat-1", ["GA-259"])).toBe(true);
    expect(kept()).toEqual(["chat-1"]);
  });

  it("still opens the cards when localStorage can't be used", () => {
    f.broken = true;
    expect(C.openChatCardsOnce("chat-1", ["GA-259"])).toBe(true);
    expect(keys("chat-1")).toEqual(["card:GA-259"]);
  });
});

describe("a card chip's click on the Chat page (openCardTab)", () => {
  it("opens the card in a Card tab at the end, shown, with the focus; its id in upper case", () => {
    S.showChat("chat-1");
    S.openSideTab({ kind: "terminal", id: "t1" });
    const before = S.useChatSide().focus;
    S.openCardTab(" ga-12 ");
    expect(keys("chat-1")).toEqual(["terminal:t1", "card:GA-12"]);
    const snap = S.useChatSide();
    expect(snap.side.current).toBe("card:GA-12");
    expect(snap.side.open).toBe(true);
    expect(snap.focus).toBe(before + 1);
    expect(S.cardTabId(" ga-12 ")).toBe("GA-12");
  });

  it("shows its tab when it is open already, in its place, also with the island hidden", () => {
    S.showChat("chat-1");
    S.openCardTab("GA-12");
    S.openCardTab("GA-13");
    S.hideSide();
    S.openCardTab("ga-12");
    expect(keys("chat-1")).toEqual(["card:GA-12", "card:GA-13"]);
    expect(S.sideNow("chat-1").current).toBe("card:GA-12");
    expect(S.sideNow("chat-1").open).toBe(true);
  });

  it("does nothing for an empty id", () => {
    S.showChat("chat-1");
    S.openCardTab("  ");
    expect(keys("chat-1")).toEqual([]);
  });
});

describe("the strip and Alt+4 with cards open", () => {
  it("counts the cards, and its title names Alt+4 (Option+4 on macOS)", () => {
    S.showChat("chat-1");
    S.openCardTab("GA-259");
    S.openCardTab("GA-283");
    S.openSideTab({ kind: "terminal", id: "t1" });
    expect(S.kindsOpen(S.sideNow())).toEqual([{ kind: "terminal", count: 1 }, { kind: "card", count: 2 }]);
    expect(S.stripTitle("card", 2, false)).toBe("Card, 2 open (Alt+4)");
    expect(S.stripTitle("card", 2, true)).toBe("Card, 2 open (Option+4)");
    expect(S.stripLabel("card", 3)).toBe("Card, 3 open");
    expect(S.stripMenuHead("card", 3)).toBe("Card · 3 open");
  });

  it("Alt+4 (Digit4, so Option+4's character doesn't matter) shows the last shown card, and hides the island while a card shows", () => {
    expect(S.sideKindKey({ code: "Digit4", altKey: true, ctrlKey: false, shiftKey: false, metaKey: false })).toBe("card");
    S.showChat("chat-1");
    S.openCardTab("GA-259");
    S.openCardTab("GA-283");
    S.showSideTab("card:GA-259");
    S.openSideTab({ kind: "terminal", id: "t1" });
    expect(S.shownKind(S.sideNow())).toBe("terminal");
    expect(S.toggleSideKind("card")).toBe(true);
    expect(S.sideNow().current).toBe("card:GA-259");
    expect(S.toggleSideKind("card")).toBe(true);
    expect(S.sideNow().open).toBe(false);
    expect(S.shownKind(S.sideNow())).toBeNull();
    expect(S.toggleSideKind("card")).toBe(true);
    expect(S.sideNow().open).toBe(true);
    expect(S.sideNow().current).toBe("card:GA-259");
  });

  it("does nothing with no card open", () => {
    S.showChat("chat-1");
    S.openSideTab({ kind: "terminal", id: "t1" });
    expect(S.toggleSideKind("card")).toBe(false);
    expect(S.sideNow().current).toBe("terminal:t1");
  });
});

describe("a run or a comment clicked in the Card tab (cardPage)", () => {
  it("a run opens the card's page on its Runs tab, once, at the run", () => {
    P.openRunOnPage("task-1", "run-9");
    expect(f.went).toEqual([{ page: "task", id: "task-1" }]);
    expect(f.revealed).toEqual(["run-run-9"]);
    expect(P.takeRunsTab("task-2")).toBe(false);
    expect(P.takeRunsTab("task-1")).toBe(true);
    expect(P.takeRunsTab("task-1")).toBe(false);
  });

  it("a comment opens the card's page at the comment, on Comments", () => {
    P.openCommentOnPage("task-1", "c-3");
    expect(f.went).toEqual([{ page: "task", id: "task-1" }]);
    expect(f.revealed).toEqual(["comment-c-3"]);
    expect(P.takeRunsTab("task-1")).toBe(false);
  });
});
