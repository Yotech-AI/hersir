// The Card kind of the Chat's side island (GA-297; design/islands-chat → Card, Figma frames 1, 7 and 8): a card for reading
// and checking quickly beside the chat. Its tab (square-kanban, the card's id; the tooltip its id and title), and under
// the island's header:
// - the head (status glyph with the hold, the id, the project), the title;
// - in Review only, first: the pull request (PullPanel, as on the card's page) and QA's verdict (its newest QA run's badge,
//   number and time, and the QA Agent's hand-over; a click opens the run's comment on the card's page);
// - Column, Hold with its reason (its end, then Read in comments), Assignee, Labels;
// - the description folded after 4 lines, the acceptance criteria (the first 3, then Show all), the latest 2 comments (each
//   folded after 3 lines, then Show all), the latest 3 runs (a click opens the run on the card's page);
// - a foot: Comment (the foot becomes the comment box), Run or Continue where the card's page allows it (not in Review or
//   Done), and Move to (the board's columns). Own page is the island header's.
// It reloads as the card's page does (useData: a write to the card, its comments or its runs). A card that is gone, or
// archived, keeps its tab, which says so.
import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { Check, ChevronDown, CircleCheck, CircleX, MessageSquare, Play, SquareKanban, StepForward, X } from "lucide-react";
import {
  addComment, cardChanges, continueRun, getProject, getTask, getTeam, itemId, listComments, listRuns, listTasks, moveTask, onRunsChanged, startRun,
  suggestAgent,
} from "../../api";
import type { CardChanges, Comment, Run, Task, Team } from "../../types";
import { useData } from "../../lib/useData";
import { useLiveRuns } from "../../lib/useLiveRuns";
import { usePending } from "../../lib/usePending";
import { relTime, textEnd } from "../../lib/format";
import { modKey } from "../../lib/keys";
import { holdName } from "../../lib/holds";
import { keyBetween } from "../../lib/sortKey";
import { badgeOf, canContinue } from "../../lib/runs";
import { answerOf } from "../../lib/screenshots";
import { keptPreview, loadPreview } from "../../lib/itemPreview";
import { showSideTab, tabKey } from "../../lib/chatSide";
import { stepTab } from "../../lib/editor";
import {
  COMMENT_LINES, COMMENTS_SHOWN, CRITERIA_SHOWN, DESCRIPTION_LINES, REASON_CHARS, RUNS_SHOWN, criteriaCount, criteriaOf, footRun, qaVerdict, runLine,
  runNumber, showAll,
} from "../../lib/chatCard";
import { openCommentOnPage, openRunOnPage } from "../../lib/cardPage";
import { Avatar } from "../Avatar";
import { BusyButton } from "../BusyButton";
import { MarkdownEditor } from "../MarkdownEditor";
import { MarkdownView } from "../MarkdownView";
import { Popover } from "../Popover";
import { PullPanel } from "../PullPanel";
import { StatusIcon } from "../StatusIcon";

// ---------- the tab ----------

/** The card's tab in the island's header: square-kanban and its id; the tooltip its id and title. ← and → go to the tab
 *  beside it (of any kind), middle click and × close it. */
export function CardTab({ id, on, keys, close }: { id: string; on: boolean; keys: string[]; close: () => void }) {
  const key = tabKey({ kind: "card", id });
  const [title, setTitle] = useState(() => titleOf(keptPreview("task", id)));
  useEffect(() => {
    let alive = true;
    loadPreview("task", id).then((p) => { if (alive) setTitle(titleOf(p)); }, () => {});
    return () => { alive = false; };
  }, [id]);
  return (
    <div className="ed-tab card-tab" role="tab" data-tab={key} tabIndex={on ? 0 : -1} aria-selected={on} title={title ? `${id}: ${title}` : id}
      onClick={() => showSideTab(key)}
      onAuxClick={(e) => { if (e.button === 1) { e.preventDefault(); close(); } }}
      onMouseDown={(e) => { if (e.button === 1) e.preventDefault(); }}
      onKeyDown={(e) => {
        if (e.key !== "ArrowRight" && e.key !== "ArrowLeft") return;
        e.preventDefault();
        const next = stepTab(keys, key, e.key === "ArrowRight" ? 1 : -1);
        if (!next) return;
        showSideTab(next);
        (e.currentTarget.parentElement?.querySelector(`[data-tab="${CSS.escape(next)}"], [data-term="${CSS.escape(next.slice(next.indexOf(":") + 1))}"]`) as HTMLElement | null)?.focus();
      }}
      ref={(el) => { if (el && on) el.scrollIntoView({ block: "nearest", inline: "nearest" }); }}>
      <SquareKanban className="icon" />
      <span className="name">{id}</span>
      <button className="x" aria-label={`Close ${id}`} title="Close (middle click)" tabIndex={-1}
        onClick={(e) => { e.stopPropagation(); close(); }}><X className="icon" /></button>
    </div>
  );
}
const titleOf = (p: Awaited<ReturnType<typeof loadPreview>> | undefined) => (p?.kind === "task" ? p.task.title : null);

