// A project's Time tracking tab (GA-331): per card, how long the agents worked on it and the tokens that took, added up
// over all of its runs, so an estimate can be checked against real numbers. A run's start and end reload it (READS
// project_time), and a card an agent works on now counts up every minute.
import { projectTime } from "../api";
import { go } from "../router";
import { useData } from "../lib/useData";
import { useClock } from "../lib/windowFocus";
import { formatTokenCount, fullCount } from "../lib/usage";
import { TIME_NOTE, WORKING_TITLE, agentTimeMs, cardNumber, columnLabel, formatAgentTime, timeTotals } from "../lib/projectTime";
import type { CardTime } from "../types";
import { DataTable, type Col } from "./DataTable";

const plural = (n: number, one: string) => `${fullCount(n)} ${one}${n === 1 ? "" : "s"}`;

/** "<n> input (cache included) · <n> output tokens": a token count's title. */
const tokensTitle = (input: number, output: number) => `${fullCount(input)} input (cache included) · ${fullCount(output)} output tokens`;

const faintDash = <span className="faint">—</span>;

export function ProjectTime({ projectId }: { projectId: string }) {
  const { data, error } = useData(() => projectTime(projectId), [projectId]);
  const now = useClock(60_000);
  if (error) return <><div className="error-banner">{error}</div><div className="content" /></>;
  // while it loads, an empty list area (not "No cards yet.")
  if (!data) return <div className="content" />;
  const cols: Col<CardTime>[] = [
    { key: "card", header: "Card", sort: (c) => cardNumber(c.identifier),
      cell: (c) => <span title={`${c.identifier} ${c.title}`}><span className="id">{c.identifier}</span> {c.title}</span> },
    { key: "column", header: "Column", width: 140, sort: (c) => columnLabel(c),
      cell: (c) => (c.archived ? <span className="muted">Archived</span> : c.column) },
    { key: "runs", header: "Runs", width: 72, align: "right", sort: (c) => c.runs, title: "Runs that started: each start, Continue and nudge is a run",
      cell: (c) => (c.runs > 0 ? fullCount(c.runs) : faintDash) },
    { key: "tokens", header: "Tokens", width: 100, align: "right", sort: (c) => c.inputTokens + c.outputTokens, title: "Input (cache included) and output tokens",
      cell: (c) => (c.inputTokens + c.outputTokens > 0
        ? <span title={tokensTitle(c.inputTokens, c.outputTokens)}>{formatTokenCount(c.inputTokens + c.outputTokens)}</span> : faintDash) },
    { key: "time", header: "Agent time", width: 124, align: "right", sort: (c) => agentTimeMs(c, now), title: "From each run's start to its end, over every run on the card",
      cell: (c) => {
        const ms = agentTimeMs(c, now);
        return (
          <span className="card-time">
            {c.runningSince != null && <span className="pulse" title={WORKING_TITLE} aria-label={WORKING_TITLE} role="img" />}
            {ms > 0 ? formatAgentTime(ms) : faintDash}
          </span>
        );
      } },
  ];
  const t = timeTotals(data, now);
  // .card-times: a fixed layout, so in a narrow window the Card column gives way and cuts its title (app.css)
  return (
    <div className="card-times">
      <DataTable rows={data} columns={cols} rowId={(c) => c.taskId} onRowClick={(c) => go({ page: "task", id: c.taskId })} keyboardNav
        initialSort={[{ id: "card", desc: true }]}
        empty={<div className="empty"><b>No cards yet.</b></div>}
        footer={data.length > 0 && <>
          <span className="card-time-sums">{plural(t.cards, "card")} · {plural(t.runs, "run")} · <span title={tokensTitle(t.inputTokens, t.outputTokens)}>{formatTokenCount(t.inputTokens + t.outputTokens)} tokens</span> · {formatAgentTime(t.timeMs)} agent time</span>
          <span>{TIME_NOTE}</span>
        </>} />
    </div>
  );
}
