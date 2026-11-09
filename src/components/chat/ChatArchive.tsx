// Chat → Archive (#/chats): every chat, the newest activity first, and a search through their titles and the messages you
// and the Team Lead wrote, each with its pin button (GA-257). It takes the conversation's place; Recent stays on the left.
// Nothing here hides or deletes a chat.
import { Fragment, useEffect, useState } from "react";
import { onRowsChanged, searchChatThreads } from "../../api";
import { href } from "../../router";
import { relTime } from "../../lib/format";
import { chatLabel, chatLabelTitle, chatWaits } from "../../lib/inbox";
import { highlight, hitAuthor, snippet } from "../../lib/chat";
import type { ChatHit, ChatStatus } from "../../types";
import { PinButton } from "./PinButton";
import type { Pins } from "./usePins";

/** How long typing pauses before the search runs. */
const PAUSE_MS = 250;

/** The chats found for `query` (all of them for ""), asked again when a chat changes. `found.query`: the search they are for. */
function useFound(query: string) {
  const [found, setFound] = useState<{ query: string; hits: ChatHit[] } | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    // A search that was overtaken by the next one is dropped when it answers.
    let alive = true;
    let un: (() => void) | undefined;
    const load = () => searchChatThreads(query).then((hits) => { if (alive) { setFound({ query, hits }); setError(null); } })
      .catch((e) => { if (alive) setError(String(e)); });
    load();
    // a pin changes nothing it finds
    onRowsChanged((table) => { if (alive && table.startsWith("chat") && table !== "chat_pins") load(); }).then((f) => (alive ? (un = f) : f())).catch(() => {});
    return () => { alive = false; un?.(); };
  }, [query]);
  return { found, error };
}

/** `text` with the matches of `query` highlighted. */
function Marked({ text, query }: { text: string; query: string }) {
  return <>{highlight(text, query).map((p, i) => (p.hit ? <mark key={i}>{p.text}</mark> : <Fragment key={i}>{p.text}</Fragment>))}</>;
}

/** `pins`: the pinned chats, for each row's Pin to the top / Unpin (GA-257); without them the rows have no pin button. */
export function ChatArchive({ live, pins }: { live: ChatStatus[]; pins?: Pins }) {
  const [text, setText] = useState("");
  const [query, setQuery] = useState("");
  useEffect(() => { const t = setTimeout(() => setQuery(text.trim()), PAUSE_MS); return () => clearTimeout(t); }, [text]);
  const { found, error } = useFound(query);
  const hits = found?.hits ?? [];
  const q = found?.query ?? "";
  return (
    <section className="chat-main chat-archive" aria-label="Archive">
      <div className="archive-head">
        <div className="chat-col">
          <input className="input archive-search" type="search" autoFocus aria-label="Search all chats" placeholder="Search chat titles and messages"
            value={text} onChange={(e) => setText(e.target.value)} />
        </div>
      </div>
      <div className="chat-scroll">
        <div className="chat-col">
          {error && <div className="error-banner" role="alert">{error}</div>}
          {found && hits.length === 0 && (q
            ? <div className="empty"><b>No chats match “{q}”.</b><span>The search looks in chat titles and in your messages and the Team Lead's, not in tool calls or Hersir's notes.</span></div>
            : <div className="empty"><b>No chats yet.</b><span>Start one with New chat. Every chat stays here, also after it has left Recent.</span></div>)}
          {hits.length > 0 && (
            <div className="archive-list" role="list" aria-label={q ? "Chats found" : "All chats"}>
              {hits.map(({ thread: t, message: m }) => (
                <div key={t.id} role="listitem" className={`archive-item${pins?.has(t.id) ? " pinned" : ""}`}>
                  <a className="archive-row" href={href({ page: "chat", id: t.id })}>
                    <span className="top">
                      {live.some((l) => l.threadId === t.id) && <span className="pulse" />}
                      {chatLabel(t) && <span className={`badge ${chatWaits(t) ? "needs" : "outline"}`} title={chatLabelTitle(t)}>{chatLabel(t)}</span>}
                      <span className="title" title={t.title}><Marked text={t.title} query={q} /></span>
                      <span className="when" title={new Date(t.updatedAt).toLocaleString("en-GB")}>{relTime(t.updatedAt)}</span>
                    </span>
                    {m && <span className="snippet"><span className="who">{hitAuthor(m)}:</span> <Marked text={snippet(m.bodyMd ?? "", q)} query={q} /></span>}
                  </a>
                  {pins && <PinButton id={t.id} pinned={pins.has(t.id)} onToggle={() => pins.toggle(t)} />}
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    </section>
  );
}
