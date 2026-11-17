// The closed sidebar's toolbar (GA-291; design/islands/README.md → A sidebar that closes, Figma Islands/Sidebar toolbar
// 269:259): at the top left, on the background, in the row of the page's top bar. Show sidebar, New task, Search and the
// Inbox with its count, each doing what the sidebar's own button does; each names itself and its key in its tooltip (the
// button's title, as on every Gizai button), the Inbox its count. Tab reaches them in this order, before the crumbs.
import { Inbox, PanelLeftOpen, Search, SquarePen } from "lucide-react";
import { useLayoutEffect, useRef } from "react";
import { listChatThreads, listTasks } from "../api";
import { href } from "../router";
import { inboxCount } from "../lib/inbox";
import { modKey } from "../lib/keys";
import { inboxTip, sidebarKeyName } from "../lib/sidebar";
import { useData } from "../lib/useData";

type Props = {
  youId: string;
  onShow: () => void;
  onNewTask: () => void;
  onSearch: () => void;
  /** Its width in px, so the page's top bar starts after it (app.css: --side-tools). */
  onWidth?: (px: number) => void;
};

export function SidebarToolbar({ youId, onShow, onNewTask, onSearch, onWidth }: Props) {
  // the same reads as the sidebar's Inbox row (shared while both ask), so the count is the sidebar's
  const tasks = useData(() => listTasks());
  const threads = useData(() => listChatThreads().catch(() => []));
  const inbox = inboxCount(tasks.data ?? [], threads.data ?? [], youId);
  const tip = inboxTip(inbox);
  const mod = modKey();
  const ref = useRef<HTMLDivElement>(null);
  // measured before it is drawn, and again when it grows or shrinks (the count, the font arriving)
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el || !onWidth) return;
    const tell = () => onWidth(Math.ceil(el.getBoundingClientRect().width));
    tell();
    if (typeof ResizeObserver === "undefined") return;
    const watch = new ResizeObserver(tell);
    watch.observe(el);
    return () => watch.disconnect();
  }, [onWidth]);
  return (
    <div ref={ref} className="side-tools" role="group" aria-label="Sidebar actions">
      <button type="button" className="tool" data-side-toggle aria-label="Show sidebar" aria-expanded="false" title={`Show sidebar (${sidebarKeyName(mod)})`} onClick={onShow}>
        <PanelLeftOpen className="icon" />
      </button>
      <button type="button" className="tool" aria-label="New task" title="New task (N)" onClick={onNewTask}>
        <SquarePen className="icon" />
      </button>
      <button type="button" className="tool" aria-label="Search" title={`Search (${mod} K)`} onClick={onSearch}>
        <Search className="icon" />
      </button>
      <a className="tool" href={href({ page: "inbox" })} aria-label={tip.label} title={tip.title}>
        <Inbox className="icon" />
        {inbox > 0 && <span className="count" aria-hidden="true">{inbox}</span>}
      </a>
    </div>
  );
}
