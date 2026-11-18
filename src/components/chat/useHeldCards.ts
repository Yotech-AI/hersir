// GA-312: the cards a chat's answers put on the waiting-for-OK hold (GA-101), as they are now: only those still on that hold,
// kept live, so one whose hold went (OK, Clear hold on the card, a move by hand, another hold) leaves the list at once.
import { useEffect, useState } from "react";
import { getTask, onRowsChanged } from "../../api";
import { waitsForYourOk } from "../../lib/holds";
import { whenShown } from "../../lib/windowFocus";
import type { Task } from "../../types";

const NONE: ReadonlyMap<string, Task> = new Map();

/** What the list shows of a card, and its hold's time (which hold it is): an answer that shows the same keeps the last one,
 *  so the chat doesn't draw again. */
const shows = (t: Task) => `${t.id}\u0000${t.identifier}\u0000${t.title}\u0000${t.stateName}\u0000${t.holdAt ?? ""}`;
const same = (a: ReadonlyMap<string, Task>, b: ReadonlyMap<string, Task>) =>
  a.size === b.size && [...a].every(([id, t]) => { const u = b.get(id); return !!u && shows(u) === shows(t); });

/** Of the cards `ids`, those on the waiting-for-OK hold now, by id; asked again after each change to a card (while the
 *  window is hidden, once it shows again). A card that is gone or archived is left out. Without cards it asks nothing. */
export function useHeldCards(ids: string[]): ReadonlyMap<string, Task> {
  const key = ids.join(" ");
  const [cards, setCards] = useState<ReadonlyMap<string, Task>>(NONE);
  useEffect(() => {
    if (!key) { setCards(NONE); return; }
    const list = key.split(" ");
    let alive = true, asked = 0;
    const uns: (() => void)[] = [];
    const load = () => {
      const n = ++asked;
      Promise.all(list.map((id) => getTask(id).catch(() => null))).then((found) => {
        // a later ask's answer counts, not an earlier one that came back after it
        if (!alive || n !== asked) return;
        const now = new Map(found.filter((t): t is Task => !!t && !t.archivedAt && waitsForYourOk(t)).map((t) => [t.id, t]));
        setCards((last) => (same(last, now) ? last : now));
      });
    };
    load();
    const again = whenShown(load);
    onRowsChanged((table) => { if (table === "tasks") again.run(); }).then((f) => (alive ? uns.push(f) : f())).catch(() => {});
    return () => { alive = false; uns.forEach((u) => u()); again.stop(); };
  }, [key]);
  return key ? cards : NONE;
}
