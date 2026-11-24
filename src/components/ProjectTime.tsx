// A project's Time tracking tab (GA-331, GA-354): for the period chosen (TimePeriod: All, This year, This month, This week or
// Custom), the agent time, tokens and API cost at the top as the Usage page's stat cards, a chart of each per day, week or
// month, and per card its runs, tokens, API cost and agent time, so an estimate can be checked against real numbers. A run
// counts in a period when it started in it, on the UTC day it started (gizai-core usage.rs). A run's start and end reload it
// (READS project_time), and the time of a run working now counts up every minute, in the list, the total and the chart.
import { projectTime } from "../api";
import { go } from "../router";
import { useData } from "../lib/useData";
import { useClock } from "../lib/windowFocus";
import { barHeights, formatTokenCount, formatUsageCost, fullCount, knownCost, unknownCostLine, unknownCostNote } from "../lib/usage";
import {
  TIME_COST_NOTE, TIME_NOTE, WORKING_TITLE, agentTimeMs, axisBars, axisLabel, barLabel, cardNumber, columnLabel, formatAgentTime, periodWords,
  runsOnCards, spansYears, timeBars, timeTotals, type BarUnit, type TimeBar, type TimeTotals,
} from "../lib/projectTime";
import type { CardTime, ProjectTime as ProjectTimeData } from "../types";
import { DataTable, type Col } from "./DataTable";
import { CustomDays, type TimePeriod } from "./TimePeriod";

const plural = (n: number, one: string) => `${fullCount(n)} ${one}${n === 1 ? "" : "s"}`;

/** "<n> input (cache included) · <n> output tokens": a token count's title. */
const tokensTitle = (input: number, output: number) => `${fullCount(input)} input (cache included) · ${fullCount(output)} output tokens`;

const faintDash = <span className="faint">—</span>;

/** An agent time, "0m" for none. */
const timeOrZero = (ms: number) => (ms > 0 ? formatAgentTime(ms) : "0m");

/** The API cost of runs, with why part of it is unknown on hover (the Usage page's). */
function Cost({ t }: { t: Pick<TimeTotals, "costUsdMicros" | "unknownCostRuns"> }) {
  return <span className={t.unknownCostRuns > 0 ? "usage-unknown" : undefined} title={unknownCostNote(t.unknownCostRuns)}>{formatUsageCost(t)}</span>;
}

/** Under the chips: the period's days in words, UTC days, and Custom's From and To. */
function PeriodLine({ period, data, now }: { period?: TimePeriod; data: ProjectTimeData; now: number }) {
  const range = data.since != null && data.until != null ? { since: data.since, until: data.until } : null;
  return (
    <div className="time-period">
      {period?.key === "custom" && <CustomDays period={period} />}
      <span className="time-period-words">{periodWords(range, data.firstRun, now)}</span>
      <span className="faint" title="Days, weeks, months and years start at midnight UTC, as on the Usage page">UTC days</span>
    </div>
  );
}

/** Agent time, Tokens and API cost, as the Usage page's stat cards. */
function TimeStats({ t, working }: { t: TimeTotals; working: boolean }) {
  return (
    <div className="stats time-stats">
      <div className="stat-card"><h4>Agent time</h4>
        <div className="t-metric usage-metric card-time">
          {working && <span className="pulse" title={WORKING_TITLE} aria-label={WORKING_TITLE} role="img" />}{timeOrZero(t.timeMs)}</div>
        <span className="sub">{runsOnCards(t)}</span></div>
      <div className="stat-card"><h4>Tokens</h4>
        <div className="t-metric usage-metric" title={tokensTitle(t.inputTokens, t.outputTokens)}>{formatTokenCount(t.inputTokens + t.outputTokens)}</div>
        <span className="sub">{formatTokenCount(t.inputTokens)} input (incl. cache) · {formatTokenCount(t.outputTokens)} output</span></div>
      <div className="stat-card"><h4>API cost</h4>
        <div className="t-metric usage-metric" title={unknownCostNote(t.unknownCostRuns)}>{knownCost(t)}</div>
        {unknownCostLine(t) && <span className="sub usage-unknown" title={unknownCostNote(t.unknownCostRuns)}>{unknownCostLine(t)}</span>}
        <span className="sub">An estimate at API prices, not a bill</span></div>
    </div>
  );
}

