import { Fragment, memo } from "react";
import Markdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";
import { openUrl } from "@tauri-apps/plugin-opener";
import { viewUrl } from "../lib/markdown";
import { parseItemUrl, splitItemLinks, type ItemKind } from "../lib/itemLinks";
import { openItem } from "../lib/openItem";
import { useBesideChat } from "../lib/besideChat";
import { modClick } from "../lib/keys";
import { useItemHover } from "./ItemHoverCard";

/** Plain text (a message you sent in the chat) with its links to Gizai items shown as chips. */
export function LinkedText({ text }: { text: string }) {
  return <>{splitItemLinks(text).map((p, i) => "link" in p
    ? <ItemChip key={i} kind={p.link.kind} itemKey={p.link.key}>{p.link.label}</ItemChip>
    : <Fragment key={i}>{p.text}</Fragment>)}</>;
}

/** A link to a Gizai item (the @ picker's): a chip with its name that opens the item's page in Gizai. Resting the pointer
 *  on it, or tabbing to it, shows its hover card (GA-287) in place of a browser tooltip. On the Chat page a card's chip
 *  opens the card beside the chat instead, and Ctrl+click (Cmd+click on macOS) its page (GA-297). */
export function ItemChip({ kind, itemKey, children }: { kind: ItemKind; itemKey: string; children: React.ReactNode }) {
  const hover = useItemHover(kind, itemKey);
  const beside = useBesideChat();
  const open = (e: React.MouseEvent) => {
    if (kind === "task" && beside && !modClick(e)) beside(itemKey);
    else openItem(kind, itemKey).catch(() => {});
  };
  return (
    <>
      <a {...hover.chip} className={`item-chip kind-${kind}`} href={`gizai:${kind}/${encodeURIComponent(itemKey)}`}
        onClick={(e) => { e.preventDefault(); e.stopPropagation(); hover.hide(); open(e); }}>{children}</a>
      {hover.card}
    </>
  );
}

const PLUGINS = [remarkGfm];
const COMPONENTS: Components = {
  a: ({ href, children }) => {
    const item = parseItemUrl(href);
    if (item) return <ItemChip kind={item.kind} itemKey={item.key}>{children}</ItemChip>;
    return (
      <a href={href || undefined} title={href || undefined}
        onClick={(e) => { e.preventDefault(); e.stopPropagation(); if (href) openUrl(href).catch(() => {}); }}>{children}</a>
    );
  },
  // Remote images are blocked by the content security policy; show the alt text instead.
  img: ({ alt }) => <span className="faint">[image{alt ? `: ${alt}` : ""}]</span>,
};

/** Renders Markdown (GFM). Raw HTML is never rendered; links are http(s)/mailto, which open in the system browser, or gizai:
 *  links to items, which show as chips and open in Gizai. Memoised on its text (GA-176): when the screen around it redraws
 *  (a streamed piece of the chat's answer, a live run's clock), the same text isn't parsed again. */
export const MarkdownView = memo(function MarkdownView({ md }: { md: string }) {
  return (
    <div className="prose">
      <Markdown remarkPlugins={PLUGINS} skipHtml urlTransform={viewUrl} components={COMPONENTS}>
        {md}
      </Markdown>
    </div>
  );
});
