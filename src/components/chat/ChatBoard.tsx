// The Board kind of the Chat's side island (GA-298; design/islands-chat → Board, Figma frame 3): the board of the chat's
// project beside the chat (useChatProject: the project of the cards the chat is about, else of the cards its answers link),
// else the team's board (the team picked on the Tasks page, all its projects). A chat has one Board tab; it follows the
// chat's cards. Its tab: columns-3 and the project's name (Board for the team's), the tooltip "Board of Hersir".
// Under the island's header a row: the project's dot and name, "the chat's project", and the legend (the mark, "In this
// chat"); then the board, compact (components/Board.tsx compact): columns 180 wide that scroll sideways, each down; it opens
// scrolled to the column of the chat's first card. The chat's cards are marked. A click on a card opens it in a Card tab
// (or shows its tab), Ctrl+click (Cmd+click on macOS) its page; dragging moves a card between columns as on the Tasks page
// (the same drop rules, saved with moveTask; a move that fails goes back and says why). Archived cards aren't listed, and
// Backlog, Done and Cancelled are rails, as on the board.
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { CircleAlert, Columns3, X } from "lucide-react";
import { getTeam, listTasks, listTeams, moveTask } from "../../api";
import type { Project, Task } from "../../types";
import { go } from "../../router";
import { useData } from "../../lib/useData";
import { useLiveRuns } from "../../lib/useLiveRuns";
import { cardsOfTeam, pickTeam, shownTeam, useCurrentTeam } from "../../lib/team";
import { modClick } from "../../lib/keys";
import { stepTab } from "../../lib/editor";
import { allChatCards, BOARD_TAB, closeSideTabs, openCardTab, showSideTab, tabKey } from "../../lib/chatSide";
import { Board } from "../Board";
import { useChatProject } from "./useChatProject";

const KEY = tabKey({ kind: "board", id: BOARD_TAB });

/** The team whose board shows: the project's team (else the first team), or with no project the team picked on the Tasks
 *  page; and every team, for the cards of the team's board. */
function useBoardTeam(project: Project | null | undefined) {
  const { data: teams } = useData(() => listTeams(), [], { counts: false });
  const [picked] = useCurrentTeam();
  const first = teams?.[0]?.id ?? null;
  const teamId = !teams || project === undefined ? undefined : project ? project.teamId || first : shownTeam(teams, picked);
  return { teams, first, teamId };
}

/** The tab's name: the project's, or Board for the team's board. */
export const boardName = (project: Project | null | undefined) => project?.name ?? "Board";

/** The Board tab in the island's header: columns-3 and the project's name; the tooltip "Board of Hersir". ← and → go to the
 *  tab beside it, middle click and × close it. */
export function BoardTab({ on, keys, close }: { on: boolean; keys: string[]; close: () => void }) {
  const { project } = useChatProject();
  const { teams, teamId } = useBoardTeam(project);
  const of = project?.name ?? teams?.find((t) => t.id === teamId)?.name ?? "the team";
  const name = boardName(project);
  return (
    <div className="ed-tab board-tab" role="tab" data-tab={KEY} tabIndex={on ? 0 : -1} aria-selected={on} title={`Board of ${of}`}
      onClick={() => showSideTab(KEY)}
      onAuxClick={(e) => { if (e.button === 1) { e.preventDefault(); close(); } }}
      onMouseDown={(e) => { if (e.button === 1) e.preventDefault(); }}
      onKeyDown={(e) => {
        if (e.key !== "ArrowRight" && e.key !== "ArrowLeft") return;
        e.preventDefault();
        const next = stepTab(keys, KEY, e.key === "ArrowRight" ? 1 : -1);
        if (!next) return;
        showSideTab(next);
        (e.currentTarget.parentElement?.querySelector(`[data-tab="${CSS.escape(next)}"], [data-term="${CSS.escape(next.slice(next.indexOf(":") + 1))}"]`) as HTMLElement | null)?.focus();
      }}
      ref={(el) => { if (el && on) el.scrollIntoView({ block: "nearest", inline: "nearest" }); }}>
      <Columns3 className="icon" />
      <span className="name">{name}</span>
      <button className="x" aria-label={`Close ${name}`} title="Close (middle click)" tabIndex={-1}
        onClick={(e) => { e.stopPropagation(); close(); }}><X className="icon" /></button>
    </div>
  );
}

/** Own page: the board on the Tasks page, of the project at its address, #/board/project/<id> (GA-372; the team's board:
 *  #/board, every project of the team picked); the tab closes here. */
export function boardOwnPage(project: Project | null | undefined) {
  if (project) pickTeam(project.teamId ?? null);
  closeSideTabs([KEY]);
  go(project ? { page: "board", project: project.id } : { page: "board" });
}

