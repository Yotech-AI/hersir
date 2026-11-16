// The @ chip's hover card (GA-287; design/islands/README.md → The chip's hover card, Figma `Islands/Hover card`): resting
// the pointer on a chip, or tabbing to it, shows a small card with the item's full name, project, agent and status.
import { useCallback, useEffect, useId, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { KIND_NAME, type ItemKind } from "../lib/itemLinks";
import { cardPlace, keptPreview, loadPreview, type ItemPreview } from "../lib/itemPreview";
import { roleLabel } from "../lib/agents";
import { holdName } from "../lib/holds";
import { CLICK_TO_OPEN, CLICK_TO_OPEN_BESIDE, useBesideChat } from "../lib/besideChat";
import { Avatar } from "./Avatar";
import { StatusIcon } from "./StatusIcon";

/** The pointer (or the focus) rests this long on a chip before its card shows; leaving the chip and the card hides it
 *  after HIDE_MS. An item not read yet shows its card when it arrives, or after WAIT_MS with the chip's text and kind
 *  until then. */
export const SHOW_MS = 350;
export const HIDE_MS = 250;
export const WAIT_MS = 150;

type Open = { item: ItemPreview | undefined; text: string };

/** Hides the card that shows now, if any: only one shows at a time. */
let hideOpen: (() => void) | undefined;

/** The chip's side of its hover card: the handlers for the chip, and the card while it shows. */
export function useItemHover(kind: ItemKind, itemKey: string) {
  const ref = useRef<HTMLAnchorElement>(null);
  const id = useId();
  const [open, setOpen] = useState<Open | undefined>(undefined);
  const t = useRef<{ show?: number; hide?: number; wait?: number; want: boolean; y?: number; run: number }>({ want: false, run: 0 });

  const clear = () => {
    window.clearTimeout(t.current.show);
    window.clearTimeout(t.current.hide);
    window.clearTimeout(t.current.wait);
  };
  const hide = useCallback(() => {
    clear();
    t.current.want = false;
    t.current.run++;
    if (hideOpen === hide) hideOpen = undefined;
    setOpen(undefined);
  }, []);
  const show = (item: ItemPreview | undefined) => {
    const text = ref.current?.textContent?.trim() || itemKey;
    if (hideOpen && hideOpen !== hide) hideOpen();
    hideOpen = hide;
    setOpen({ item, text });
  };
  /** The delay is over: show the card with the item kept, else read it first. */
  const reveal = () => {
    if (!t.current.want || !ref.current) return;
    const kept = keptPreview(kind, itemKey);
    if (kept) { show(kept); return; }
    const run = ++t.current.run;
    let arrived = false;
    loadPreview(kind, itemKey).then((item) => {
      arrived = true;
      if (t.current.run === run && t.current.want) show(item);
    }, () => {});
    t.current.wait = window.setTimeout(() => { if (!arrived && t.current.run === run && t.current.want) show(undefined); }, WAIT_MS);
  };
  /** The pointer or the focus came to the chip (or the pointer to its card). */
  const want = (onCard = false) => {
    window.clearTimeout(t.current.hide);
    t.current.want = true;
    if (open || onCard) return;
    window.clearTimeout(t.current.show);
    t.current.show = window.setTimeout(reveal, SHOW_MS);
  };
  /** The pointer left the chip or its card. */
  const leave = () => {
    t.current.want = false;
    window.clearTimeout(t.current.show);
    window.clearTimeout(t.current.hide);
    t.current.hide = window.setTimeout(hide, HIDE_MS);
  };

  // Escape hides it (the drawer it may be in stays open: drawers leave an Escape that was handled).
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") { e.preventDefault(); hide(); } };
    document.addEventListener("keydown", onKey, true);
    return () => document.removeEventListener("keydown", onKey, true);
  }, [open, hide]);
  useEffect(() => () => { clear(); if (hideOpen === hide) hideOpen = undefined; }, [hide]);

  /** The chip's line the card goes by: for a chip that wraps, the line the pointer is on. */
  const anchor = useCallback((): DOMRect | undefined => {
    const lines = ref.current ? [...ref.current.getClientRects()] : [];
    const y = t.current.y;
    return (y !== undefined ? lines.find((r) => y >= r.top && y <= r.bottom) : undefined) ?? lines[0];
  }, []);

  const chip = {
    ref,
    "aria-describedby": open ? id : undefined,
    onMouseEnter: (e: React.MouseEvent) => { t.current.y = e.clientY; want(); },
    onMouseMove: (e: React.MouseEvent) => { if (!open) t.current.y = e.clientY; },
    onMouseLeave: leave,
    onFocus: () => { if (!open) t.current.y = undefined; want(); },
    onBlur: hide,
  };
  const card = open ? <ItemHoverCard id={id} kind={kind} text={open.text} item={open.item} anchor={anchor}
    onEnter={() => want(true)} onLeave={leave} /> : null;
  return { chip, card, hide };
}

