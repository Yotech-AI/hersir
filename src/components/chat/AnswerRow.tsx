// The row of buttons under a Chief answer (GA-298; design/islands-chat → What opens, and when; Figma frame 2): one row
// under the whole answer (its text and tool messages), ghost and small as Open in terminal (GA-279), wrapping, in this order:
// 1. Open GA-12 (square-kanban) for each card the answer links, at most three, then "and 2 more" (a menu of the rest);
// 2. Open board (columns-3) when its tool calls made or moved cards;
// 3. Open in Editor (file-code) with the file and line in mono (tauri.conf.json:4; its title the whole path), for each file
//    and line it names that is a file in the chat project's folder or one of the Editor's folders, at most three;
// 4. Open in terminal, GA-279's (OpenInTerminal.tsx, which holds the row and puts these first).
// A click opens it in the chat's side island (showing an open tab instead of a second one); Ctrl+click (Cmd+click on macOS)
// on a card opens its page, as a chip does. Nothing opens by itself. An answer with none of these has no row.
// What the buttons read is src/lib/chatOpen.ts's. Whether a named file is there is asked once per file (editorVersions, per
// folder at once) and kept while Hersir runs; until it is known, a row with nothing else keeps a button's room.
import { memo, useEffect, useMemo, useState } from "react";
import { ChevronDown, Columns3, FileCode, SquareKanban } from "lucide-react";
import { editorVersions } from "../../api";
import type { ChatMessage, EditorFolder } from "../../types";
import {
  andMore, CARDS_SHOWN, fileLabel, fileLines, FILES_SHOWN, foldersToLook, inFolder, isAbsolute, linkedCards, madeOrMoved, openCardLabel, type FileLine,
} from "../../lib/chatOpen";
import { openBoardTab, openCardTab } from "../../lib/chatSide";
import { openItem } from "../../lib/openItem";
import { modClick, modKey } from "../../lib/keys";
import { ContextMenu } from "../editor/Menus";
import { OpenInTerminal, refreshEditorFolders, useEditorFolders } from "./OpenInTerminal";
import { openEditorTab } from "./ChatEditor";
import { useChatProject } from "./useChatProject";

// ---------- the named files that are there ----------

/** Whether a file is there, by its folder and path; kept while Hersir runs. */
const there = new Map<string, boolean>();
const fileKey = (folder: string, path: string) => `${folder}\n${path}`;

type Found = { folder: EditorFolder; path: string; line: number };

/** The files an answer names that are there, at most FILES_SHOWN, in the order named: an absolute path in the Editor folder
 *  it is in; a path from a folder in the first folder that has it (the chat project's first). Undefined until known. */
function useAnswerFiles(wanted: FileLine[]): Found[] | undefined {
  const on = wanted.length > 0;
  const folders = useEditorFolders(on);
  const { project } = useChatProject();
  const projectId = project === undefined ? undefined : project?.id ?? null;
  const [tick, setTick] = useState(0);
  // each named file's places to look, in order
  const plan = useMemo(() => {
    if (!on || !folders || projectId === undefined) return null;
    return wanted.map((w) => {
      if (!isAbsolute(w.path)) return foldersToLook(folders, projectId).map((folder) => ({ folder, path: w.path, line: w.line }));
      const at = inFolder(w.path, folders);
      return at ? [{ folder: at.folder, path: at.path, line: w.line }] : [];
    });
  }, [on, folders, projectId, wanted]);

  useEffect(() => {
    if (!plan) return;
    // a path in none of the folders may be in one that is new since they were asked
    if (plan.some((c, i) => !c.length && isAbsolute(wanted[i]!.path))) refreshEditorFolders();
    const ask = new Map<string, string[]>();
    for (const c of plan.flat()) {
      const list = ask.get(c.folder.id) ?? [];
      if (!there.has(fileKey(c.folder.id, c.path)) && !list.includes(c.path)) ask.set(c.folder.id, [...list, c.path]);
    }
    if (!ask.size) return;
    let alive = true;
    Promise.all([...ask].map(([folder, paths]) => editorVersions(folder, paths).then(
      (vs) => paths.forEach((p, i) => there.set(fileKey(folder, p), !!vs[i])),
      () => paths.forEach((p) => there.set(fileKey(folder, p), false)),
    ))).then(() => { if (alive) setTick((t) => t + 1); });
    return () => { alive = false; };
  }, [plan]); // eslint-disable-line react-hooks/exhaustive-deps

  return useMemo(() => {
    if (!on) return [];
    if (!plan) return undefined;
    const out: Found[] = [];
    for (const places of plan) {
      let hit: Found | null = null;
      for (const c of places) {
        const is = there.get(fileKey(c.folder.id, c.path));
        if (is === undefined) return undefined;
        if (is) { hit = c; break; }
      }
      if (hit && !out.some((o) => o.folder.id === hit!.folder.id && o.path === hit!.path && o.line === hit!.line)) out.push(hit);
      if (out.length === FILES_SHOWN) break;
    }
    return out;
  }, [on, plan, tick]);
}

