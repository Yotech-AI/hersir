// Hersir's own log (GA-281, GA-282): what the notice after an unexpected end and Settings → General → Data say about it.

/** The name of a log from its full path: from /…/logs/hersir-2026-11-13T14-50-26Z.log it is hersir-2026-11-13T14-50-26Z.log.
 *  Windows' backslashes count as separators too. */
export function logName(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() ?? path;
}

/** When an ISO 8601 time ("2026-11-13T14:47:12Z") was, in this computer's time zone: { date: "13 Nov", time: "15:47" } in
 *  Amsterdam. Null for a time that doesn't parse. */
export function localWhen(iso: string): { date: string; time: string } | null {
  const at = new Date(iso);
  if (Number.isNaN(at.getTime())) return null;
  return {
    date: at.toLocaleDateString("en-GB", { day: "numeric", month: "short" }),
    time: at.toLocaleTimeString("en-GB", { hour: "2-digit", minute: "2-digit" }),
  };
}

/** The notice's sentence before the file's name: "Hersir ended unexpectedly on 13 Nov at 15:47. Its log can help find out
 *  why: ", from the log's last write (`lastWrittenAt`). Without a time that parses it leaves the date out. */
export function unexpectedEndText(lastWrittenAt: string): string {
  const when = localWhen(lastWrittenAt);
  return `Hersir ended unexpectedly${when ? ` on ${when.date} at ${when.time}` : ""}. Its log can help find out why: `;
}

/** Settings → General → Data → Hersir's logs: its hint, word for word from GA-282. */
export const LOGS_HINT = "One file per start; the 20 newest are kept. If Hersir crashed, send us the file of that start.";
