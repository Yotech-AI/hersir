// A card's hold by name, as the Properties panel and the @ chip's hover card (GA-287) show it.

export const HOLD_NAMES: Record<string, string> = {
  needs_decision: "Needs your decision", stalled: "Stalled", merge_conflict: "Merge conflict",
  waiting_approval: "Waiting for approval", rate_limited: "Rate limited", blocked: "Blocked",
};

/** A hold's name: its key's name, else the key itself. */
export const holdName = (hold: string): string => HOLD_NAMES[hold] ?? hold;