// ---------- the card ----------

/** The card's own id from its id (GA-259): undefined while asked, null when there is no such card. */
function useCardId(identifier: string): string | null | undefined {
  const [found, setFound] = useState<{ for: string; id: string | null }>();
  useEffect(() => {
    let alive = true;
    itemId("task", identifier).then((id) => { if (alive) setFound({ for: identifier, id }); }, () => { if (alive) setFound({ for: identifier, id: null }); });
    return () => { alive = false; };
  }, [identifier]);
  return found?.for === identifier ? found.id : undefined;
}

/** The shown Card tab's body (`identifier`: GA-259). */
export function ChatCardBody({ identifier }: { identifier: string }) {
  const id = useCardId(identifier);
  if (id === null) return <Gone text="This card is gone." />;
  if (!id) return <div className="chat-side-body" />;
  return <CardView key={id} id={id} />;
}

const Gone = ({ text }: { text: string }) => <div className="chat-side-body chat-card-gone"><p className="t-body">{text}</p></div>;

/** A card's changes, for the pull request's counts (in Review only): when it shows, and again when a run starts or ends. */
function useChanges(id: string, on: boolean): CardChanges | null {
  const [data, setData] = useState<CardChanges | null>(null);
  useEffect(() => {
    if (!on) { setData(null); return; }
    let alive = true, un: (() => void) | undefined;
    const load = () => { cardChanges(id).then((d) => { if (alive) setData(d); }, () => { if (alive) setData(null); }); };
    load();
    onRunsChanged(() => alive && load()).then((f) => (alive ? (un = f) : f())).catch(() => {});
    return () => { alive = false; un?.(); };
  }, [id, on]);
  return data;
}

/** Comments being written in a Card tab, by card: one stays while another tab shows. */
const drafts = new Map<string, string>();