// ---------- the row ----------

/** A card's button: beside the chat, Ctrl+click (Cmd+click) its page. */
const openCard = (id: string, e: { ctrlKey: boolean; metaKey: boolean }) => {
  if (modClick(e)) openItem("task", id).catch(() => {});
  else openCardTab(id);
};

/** Under a Chief answer (`items`: its messages, text and tool): its row of buttons, or nothing. `you`: for Open in
 *  terminal's folder picker. */
export const AnswerRow = memo(function AnswerRow({ items, you }: { items: ChatMessage[]; you?: string }) {
  const md = useMemo(() => items.filter((m) => m.role !== "tool").map((m) => m.bodyMd ?? "").filter((t) => t.trim()).join("\n\n"), [items]);
  const cards = useMemo(() => linkedCards(md), [md]);
  const board = useMemo(() => madeOrMoved(items.filter((m) => m.role === "tool")), [items]);
  const wanted = useMemo(() => fileLines(md), [md]);
  const files = useAnswerFiles(wanted);
  const [more, setMore] = useState<{ x: number; y: number } | null>(null);
  const shown = cards.slice(0, CARDS_SHOWN);
  const rest = cards.slice(CARDS_SHOWN);
  const any = cards.length > 0 || board || (files?.length ?? 0) > 0;
  // until the files are known, a row with nothing else keeps a button's room, so the chat doesn't jump
  const wait = !any && wanted.length > 0 && files === undefined;
  const lead = any || wait ? (
    <>
      {shown.map((id) => (
        <button key={id} type="button" className="btn ghost sm" title={`Open ${id} beside the chat (${modKey()}+click: its page)`} onClick={(e) => openCard(id, e)}>
          <SquareKanban className="icon" />{openCardLabel(id)}
        </button>
      ))}
      {rest.length > 0 && (
        <button type="button" className="btn ghost sm" aria-haspopup="menu" aria-expanded={!!more} title={rest.join(", ")}
          onMouseDown={(e) => { if (more) e.stopPropagation(); }}
          onClick={(e) => { const r = e.currentTarget.getBoundingClientRect(); setMore(more ? null : { x: r.left, y: r.bottom + 4 }); }}>
          {andMore(rest.length)}<ChevronDown className="icon" />
        </button>
      )}
      {board && <button type="button" className="btn ghost sm" title="Open the board beside the chat" onClick={openBoardTab}><Columns3 className="icon" />Open board</button>}
      {files?.map((f) => (
        <button key={`${f.folder.id}\n${f.path}:${f.line}`} type="button" className="btn ghost sm" title={`${f.folder.path}/${f.path}, line ${f.line}`}
          onClick={() => openEditorTab(f.folder.id, f.path, f.line)}>
          <FileCode className="icon" />Open in Editor<span className="mono">{fileLabel(f.path, f.line)}</span>
        </button>
      ))}
      {wait && <span className="btn ghost sm msg-term-wait" aria-hidden><FileCode className="icon" />Open in Editor</span>}
      {more && <ContextMenu at={more} label="More cards" onClose={() => setMore(null)}
        items={rest.map((id) => ({ label: openCardLabel(id), title: `Open ${id} beside the chat`, run: () => openCardTab(id) }))} />}
    </>
  ) : null;
  return <OpenInTerminal md={md} you={you} lead={lead} />;
});
