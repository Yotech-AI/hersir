// GA-349: a card in Claude Desktop's chat (App.tsx), with what its page in Hersir shows: its identifier and title, column,
// assignee, labels, priority, the hold with its reason, the description and the acceptance criteria (the app's own
// Markdown), the pull request (opened in the browser through the chat), the last comments, the last runs and, while an
// agent works on it, its live output. Comment, Move to and Run or Stop write with the model's tools (comment_on_task,
// move_task, start_agent_run, stop_agent_run); an answer that is an error shows its text by its button.
import { useEffect, useRef, useState } from "react";
import { ArrowRightLeft, ExternalLink, LoaderCircle, MessageSquare, Play, Square } from "lucide-react";
import { MarkdownView } from "../components/MarkdownView";
import { PriorityIcon, PRIORITY_NAMES } from "../components/StatusIcon";
import { Avatar } from "../components/Avatar";
import { holdName } from "../lib/holds";
import { relTime } from "../lib/format";
import { pullLabel } from "../lib/pulls";
import { modKey } from "../lib/keys";
import { MOVE_TASK } from "./BoardView";
import type { CardData, CardRun } from "./views";

/** The model's tools the card's buttons write with. */
export const COMMENT_ON_TASK = "comment_on_task";
export const START_AGENT_RUN = "start_agent_run";
export const STOP_AGENT_RUN = "stop_agent_run";

type Write = (name: string, args: Record<string, unknown>) => Promise<string | null>;
type Button = "comment" | "move" | "run";

/** A time Hersir gives (UTC, like 2026-11-23T14:05:09Z), as the card page says it: "5 min ago"; empty when there is none. */
function when(at: string | null | undefined): string {
  const ms = at ? Date.parse(at) : NaN;
  return Number.isFinite(ms) ? relTime(ms) : "";
}

/** A word of Hersir's (ready_for_testing), in sentence case for people (Ready for testing). */
function words(s: string): string {
  const t = s.replace(/_/g, " ").trim();
  return t ? t[0].toUpperCase() + t.slice(1) : t;
}

/** A run of the card: its agent, how it went, when, and its summary. */
function RunRow({ run, openLink }: { run: CardRun; openLink: (url: string) => void }) {
  const working = run.status === "running" && !run.ended_at;
  const end = when(run.ended_at);
  return (
    <li className="mcp-run">
      <div className="by">
        <b>{run.agent}</b>
        {working ? <span className="badge live"><span className="pulse" />Working</span> : <span className="badge">{words(run.outcome ?? run.status)}</span>}
        <span className="faint">{working ? `started ${when(run.started_at)}` : end ? `ended ${end}` : when(run.started_at)}</span>
      </div>
      {run.summary && <MarkdownView md={run.summary} openLink={openLink} />}
    </li>
  );
}

/** The live run's output: its last lines, kept scrolled to the end unless you scrolled up to read. */
function LiveOutput({ agent, output }: { agent: string; output: string }) {
  const box = useRef<HTMLPreElement>(null);
  const atEnd = useRef(true);
  useEffect(() => {
    const el = box.current;
    if (el && atEnd.current) el.scrollTop = el.scrollHeight;
  }, [output]);
  return (
    <section className="mcp-block mcp-live">
      <h3><span className="pulse" aria-hidden />{agent || "An agent"} is working</h3>
      <pre ref={box} className="mcp-output" tabIndex={0} aria-label="The live run's output"
        onScroll={(e) => { const el = e.currentTarget; atEnd.current = el.scrollHeight - el.scrollTop - el.clientHeight < 24; }}>
        {output || "No output yet."}
      </pre>
    </section>
  );
}