function CardView({ id }: { id: string }) {
  const { data: task, error } = useData(() => getTask(id), [id]);
  const { data: comments } = useData(() => listComments(id), [id]);
  const { data: runs } = useData(() => listRuns(id), [id]);
  const { data: project } = useData(() => (task?.projectId ? getProject(task.projectId) : Promise.resolve(null)), [task?.projectId], { counts: false });
  // its project's team (a card without a project: the first team's), for its columns and the agents that run it
  const { data: team } = useData(() => getTeam(project?.teamId ?? null), [project?.teamId ?? ""]);
  const live = useLiveRuns().find((r) => r.taskId === id) ?? null;
  const review = task?.stateCategory === "review";
  const changes = useChanges(id, !!review && !task?.archivedAt);
  const [allCriteria, setAllCriteria] = useState(false);
  const [allComments, setAllComments] = useState(false);
  const [readReason, setReadReason] = useState(0);
  const bodyRef = useRef<HTMLDivElement>(null);

  // Read in comments: every comment unfolds, and the latest one that holds the reason (an agent's summary is posted as one)
  // scrolls into sight, else the comments
  useEffect(() => {
    if (!readReason) return;
    const reason = task?.holdReason?.trim();
    const c = reason ? [...(comments ?? [])].reverse().find((x) => x.bodyMd.includes(reason)) : undefined;
    const box = bodyRef.current;
    (box?.querySelector(c ? `[data-comment="${CSS.escape(c.id)}"]` : ".chat-card-comments") as HTMLElement | null)?.scrollIntoView({ block: "start" });
  }, [readReason]); // eslint-disable-line react-hooks/exhaustive-deps

  if (error) return <Gone text="This card is gone." />;
  if (!task) return <div className="chat-side-body" />;
  if (task.archivedAt) return <Gone text="This card was archived." />;

  const criteria = criteriaOf(task.acceptanceMd);
  const acceptance = (task.acceptanceMd ?? "").trim();
  const list = comments ?? [];
  const shownComments = allComments ? list : list.slice(-COMMENTS_SHOWN);
  const runList = runs ?? [];
  const verdict = review ? qaVerdict(runList) : null;
  const reasonEnd = task.holdReason ? textEnd(task.holdReason, REASON_CHARS) : null;
  const columns = [...(team?.states ?? [])].sort((a, b) => (a.sortKey < b.sortKey ? -1 : 1));
  const mergeTo = (columns.find((s) => s.category === "deploy") ?? columns.find((s) => s.category === "done"))?.name;

  return (
    <div className="chat-card">
      <div className="chat-card-body" ref={bodyRef}>
        <div className="chat-card-head t-small">
          <StatusIcon category={task.stateCategory} hold={task.hold} title={task.hold ? "On hold" : task.stateName} />
          <span className="t-id id">{task.identifier}</span>
          {task.projectName && <span className="proj"><span className="dot" style={{ background: task.projectColor ?? "var(--text-3)" }} />{task.projectName}</span>}
          {live && <span className="badge live"><span className="pulse" />Live</span>}
        </div>
        <h2 className="t-section chat-card-title">{task.title}</h2>

        {review && <PullPanel task={task} live={!!live} mergeTo={mergeTo} changes={changes} />}
        {verdict && <Verdict run={verdict} n={runNumber(runList, verdict.id)} comment={list.find((c) => c.runId === verdict.id) ?? null} taskId={id} />}

        <dl className="chat-card-props t-small">
          <div><dt>Column</dt><dd><StatusIcon category={task.stateCategory} />{task.stateName}</dd></div>
          {task.hold && (
            <div><dt>Hold</dt><dd className="hold">
              {task.withLead ? <span className="badge live" title="The Chief answers it, or asks you in the Inbox">With the Chief</span>
                : <span className="badge needs">{holdName(task.hold)}</span>}
              {task.holdReason && <span className="reason" title={reasonEnd ? task.holdReason : undefined}>{reasonEnd ?? task.holdReason}</span>}
              {reasonEnd && <button type="button" className="link" onClick={() => { setAllComments(true); setReadReason((n) => n + 1); }}>Read in comments</button>}
            </dd></div>
          )}
          <div><dt>Assignee</dt><dd>{task.assigneeName
            ? <><Avatar name={task.assigneeName} kind={task.assigneeKind} size="sm" />{task.assigneeName}</>
            : <span className="none">Not assigned</span>}</dd></div>
          {task.labels.length > 0 && (
            <div><dt>Labels</dt><dd className="labels">{task.labels.map((l) => (
              <span key={l.id} className="label-pill"><span className="dot" style={{ background: l.color ?? "var(--text-3)" }} />{l.name}</span>
            ))}</dd></div>
          )}
        </dl>

        <section className="chat-card-sec" aria-label="Description">
          <h3 className="t-group">Description</h3>
          {task.descriptionMd.trim() ? <Fold lines={DESCRIPTION_LINES}><MarkdownView md={task.descriptionMd} /></Fold> : <p className="t-body none">None.</p>}
        </section>

        {acceptance && (
          <section className="chat-card-sec" aria-label="Acceptance criteria">
            <h3 className="t-group">Acceptance criteria{criteria && <span className="n">{criteriaCount(criteria)}</span>}</h3>
            {criteria ? <>
              <ul className="chat-card-criteria t-body">
                {(allCriteria ? criteria : criteria.slice(0, CRITERIA_SHOWN)).map((c, i) => (
                  <li key={i} className={c.done ? "done" : undefined}>
                    <span className="box" role="img" aria-label={c.done ? "Done" : "Not done"}>{c.done && <Check className="icon" />}</span>
                    <MarkdownView md={c.text} />
                  </li>
                ))}
              </ul>
              {criteria.length > CRITERIA_SHOWN && (
                <button type="button" className="link" onClick={() => setAllCriteria((a) => !a)}>{allCriteria ? "Show less" : showAll(criteria.length)}</button>
              )}
            </> : <Fold lines={DESCRIPTION_LINES}><MarkdownView md={acceptance} /></Fold>}
          </section>
        )}

        <section className="chat-card-sec chat-card-comments" aria-label="Comments">
          <h3 className="t-group">Comments{list.length > 0 && <span className="n">{list.length}</span>}</h3>
          {comments && list.length === 0 && <p className="t-small none">No comments yet.</p>}
          {shownComments.map((c) => <CardComment key={c.id} c={c} />)}
          {list.length > COMMENTS_SHOWN && (
            <button type="button" className="link" onClick={() => setAllComments((a) => !a)}>{allComments ? "Show less" : showAll(list.length)}</button>
          )}
        </section>

        <section className="chat-card-sec" aria-label="Runs">
          <h3 className="t-group">Runs{runList.length > 0 && <span className="n">{runList.length}</span>}</h3>
          {runs && runList.length === 0 && <p className="t-small none">No runs yet.</p>}
          {runList.slice(0, RUNS_SHOWN).map((r) => <RunLine key={r.id} r={r} n={runNumber(runList, r.id)} taskId={id} />)}
        </section>
      </div>
      {team ? <CardFoot task={task} team={team} runs={runList} live={!!live} columns={columns} />
        : <div className="chat-card-foot" />}
    </div>
  );
}

