// GA-102: what the Team Lead couldn't do after a tool from outside Gizai, under the answer that tried it: each call with one
// plain line about what it does, a preview, and Approve and Discard; Approve all when there are several. Approve runs exactly
// that call, once, with every other rule in force; Discard runs nothing. Either way a note in the chat says what happened.
// GA-261: a chat stays marked once it used such a tool, so a later answer's calls wait too: the line says "This chat used".
// GA-312: also the cards that answer put on the waiting-for-OK hold (GA-101), while they are on it: each with its column and
// OK, which takes the hold off as Clear hold on the card does. Approve all runs the calls, then takes the cards' holds off.
import { useState } from "react";
import { ArrowRight, Check, CircleAlert, X } from "lucide-react";
import { approvePendingAction, approvePendingActions, discardPendingAction, updateTask } from "../../api";
import { foldDiff, outsideLabel } from "../../lib/pending";
import { usePending } from "../../lib/usePending";
import type { DiffLine, PendingAction, PendingPreview, Task } from "../../types";
import { BusyButton } from "../BusyButton";
import { ItemChip } from "../MarkdownView";

/** What changes against the current version, line by line; long unchanged runs fold until you open them. */
function Diff({ lines }: { lines: DiffLine[] }) {
  const [all, setAll] = useState(false);
  const parts = all ? lines : foldDiff(lines);
  return (
    <div className="pa-diff" aria-label="What changes">
      {parts.map((l, i) => l.kind === "fold"
        ? <button key={i} type="button" className="pa-fold" onClick={() => setAll(true)}>{l.count === 1 ? "1 unchanged line" : `${l.count} unchanged lines`}</button>
        : <div key={i} className={`pa-dl ${l.kind}`}><span className="pa-sign" aria-label={l.kind === "add" ? "added" : l.kind === "del" ? "removed" : undefined}>
            {l.kind === "add" ? "+" : l.kind === "del" ? "−" : ""}</span>{l.text || " "}</div>)}
    </div>
  );
}

/** What the call is about and what changes, the text it saves or adds, and why approving fails as things are now. */
function Preview({ p }: { p: PendingPreview }) {
  return (
    <>
      {p.fields.length > 0 && (
        <dl className="pa-fields">
          {p.fields.map((f, i) => (
            <div key={i}><dt>{f.label}</dt>
              <dd>{f.from != null ? <><span className="pa-from">{f.from}</span><ArrowRight className="icon sm" aria-label="becomes" />{f.value}</> : f.value}</dd></div>
          ))}
        </dl>
      )}
      {p.warning && <div className="pa-warn"><CircleAlert className="icon sm" /><span>{p.warning}</span></div>}
      {p.diff ? <Diff lines={p.diff} /> : p.text ? <pre className="pa-text" aria-label="Text">{p.text}</pre> : null}
    </>
  );
}

/** What Approve all did: the calls it ran, and the cards whose holds it took off. */
type AllDone = { ran: PendingAction[]; cards: string[] };

/** Why these wait: the chat used a tool from outside Hersir, so Hersir didn't run the calls, and no agent starts the cards. */
function why(outside: string, calls: number, cards: number): string {
  const used = `This chat used ${outside ? `${outsideLabel(outside)}, a tool` : "a tool"} from outside Hersir`;
  const held = `no agent starts ${cards === 1 ? "this card until you OK it" : "these cards until you OK them"}`;
  if (cards === 0) return `${used}, so Hersir didn't run ${calls === 1 ? "this" : "these"}.`;
  if (calls === 0) return `${used}, so ${held}.`;
  return `${used}, so Hersir didn't run ${calls === 1 ? "this call" : "these calls"}, and ${held}.`;
}

/** The calls one answer couldn't make, in the order it tried them, and (GA-312) `cards`: those it put on the waiting-for-OK
 *  hold that are on it now, in the order it did. `outsideTool`: the tool from outside Hersir the chat used, for when only
 *  cards wait. `threadId`: the chat; switching to another ends a wait. `onError`: why an Approve, Discard or OK didn't go through. */