/** The shown Board tab's body. */
export function ChatBoardBody() {
  const { cards, project, projects } = useChatProject();
  const { teams, first, teamId } = useBoardTeam(project);
  const { data: team } = useData(() => (teamId === undefined ? Promise.resolve(null) : getTeam(teamId)), [teamId ?? "-"]);
  const { data: fetched } = useData(() => (project === undefined ? Promise.resolve(null) : listTasks(project ? { projectId: project.id } : {})), [project === undefined ? "-" : project?.id ?? ""]);
  const [tasks, setTasks] = useState<Task[]>([]);
  useEffect(() => { if (fetched) setTasks(fetched); }, [fetched]);
  const tasksRef = useRef(tasks);
  tasksRef.current = tasks;
  const [failed, setFailed] = useState<string>();
  const marked = useMemo(() => new Set(allChatCards(cards)), [cards]);

  const states = useMemo(() => (team ? [...team.states].sort((a, c) => (a.sortKey < c.sortKey ? -1 : 1)) : []), [team]);
  const several = (teams?.length ?? 0) > 1;
  const shown = useMemo(() => (project ? tasks.filter((t) => t.projectId === project.id)
    : several && team && projects && first ? cardsOfTeam(tasks, projects, team.id, first) : tasks), [tasks, project, several, team, projects, first]);
  const live = useLiveRuns();
  const members = team?.members;
  const working = useMemo(() => new Map(live.map((r) => [r.taskId, members?.find((m) => m.actorId === r.agentId)?.name ?? "An agent"])), [live, members]);

  const onMove = useCallback(async (id: string, stateId: string, sortKey: string) => {
    const before = tasksRef.current;
    const st = states.find((s) => s.id === stateId);
    setFailed(undefined);
    setTasks((ts) => ts.map((t) => (t.id === id ? { ...t, stateId, sortKey, stateName: st?.name ?? t.stateName, stateCategory: st?.category ?? t.stateCategory } : t)));
    try { await moveTask(id, stateId, sortKey); }
    catch (e) { setTasks(before); setFailed(`Couldn't move ${before.find((x) => x.id === id)?.identifier ?? "the card"}: ${String(e).replace(/^Error:\s*/, "")}`); }
  }, [states]);
  const onOpen = useCallback((id: string, e?: { ctrlKey: boolean; metaKey: boolean }) => {
    const t = tasksRef.current.find((x) => x.id === id);
    if (!t) return;
    if (e && modClick(e)) go({ page: "task", id });
    else openCardTab(t.identifier);
  }, []);

  // it opens scrolled to the column of the chat's first card on it
  const box = useRef<HTMLDivElement>(null);
  const scrolled = useRef(false);
  useEffect(() => {
    if (scrolled.current || !states.length || !shown.length) return;
    scrolled.current = true;
    const firstCard = allChatCards(cards).map((c) => shown.find((t) => t.identifier === c)).find(Boolean);
    const board = box.current?.querySelector<HTMLElement>(".board");
    const col = firstCard && board ? board.querySelector<HTMLElement>(`[data-card="${CSS.escape(firstCard.identifier)}"]`)?.closest<HTMLElement>(".col") : null;
    if (board && col) board.scrollLeft = Math.max(0, col.offsetLeft - board.offsetLeft - 6);
  }, [states.length, shown.length]); // eslint-disable-line react-hooks/exhaustive-deps

  const ready = project !== undefined && !!team && !!fetched;
  return (
    <div className="chat-board" ref={box}>
      <div className="chat-board-head">
        {project ? <>
          <span className="dot" style={{ background: project.color ?? "var(--text-3)" }} aria-hidden />
          <b className="t-label">{project.name}</b>
          <span className="t-small faint-2">the chat's project</span>
        </> : <b className="t-label">{team?.name ?? "Board"}</b>}
        {marked.size > 0 && <span className="legend t-small"><span className="mark" aria-hidden />In this chat</span>}
      </div>
      {ready && states.length > 0 && <Board tasks={shown} states={states} onMove={onMove} onOpen={onOpen} working={working} members={members} compact marked={marked} />}
      {ready && states.length === 0 && <div className="chat-board-empty t-body faint">This board has no columns yet.</div>}
      {failed && (
        <div className="ed-bar file failed chat-board-failed" role="alert">
          <CircleAlert className="icon" /><span className="say">{failed}</span>
          <button type="button" className="btn ghost sm icon-only" aria-label="Close" title="Close" onClick={() => setFailed(undefined)}><X className="icon" /></button>
        </div>
      )}
    </div>
  );
}