/** A bar per day, week or month: the agent time, or the input and output tokens stacked (the Usage page's DayBars). */
function Bars({ bars, unit, show }: { bars: TimeBar[]; unit: BarUnit; show: "time" | "tokens" }) {
  const years = spansYears(bars);
  const heights = barHeights(bars.map((b) => (show === "time" ? b.timeMs : b.inputTokens + b.outputTokens)));
  const tip = (b: TimeBar) => {
    const name = barLabel(b, unit, years);
    if (b.future) return `${name}: not yet`;
    return show === "time"
      ? `${name}: ${timeOrZero(b.timeMs)} · ${plural(b.runs, "run")} · ${formatUsageCost(b)}`
      : `${name}: ${formatTokenCount(b.inputTokens)} input, ${formatTokenCount(b.outputTokens)} output tokens · ${formatUsageCost(b)}`;
  };
  const axis = axisBars(bars);
  return (
    <>
      <div className={`bars usage-bars${bars.length > 40 ? " dense" : ""}`} role="img" aria-label={show === "time" ? `Agent time per ${unit}` : `Tokens per ${unit}`}>
        {bars.map((b, i) => {
          const h = heights[i];
          if (b.future) return <div key={b.start} className="day" title={tip(b)} />;
          if (h === 0) return <div key={b.start} className="day" title={tip(b)}><div className="seg-none" style={{ height: "3%" }} /></div>;
          if (show === "time") return <div key={b.start} className="day" title={tip(b)}><div className="seg-time" style={{ height: `${h}%` }} /></div>;
          const all = b.inputTokens + b.outputTokens;
          return (
            <div key={b.start} className="day" title={tip(b)}>
              <div className="seg-in" style={{ height: `${(h * b.inputTokens) / all}%` }} />
              {b.outputTokens > 0 && <div className="seg-out" style={{ height: `${Math.max(1, (h * b.outputTokens) / all)}%` }} />}
            </div>
          );
        })}
      </div>
      <div className={`axis${axis.length === 1 ? " one" : ""}`}>{axis.map((b) => <span key={b.start}>{axisLabel(b.start, unit, years)}</span>)}</div>
    </>
  );
}

/** Agent time and tokens per day, week or month, the chart's title says which. */
function TimeCharts({ data, t, now }: { data: ProjectTimeData; t: TimeTotals; now: number }) {
  if (data.days.length === 0) return null;
  const running = data.cards.flatMap((c) => (c.runningSince != null ? [c.runningSince] : []));
  const { unit, bars } = timeBars(data.days, running, now);
  return (
    <div className="usage-charts time-charts">
      <div className="stat-card"><h4>Agent time per {unit}</h4><span className="sub">{timeOrZero(t.timeMs)} · {plural(t.runs, "run")}</span>
        <Bars bars={bars} unit={unit} show="time" /></div>
      <div className="stat-card"><h4>Tokens per {unit}</h4><span className="sub">{formatTokenCount(t.inputTokens + t.outputTokens)} tokens</span>
        <Bars bars={bars} unit={unit} show="tokens" />
        <div className="legend"><span><i className="seg-in" />Input (incl. cache)</span><span><i className="seg-out" />Output</span></div></div>
    </div>
  );
}

export function ProjectTime({ projectId, period }: { projectId: string; period?: TimePeriod }) {
  const since = period?.range?.since, until = period?.range?.until;
  const { data, error } = useData(() => projectTime(projectId, since, until), [projectId, since, until]);
  const now = useClock(60_000);
  if (error) return <><div className="error-banner">{error}</div><div className="content" /></>;
  // while it loads, an empty list area (not "No cards yet.")
  if (!data) return <div className="content" />;
  const rows = data.cards;
  const inPeriod = data.since != null;
  const cols: Col<CardTime>[] = [
    { key: "card", header: "Card", sort: (c) => cardNumber(c.identifier),
      cell: (c) => <span title={`${c.identifier} ${c.title}`}><span className="id">{c.identifier}</span> {c.title}</span> },
    { key: "column", header: "Column", width: 140, sort: (c) => columnLabel(c),
      cell: (c) => (c.archived ? <span className="muted">Archived</span> : c.column) },
    { key: "runs", header: "Runs", width: 72, align: "right", sort: (c) => c.runs,
      title: inPeriod ? "Runs that started in the period: each start, Continue and nudge is a run" : "Runs that started: each start, Continue and nudge is a run",
      cell: (c) => (c.runs > 0 ? fullCount(c.runs) : faintDash) },
    { key: "tokens", header: "Tokens", width: 100, align: "right", sort: (c) => c.inputTokens + c.outputTokens, title: "Input (cache included) and output tokens",
      cell: (c) => (c.inputTokens + c.outputTokens > 0
        ? <span title={tokensTitle(c.inputTokens, c.outputTokens)}>{formatTokenCount(c.inputTokens + c.outputTokens)}</span> : faintDash) },
    { key: "cost", header: "API cost", width: 120, align: "right", sort: (c) => c.costUsdMicros,
      title: "What these tokens would cost at API prices: an estimate, not a bill",
      cell: (c) => (c.costUsdMicros > 0 || c.unknownCostRuns > 0 ? <Cost t={c} /> : faintDash) },
    { key: "time", header: "Agent time", width: 124, align: "right", sort: (c) => agentTimeMs(c, now),
      title: inPeriod ? "From each run's start to its end, over the card's runs that started in the period" : "From each run's start to its end, over every run on the card",
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
  const t = timeTotals(rows, now);
  const head = (
    <div className="time-head">
      <PeriodLine period={period} data={data} now={now} />
      <TimeStats t={t} working={rows.some((c) => c.runningSince != null)} />
      <TimeCharts data={data} t={t} now={now} />
    </div>
  );
  // .card-times: a fixed layout, so in a narrow window the Card column gives way and cuts its title (app.css)
  return (
    <div className="card-times">
      <DataTable head={head} rows={rows} columns={cols} rowId={(c) => c.taskId} onRowClick={(c) => go({ page: "task", id: c.taskId })} keyboardNav
        initialSort={[{ id: "card", desc: true }]}
        empty={<div className="empty"><b>{inPeriod ? "No runs in this period." : "No cards yet."}</b></div>}
        footer={<span>{TIME_NOTE} {TIME_COST_NOTE}</span>} />
    </div>
  );
}
