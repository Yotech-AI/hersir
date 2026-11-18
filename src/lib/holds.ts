// A card's hold by name, as the Properties panel and the @ chip's hover card (GA-287) show it.

export const HOLD_NAMES: Record<string, string> = {
  needs_decision: "Needs your decision", stalled: "Stalled", merge_conflict: "Merge conflict",
  waiting_approval: "Waiting for approval", rate_limited: "Rate limited", blocked: "Blocked",
};

/** A hold's name: its key's name, else the key itself. */
export const holdName = (hold: string): string => HOLD_NAMES[hold] ?? hold;

/** GA-261: the kind of the hold the Chief's change put on a card after a tool from outside Hersir in the chat (GA-101): only
 *  you take it off (`model::HOLD_WAITING_FOR_OK`). */
export const HOLD_WAITING_FOR_OK = "waiting_for_ok";

/** The card waits on that hold for your OK (`Task::waits_for_your_ok`). */
export const waitsForYourOk = (t: { hold?: string | null; holdKind?: string | null }): boolean => !!t.hold && t.holdKind === HOLD_WAITING_FOR_OK;
