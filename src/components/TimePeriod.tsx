// A project's Time tracking tab (GA-354): the period it shows. The chips sit in the project's island header, right of its
// tabs, as the Usage page's; the tab (ProjectTime) reads the period chosen, and holds the From and To days of Custom under
// them. All is the default; This year, This month and This week (Monday to Sunday) are the whole UTC year, month and week
// of today; Custom, its From and To days (both included), applies once both are set.
import { useState } from "react";
import { TIME_PERIODS, customRange, dayInput, periodRange, type TimePeriodKey, type TimeRange } from "../lib/projectTime";

export type TimePeriod = {
  key: TimePeriodKey;
  /** The period shown: null for All. Custom keeps the last one while its days aren't both set. */
  range: TimeRange | null;
  /** Custom's From and To days, as date inputs hold them ("2026-11-24", or ""). */
  from: string;
  to: string;
  /** Why Custom's days don't make a period yet, or null. */
  problem: string | null;
  choose: (key: TimePeriodKey) => void;
  setDays: (from: string, to: string) => void;
};

export function useTimePeriod(): TimePeriod {
  const [key, setKey] = useState<TimePeriodKey>("all");
  const [range, setRange] = useState<TimeRange | null>(null);
  const [days, setDaysState] = useState({ from: "", to: "" });
  const setDays = (from: string, to: string) => {
    setDaysState({ from, to });
    const c = customRange(from, to);
    if ("range" in c) setRange(c.range);
  };
  const choose = (k: TimePeriodKey) => {
    setKey(k);
    if (k !== "custom") { setRange(periodRange(k, Date.now())); return; }
    // Custom starts from its last days, else from the period shown now (from All: none, until both are picked)
    if (days.from || days.to) setDays(days.from, days.to);
    else if (range) setDays(dayInput(range.since), dayInput(range.until - 1));
  };
  const c = customRange(days.from, days.to);
  return { key, range, from: days.from, to: days.to, problem: key === "custom" && "problem" in c ? c.problem : null, choose, setDays };
}

/** The chips, for the project's island header. */
export function TimePeriodChips({ period }: { period: TimePeriod }) {
  return (
    <div className="toolbar usage-tools time-tools">
      <div className="chips" role="group" aria-label="Period">{TIME_PERIODS.map((p) => (
        <button key={p.key} className={`chip${period.key === p.key ? " on" : ""}`} aria-pressed={period.key === p.key}
          onClick={() => period.choose(p.key)}>{p.label}</button>))}</div>
    </div>
  );
}

/** Custom's From and To days (both included). */
export function CustomDays({ period }: { period: TimePeriod }) {
  const bad = period.problem != null && period.from !== "" && period.to !== "";
  return (
    <span className="time-days">
      <label>From <input type="date" className="input" value={period.from} max={period.to || undefined} placeholder="YYYY-MM-DD"
        aria-invalid={bad || undefined} onChange={(e) => period.setDays(e.target.value, period.to)} /></label>
      <label>To <input type="date" className="input" value={period.to} min={period.from || undefined} placeholder="YYYY-MM-DD"
        aria-invalid={bad || undefined} onChange={(e) => period.setDays(period.from, e.target.value)} /></label>
      {period.problem && <span className={bad ? "time-problem" : "faint"}>{period.problem}</span>}
    </span>
  );
}