/** Text folded after `lines` lines (t-body's 20px each), with Show more and Show less while it is longer. */
function Fold({ lines, children }: { lines: number; children: ReactNode }) {
  const ref = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  const [over, setOver] = useState(false);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el || open) return;
    const check = () => setOver(el.scrollHeight > el.clientHeight + 1);
    check();
    if (typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(check);
    ro.observe(el);
    return () => ro.disconnect();
  }, [open, children]);
  return (
    <>
      <div ref={ref} className={`fold${open ? " open" : ""}`} style={{ "--fold-lines": lines } as React.CSSProperties}>{children}</div>
      {(open || over) && <button type="button" className="link" aria-expanded={open} onClick={() => setOpen((o) => !o)}>{open ? "Show less" : "Show more"}</button>}
    </>
  );
}

function CardComment({ c }: { c: Comment }) {
  return (
    <div className="chat-card-comment" data-comment={c.id}>
      <Avatar name={c.authorName} kind={c.authorKind} size="sm" />
      <div className="body">
        <div className="by"><b className="t-body-strong">{c.authorName}</b><span className="t-small when" title={new Date(c.createdAt).toLocaleString("en-GB")}>{relTime(c.createdAt)}</span></div>
        <Fold lines={COMMENT_LINES}><MarkdownView md={c.bodyMd} /></Fold>
      </div>
    </div>
  );
}

/** A run's badge: teal while it works, then its answer (answerOf: QA passed, Needs a decision, …). */
function runBadge(r: Run) {
  const ended = (["succeeded", "failed", "cancelled", "timed_out"] as const).find((s) => s === r.status);
  const b = ended ? answerOf({ outcome: r.outcome ?? null, status: ended }) : badgeOf(r);
  return <span className={`badge ${b.cls}`}>{b.text}</span>;
}

/** One of the latest runs: Run 2, the agent, its badge and when; a click opens it on the card's page. */
function RunLine({ r, n, taskId }: { r: Run; n: number; taskId: string }) {
  const at = r.endedAt ?? r.createdAt;
  return (
    <button type="button" className="chat-card-run t-small" title={`Open run ${n} on the card's page`} onClick={() => openRunOnPage(taskId, r.id)}>
      <span className="n">Run {n}</span><span className="agent">{r.agentName}</span>{runBadge(r)}
      <span className="when" title={new Date(at).toLocaleString("en-GB")}>{relTime(at)}</span>
    </button>
  );
}

/** QA's verdict on a card in Review: the run's badge, its number and when, and the QA Agent's hand-over (3 lines); a click
 *  on the hand-over opens the run's comment on the card's page (the run itself when it has none). */
function Verdict({ run, n, comment, taskId }: { run: Run; n: number; comment: Comment | null; taskId: string }) {
  const passed = run.outcome === "qa_pass";
  const Icon = passed ? CircleCheck : CircleX;
  const text = run.summaryMd ?? comment?.bodyMd ?? "";
  const open = () => (comment ? openCommentOnPage(taskId, comment.id) : openRunOnPage(taskId, run.id));
  return (
    <section className="run-card chat-card-verdict" aria-label="QA's verdict">
      <div className="run-head">
        <Icon className={`icon ${passed ? "ok" : "fail"}`} /><span className="title">QA's verdict</span>{runBadge(run)}
        <span className="right">{runLine(n, relTime(run.endedAt ?? run.createdAt))}</span>
      </div>
      {text.trim() && (
        <div className="run-summary handover" role="link" tabIndex={0} title={comment ? "Open its comment on the card's page" : "Open the run on the card's page"}
          onClick={open} onKeyDown={(e) => { if (e.key === "Enter") { e.preventDefault(); open(); } }}>
          <MarkdownView md={text} />
        </div>
      )}
    </section>
  );
}

// ---------- the foot ----------