export function CardView({ card, columns, loadColumns, write, openLink }: {
  card: CardData;
  /** The columns of the card's board, by name, for Move to; null until they are read (`loadColumns`). */
  columns: string[] | null;
  loadColumns: () => void;
  write: Write;
  openLink: (url: string) => void;
}) {
  const [busy, setBusy] = useState<Button | null>(null);
  const [errors, setErrors] = useState<Partial<Record<Button, string>>>({});
  const [composing, setComposing] = useState(false);
  const [text, setText] = useState("");
  const [moving, setMoving] = useState(false);
  const task = card.identifier;

  // Another card: nothing of the last one's stays.
  useEffect(() => {
    setErrors({});
    setComposing(false);
    setMoving(false);
    setText("");
  }, [task]);

  const run = async (button: Button, name: string, args: Record<string, unknown>): Promise<boolean> => {
    setBusy(button);
    setErrors((e) => ({ ...e, [button]: undefined }));
    const error = await write(name, { task, ...args });
    setBusy(null);
    if (error) setErrors((e) => ({ ...e, [button]: error }));
    return !error;
  };

  const post = async () => {
    const body = text.trim();
    if (!body || busy) return;
    if (await run("comment", COMMENT_ON_TASK, { body_md: body })) {
      setText("");
      setComposing(false);
    }
  };

  const openMove = () => {
    setMoving((m) => !m);
    if (!columns) loadColumns();
  };

  const moveTo = async (column: string) => {
    if (await run("move", MOVE_TASK, { column })) setMoving(false);
  };

  const live = card.live;
  const elsewhere = (columns ?? []).filter((c) => c.toLowerCase() !== card.column.toLowerCase());

  return (
    <div className="mcp-card">
      <div className="task-head">
        <PriorityIcon priority={card.priority} />
        <span className="id">{card.identifier}</span>
        {card.project && <span className="faint">{card.project}</span>}
        {live && <span className="badge live"><span className="pulse" />Live</span>}
        {card.hold && <span className="badge needs">On hold</span>}
      </div>
      <h2 className="mcp-card-title">{card.title}</h2>

      <dl className="mcp-props">
        <dt>Column</dt><dd>{card.column}</dd>
        <dt>Assignee</dt>
        <dd>{card.assignee ? <span className="mcp-who"><Avatar name={card.assignee} size="sm" />{card.assignee}</span> : <span className="faint">No one</span>}</dd>
        <dt>Priority</dt><dd>{PRIORITY_NAMES[card.priority] ?? "None"}</dd>
        <dt>Labels</dt>
        <dd className="mcp-labels">{card.labels.length === 0 ? <span className="faint">None</span>
          : card.labels.map((l) => <span key={l.name} className="label-pill"><span className="dot" style={{ background: l.color ?? "var(--text-3)" }} />{l.name}</span>)}</dd>
        {card.pr_url && (<>
          <dt>Pull request</dt>
          <dd><a className="mcp-link" href={card.pr_url} title={card.pr_url}
            onClick={(e) => { e.preventDefault(); openLink(card.pr_url!); }}>{pullLabel(card.pr_url)}<ExternalLink className="icon sm" /></a></dd>
        </>)}
      </dl>

      {card.hold && (
        <div className="mcp-hold" role="note">
          <b>On hold: {holdName(card.hold).toLowerCase()}</b>
          {card.hold_reason ? <MarkdownView md={card.hold_reason} openLink={openLink} /> : <span className="faint">No reason given.</span>}
        </div>
      )}

      <div className="mcp-actions">
        <div className="mcp-action">
          <button className="btn sm" aria-expanded={composing} onClick={() => setComposing((c) => !c)} disabled={busy === "comment"}>
            <MessageSquare className="icon" />Comment
          </button>
        </div>
        <div className="mcp-action">
          <button className="btn sm" aria-expanded={moving} onClick={openMove} disabled={busy === "move"}>
            {busy === "move" ? <LoaderCircle className="icon spin" /> : <ArrowRightLeft className="icon" />}Move to
          </button>
        </div>
        <div className="mcp-action">
          {live ? (
            <button className="btn sm" onClick={() => void run("run", STOP_AGENT_RUN, {})} disabled={busy === "run"}
              title={`Stop ${live.agent || "the agent"}`}>
              {busy === "run" ? <><LoaderCircle className="icon spin" />Stopping…</> : <><Square className="icon" />Stop</>}
            </button>
          ) : (
            <button className="btn sm" onClick={() => void run("run", START_AGENT_RUN, {})} disabled={busy === "run"}
              title="Start the assigned agent, else the column's first">
              {busy === "run" ? <><LoaderCircle className="icon spin" />Starting…</> : <><Play className="icon" />Run</>}
            </button>
          )}
        </div>
      </div>
      {errors.run && <p className="mcp-error" role="alert">{errors.run}</p>}

      {moving && (
        <div className="mcp-move" role="group" aria-label="Move to a column">
          {columns === null ? <span className="faint">Reading the columns…</span>
            : elsewhere.length === 0 ? <span className="faint">No other column to move it to.</span>
            : elsewhere.map((c) => (
              <button key={c} className="btn ghost sm" onClick={() => void moveTo(c)} disabled={busy === "move"}>{c}</button>
            ))}
        </div>
      )}
      {errors.move && <p className="mcp-error" role="alert">{errors.move}</p>}

      {composing && (
        <div className="mcp-compose">
          <textarea className="textarea" aria-label="Your comment" placeholder="Write a comment. Markdown works." value={text} autoFocus
            onChange={(e) => setText(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) { e.preventDefault(); void post(); }
              if (e.key === "Escape") setComposing(false);
            }} />
          <div className="mcp-compose-acts">
            <button className="btn primary sm" onClick={() => void post()} disabled={!text.trim() || busy === "comment"}>
              {busy === "comment" ? <><LoaderCircle className="icon spin" />Posting…</> : "Post comment"}
            </button>
            <button className="btn ghost sm" onClick={() => setComposing(false)}>Cancel</button>
            <span className="faint mcp-hint">{modKey()}+Enter posts</span>
          </div>
        </div>
      )}
      {errors.comment && <p className="mcp-error" role="alert">{errors.comment}</p>}

      <section className="mcp-block">
        <h3>Description</h3>
        {card.description_md.trim() ? <MarkdownView md={card.description_md} openLink={openLink} /> : <p className="faint">None.</p>}
      </section>
      <section className="mcp-block">
        <h3>Acceptance criteria</h3>
        {card.acceptance_md?.trim() ? <MarkdownView md={card.acceptance_md} openLink={openLink} /> : <p className="faint">None.</p>}
      </section>

      {live && <LiveOutput agent={live.agent} output={live.output} />}

      <section className="mcp-block">
        <h3>Comments {card.comments.length > 0 && <span className="n">{card.comments.length}</span>}</h3>
        {card.comments.length === 0 ? <p className="faint">No comments yet.</p> : card.comments.map((c, i) => (
          <div className="comment" key={`${c.at}-${i}`}>
            <Avatar name={c.author} size="lg" />
            <div className="body">
              <div className="by"><b>{c.author}</b><span className="faint" title={c.at}>{when(c.at)}</span></div>
              <MarkdownView md={c.body_md} openLink={openLink} />
            </div>
          </div>
        ))}
      </section>

      <section className="mcp-block">
        <h3>Runs {card.runs.length > 0 && <span className="n">{card.runs.length}</span>}</h3>
        {card.runs.length === 0 ? <p className="faint">No runs yet.</p>
          : <ul className="mcp-runs">{card.runs.map((r, i) => <RunRow key={`${r.started_at}-${i}`} run={r} openLink={openLink} />)}</ul>}
      </section>
    </div>
  );
}
