// The chat's project (GA-298; src/lib/chatOpen.ts chatProjectOf): the project of the cards the chat is about, else of the
// cards its answers link. The projects are asked once for the whole Chat page (the Board tab, its tab's title, the answers'
// files and Open a file…), again when the window gets the focus (not more than once in FOCUS_GAP) and when a project is
// written.
import { useSyncExternalStore } from "react";
import { listProjects } from "../../api";
import type { Project } from "../../types";
import { chatProjectOf } from "../../lib/chatOpen";
import { useChatCards, type ChatCards } from "../../lib/chatSide";
import { onWrites } from "../../lib/useData";
import { FOCUS_GAP, onWindowFocus } from "../../lib/windowFocus";

let projects: Project[] | undefined;
let asking = false;
let askedAt = -Infinity;
const subs = new Set<() => void>();
let stops: (() => void)[] = [];

function load(force = false) {
  const now = Date.now();
  if (asking || (!force && projects && now - askedAt <= FOCUS_GAP && now >= askedAt)) return;
  asking = true;
  askedAt = now;
  Promise.resolve().then(listProjects).then(
    (list) => { asking = false; projects = list; subs.forEach((f) => f()); },
    () => { asking = false; if (!projects) { projects = []; subs.forEach((f) => f()); } },
  );
}

const subscribe = (f: () => void) => {
  subs.add(f);
  if (subs.size === 1) {
    stops = [onWindowFocus(() => load()), onWrites((changed) => { if (changed.has("projects")) load(true); })];
    load();
  }
  return () => {
    subs.delete(f);
    if (!subs.size) { stops.forEach((s) => s()); stops = []; }
  };
};
const now = () => projects;

/** Hersir's projects; undefined until they are known. */
export const useProjects = (): Project[] | undefined => useSyncExternalStore(subscribe, now, now);
/** The projects as last known, for code outside React. */
export const projectsNow = (): Project[] | undefined => projects;

/** The chat on show's cards and its project: undefined until the projects are known, null when none of its cards has one
 *  (the team's board). */
export function useChatProject(): { cards: ChatCards; project: Project | null | undefined; projects: Project[] | undefined } {
  const cards = useChatCards();
  const list = useProjects();
  return { cards, projects: list, project: list ? chatProjectOf(cards, list) : undefined };
}