/** Comment, Run or Continue, and Move to; with Comment the foot is the comment box until it posts or Cancel. */
function CardFoot({ task, team, runs, live, columns }: { task: Task; team: Team; runs: Run[]; live: boolean; columns: Team["states"] }) {
  const [writing, setWriting] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [suggested, setSuggested] = useState<string | null>(null);
  useEffect(() => { suggestAgent(task.id).then(setSuggested, () => setSuggested(null)); }, [task.id, task.stateId, task.assigneeId]);
  useEffect(() => { setErr(null); }, [task.id]);
  const agents = team.members.filter((m) => m.kind === "agent" && m.status === "active");
  const suggestedName = agents.find((a) => a.actorId === suggested)?.name;
  const last = runs[0];
  const continuable = !!last && canContinue(last) && agents.length > 0;
  const action = footRun({ category: task.stateCategory, held: !!task.hold, archived: !!task.archivedAt, live, continuable, canRun: !!suggestedName });
  // Run and Continue spin until the live run shows, or the run ended (it can end before it shows live)
  const pending = usePending<"start" | "resume">((_, runId) => live || runs.some((r) => r.id === runId && r.endedAt != null), task.id);
  const fail = (e: unknown) => setErr(String(e).replace(/^Error:\s*/, ""));
  const start = () => { setErr(null); pending.act("start", () => startRun(task.id, null), fail); };
  const resume = () => { if (last) { setErr(null); pending.act("resume", () => continueRun(last.id), fail); } };
  const move = async (stateId: string) => {
    setErr(null);
    try {
      // to the bottom of the column, as the card's Properties do
      const others = (await listTasks({ projectId: task.projectId })).filter((t) => t.stateId === stateId && t.id !== task.id);
      await moveTask(task.id, stateId, keyBetween(others.map((t) => t.sortKey).sort().pop() ?? null, null));
    } catch (e) { fail(e); }
  };

  if (writing) return <CardComposer taskId={task.id} done={() => setWriting(false)} />;
  return (
    <div className="chat-card-foot">
      {err && <p className="err t-small" role="alert">{err}</p>}
      <div className="row">
        <button type="button" className="btn sm" onClick={() => setWriting(true)}><MessageSquare className="icon" />Comment</button>
        {action === "continue" && (
          <BusyButton className="btn sm primary" pending={pending} name="resume" busyLabel="Continuing…" icon={<StepForward className="icon" />} onClick={resume}
            title={`Continue picks up ${last!.agentName}'s session where it stopped${task.hold ? " and clears the hold" : ""}`}>Continue</BusyButton>
        )}
        {action === "run" && (
          <BusyButton className="btn sm primary" pending={pending} name="start" busyLabel="Starting…" icon={<Play className="icon" />} onClick={start}
            title={`Run starts ${suggestedName}`}>Run</BusyButton>
        )}
        <span className="grow" />
        <Popover label="Move to" up align="right" button={(open) => (
          <button type="button" className={`btn sm${open ? " on" : ""}`} aria-label="Move to">Move to<ChevronDown className="icon" /></button>
        )}>
          {(close) => columns.map((s) => (
            <button key={s.id} type="button" className="opt" role="menuitemradio" aria-checked={s.id === task.stateId}
              onClick={() => { close(); if (s.id !== task.stateId) move(s.id); }}>
              <StatusIcon category={s.category} />{s.name}{s.id === task.stateId && <Check className="icon tick" />}
            </button>
          ))}
        </Popover>
      </div>
    </div>
  );
}

/** The comment box in the foot: the text box, then Comment and Cancel. Ctrl+Enter (Cmd+Enter) posts, Escape cancels; once
 *  posted the comment shows under Comments and the box closes. */
function CardComposer({ taskId, done }: { taskId: string; done: () => void }) {
  const [first] = useState(() => drafts.get(taskId) ?? "");
  const [text, setText] = useState(first);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const write = (md: string) => { if (md) drafts.set(taskId, md); else drafts.delete(taskId); setText(md); };
  const post = async (md: string) => {
    if (!md.trim() || busy) return;
    setBusy(true);
    setErr(null);
    try { await addComment(taskId, md); drafts.delete(taskId); done(); }
    catch (e) { setErr(String(e).replace(/^Error:\s*/, "")); setBusy(false); }
  };
  return (
    <div className="chat-card-foot writing">
      {/* no formatting bar in the island's width: Markdown and @ work as typed */}
      <MarkdownEditor value={first} onChange={write} onSave={post} onCancel={done} autoFocus ariaLabel="New comment" minHeight={60} pickerUp toolbar={false}
        placeholder="Write a comment. @ links a task, project, client or agent." />
      {err && <p className="err t-small" role="alert">{err}</p>}
      <div className="row">
        <button type="button" className="btn sm primary" disabled={!text.trim() || busy} onClick={() => post(text)}>Comment</button>
        <button type="button" className="btn sm ghost" onClick={done}>Cancel</button>
        <span className="grow" /><span className="hint t-small">{modKey()}+Enter posts · Esc cancels</span>
      </div>
    </div>
  );
}