export function PendingActions({ actions, cards = [], outsideTool, threadId, onError }: {
  actions: PendingAction[]; cards?: Task[]; outsideTool?: string | null; threadId?: string | null; onError: (e: string) => void;
}) {
  // Busy from a click until the call or card has left the list (approved, discarded, off hold), and for Approve all until it
  // has run them all and they have all left it, so nothing is clicked twice.
  const shown = (id: string) => actions.some((a) => a.id === id);
  const held = (id: string) => cards.some((c) => c.id === id);
  const pending = usePending<string>((k, v) => {
    if (k !== "all") return k.startsWith("ok:") ? !held(k.slice(3)) : !shown(k.slice(k.indexOf(":") + 1));
    const done = v as AllDone | undefined;
    return !!done && !done.ran.some((a) => shown(a.id)) && !done.cards.some(held);
  }, threadId ?? actions[0]?.threadId);
  const fail = (e: unknown) => onError(String(e));
  const waiting = actions.filter((a) => a.status === "pending");
  const outside = actions[0]?.outsideTool || outsideTool || "";
  // the calls first, in their order, then the cards' holds
  const approveAll = async (): Promise<AllDone> => {
    const ran = waiting.length ? await approvePendingActions(waiting.map((a) => a.id)) : [];
    const ids = cards.map((c) => c.id);
    await Promise.all(ids.map((id) => updateTask(id, { hold: "" })));
    return { ran, cards: ids };
  };
  const allTitle = cards.length === 0 ? "Runs each of these, in this order"
    : waiting.length === 0 ? "Takes each card's hold off, so its column's agents can start it"
    : "Runs each call, in this order, then takes the cards' holds off";
  return (
    <section className="pending-actions" aria-label="Waiting for your OK">
      <div className="pa-head">
        <span className="badge needs">Waiting for your OK</span>
        <span className="pa-why" title={outside || undefined}>{why(outside, actions.length, cards.length)}</span>
        {waiting.length + cards.length > 1 && (
          <BusyButton className="btn sm primary" pending={pending} name="all" busyLabel="Approving…" icon={<Check className="icon" />}
            title={allTitle} onClick={() => pending.act("all", approveAll, fail)}>
            Approve all</BusyButton>
        )}
      </div>
      {actions.map((a) => (
        <div key={a.id} className="pa-card">
          <div className="pa-line"><b>{a.preview?.line ?? a.tool}</b><span className="pa-tool">{a.tool}</span></div>
          {a.preview && <Preview p={a.preview} />}
          <div className="pa-actions">
            {a.status === "running" ? <span className="faint">Approved: running…</span> : (
              <>
                <BusyButton className="btn sm primary" pending={pending} name={`approve:${a.id}`} busyLabel="Approving…" icon={<Check className="icon" />}
                  title="Runs exactly this call, once" onClick={() => pending.act(`approve:${a.id}`, () => approvePendingAction(a.id), fail)}>Approve</BusyButton>
                <BusyButton className="btn sm" pending={pending} name={`discard:${a.id}`} busyLabel="Discarding…" icon={<X className="icon" />}
                  title="Runs nothing" onClick={() => pending.act(`discard:${a.id}`, () => discardPendingAction(a.id), fail)}>Discard</BusyButton>
              </>
            )}
          </div>
        </div>
      ))}
      {cards.map((c) => (
        <div key={c.id} className="pa-card pa-held" aria-label={`${c.identifier} waits on hold`}>
          <div className="pa-line">
            <span className="pa-card-name"><ItemChip kind="task" itemKey={c.identifier}>{c.identifier} {c.title}</ItemChip></span>
            <span className="pa-column" title="Its column">{c.stateName}</span>
            <BusyButton className="btn sm primary" pending={pending} name={`ok:${c.id}`} busyLabel="Taking it off…" icon={<Check className="icon" />}
              title="Takes the hold off, so its column's agents can start it" onClick={() => pending.act(`ok:${c.id}`, () => updateTask(c.id, { hold: "" }), fail)}>
              OK</BusyButton>
          </div>
        </div>
      ))}
    </section>
  );
}