/** The card itself, in a portal on body (as menus are), so nothing clips it. */
export function ItemHoverCard({ id, kind, text, item, anchor, onEnter, onLeave }: {
  id: string; kind: ItemKind; text: string; item: ItemPreview | undefined; anchor: () => DOMRect | undefined;
  onEnter: () => void; onLeave: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  // on the Chat page a card's chip opens it beside the chat (GA-297)
  const beside = !!useBesideChat() && kind === "task";
  const place = useCallback(() => {
    const el = ref.current, at = anchor();
    if (!el || !at) return;
    const p = cardPlace(at, { width: el.offsetWidth, height: el.offsetHeight }, { width: window.innerWidth, height: window.innerHeight });
    el.style.left = `${p.left}px`;
    el.style.top = `${p.top}px`;
    el.style.visibility = "visible";
  }, [anchor]);
  useLayoutEffect(place);
  useEffect(() => {
    window.addEventListener("scroll", place, true);
    window.addEventListener("resize", place);
    return () => { window.removeEventListener("scroll", place, true); window.removeEventListener("resize", place); };
  }, [place]);
  // A click or a press on the card stays on it: through the portal it would reach the chip's text (a description that
  // edits on a click, a message), and the card isn't a button.
  const stop = (e: React.SyntheticEvent) => e.stopPropagation();
  return createPortal(
    <div ref={ref} id={id} role="tooltip" className="chip-card" style={{ visibility: "hidden" }}
      onMouseEnter={onEnter} onMouseLeave={onLeave} onClick={stop} onMouseDown={stop} onPointerDown={stop} onDoubleClick={stop}>
      <CardBody kind={kind} text={text} item={item} foot={beside ? CLICK_TO_OPEN_BESIDE : CLICK_TO_OPEN} />
    </div>,
    document.body,
  );
}

/** `foot`: the last line, what a click does. */
function CardBody({ kind, text, item, foot }: { kind: ItemKind; text: string; item: ItemPreview | undefined; foot: string }) {
  const end = <p className="t-caption chip-card-foot">{foot}</p>;
  if (!item) return <><Head name={text} /><p className="t-small chip-card-note">{KIND_NAME[kind]}</p>{end}</>;
  switch (item.kind) {
    case "missing":
      return <><Head name={text} /><p className="t-small chip-card-note">Not found</p></>;
    case "task": {
      const { task } = item;
      return <>
        <Head id={task.identifier} name={task.title} />
        <Rows>
          <Row label="Project">{task.projectName ? <Val mark={<Dot color={task.projectColor} />}>{task.projectName}</Val> : <None />}</Row>
          <Row label="Agent">{task.assigneeName
            ? <Val mark={<Avatar name={task.assigneeName} kind={task.assigneeKind} size="sm" />}>{task.assigneeName}</Val>
            : <None>Not assigned</None>}</Row>
          <Row label="Status">
            <Val mark={<StatusIcon category={task.stateCategory} title={task.stateName} />}>{task.stateName}</Val>
            {task.hold && <span className="chip-card-hold">On hold: {task.withLead ? "With the Chief" : holdName(task.hold)}</span>}
          </Row>
        </Rows>
        {end}
      </>;
    }
    case "project": {
      const { project } = item;
      return <>
        <Head mark={<Dot color={project.color} />} name={<>{project.name}<span className="t-id chip-card-key">{project.key}</span></>} />
        <Rows>
          <Row label="Client">{project.clientName ? <Val>{project.clientName}</Val> : <None />}</Row>
          <Row label="Status"><Val>{named(project.status)}</Val></Row>
        </Rows>
        {end}
      </>;
    }
    case "agent": {
      const { agent, workingOn } = item;
      return <>
        <Head mark={<Avatar name={agent.name} kind="agent" size="sm" />} name={agent.name} />
        <Rows>
          <Row label="Role"><Val>{roleLabel(agent.roleKey)}</Val></Row>
          <Row label="Status">{workingOn
            ? <Val mark={<span className="chip-card-live" />} className="live">Working on {workingOn}</Val>
            : <Val>{agent.status === "paused" ? "Paused" : "Active"}</Val>}</Row>
        </Rows>
        {end}
      </>;
    }
    case "client":
      return <>
        <Head name={item.client.name} />
        <Rows><Row label="Status"><Val>{named(item.client.status)}</Val></Row></Rows>
        {end}
      </>;
    case "person": {
      const { person } = item;
      return <>
        <Head mark={<Avatar name={person.name} size="sm" />} name={person.name} />
        <Rows><Row label="Email">{person.email ? <Val className="anywhere">{person.email}</Val> : <None />}</Row></Rows>
        {end}
      </>;
    }
    case "doc": {
      const { doc, project } = item;
      return <>
        <Head name={doc.title} />
        <Rows><Row label="Project">{project ? <Val mark={<Dot color={project.color} />}>{project.name}</Val> : <None />}</Row></Rows>
        {end}
      </>;
    }
  }
}

/** A status as the app names it: planned → Planned. */
const named = (s: string) => (s ? s[0]!.toUpperCase() + s.slice(1).replaceAll("_", " ") : s);

function Head({ id, mark, name }: { id?: string; mark?: ReactNode; name: ReactNode }) {
  return (
    <div className="chip-card-head">
      {id && <span className="t-id chip-card-id">{id}</span>}
      <div className="chip-card-title">{mark}<span className="t-body-strong chip-card-name">{name}</span></div>
    </div>
  );
}

const Rows = ({ children }: { children: ReactNode }) => <dl className="chip-card-rows">{children}</dl>;

function Row({ label, children }: { label: string; children: ReactNode }) {
  return <div className="chip-card-row t-small"><dt>{label}</dt><dd>{children}</dd></div>;
}

function Val({ mark, className, children }: { mark?: ReactNode; className?: string; children: ReactNode }) {
  return <span className={`chip-card-val${className ? ` ${className}` : ""}`}>{mark}<span>{children}</span></span>;
}

const None = ({ children = "None" }: { children?: ReactNode }) => <span className="chip-card-val none"><span>{children}</span></span>;

const Dot = ({ color }: { color?: string | null }) => <span className="chip-card-dot" style={{ background: color ?? "var(--text-3)" }} />;

