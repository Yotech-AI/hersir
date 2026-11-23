// GA-349: a chat with Hersir's Chief in Claude Desktop's chat (App.tsx): the thread's messages as chief_reply gives them
// (yours, the Chief's and Hersir's notes), a box to send one (ask_chief) and, while the Chief works, its reply as it comes
// in (chief_reply's draft). The thread is the one show named, else the chat From Claude Desktop, which the first message
// makes.
import { useEffect, useRef, useState } from "react";
import { LoaderCircle, SendHorizontal } from "lucide-react";
import { MarkdownView } from "../components/MarkdownView";
import { modKey } from "../lib/keys";
import type { ChiefData } from "./views";

/** A message's time, as the chat shows it: 14:05. */
function clock(at: string): string {
  const ms = Date.parse(at);
  return Number.isFinite(ms) ? new Date(ms).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }) : "";
}

/** The chat's status, as a badge: teal while the Chief works now, gold when it waits for you in Hersir. */
function Status({ status }: { status: ChiefData["status"] }) {
  if (status === "working") return <span className="badge live"><span className="pulse" />The Chief is working</span>;
  if (status === "waiting for you") return <span className="badge needs" title="Approve its calls, or send your message, in Hersir's chat">Waits for you in Hersir</span>;
  return <span className="badge">Answered</span>;
}

export function ChiefView({ chief, ask, openLink }: {
  chief: ChiefData;
  /** Sends a message with ask_chief; its error's text, or null when it went. */
  ask: (message: string) => Promise<string | null>;
  openLink: (url: string) => void;
}) {
  const [text, setText] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const list = useRef<HTMLDivElement>(null);

  // The newest message, or the reply coming in, stays in sight in full screen, where the messages scroll (inline, the
  // frame grows with them). Only this box scrolls: never the chat around the view.
  const last = chief.messages[chief.messages.length - 1]?.id;
  useEffect(() => {
    const el = list.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [last, chief.draft]);

  const send = async () => {
    const message = text.trim();
    if (!message || busy) return;
    setBusy(true);
    setError(null);
    const failed = await ask(message);
    setBusy(false);
    if (failed) setError(failed);
    else setText("");
  };

  return (
    <div className="mcp-chief">
      <div className="mcp-chief-head"><Status status={chief.status} /></div>
      <div className="mcp-messages" ref={list}>
        {chief.messages.length === 0 && !chief.draft && (
          <p className="faint mcp-empty">No messages yet. Ask the Chief something: it answers with its memory, its copies of the code and the board.</p>
        )}
        {chief.messages.map((m) => (
          <div key={m.id} className={`mcp-msg from-${m.from === "you" ? "you" : m.from === "Chief" ? "chief" : "hersir"}`}>
            <div className="by"><b>{m.from === "you" ? "You" : m.from}</b><span className="faint">{clock(m.time)}</span></div>
            {m.from === "you" ? <p className="mcp-plain">{m.text}</p> : <MarkdownView md={m.text} openLink={openLink} />}
          </div>
        ))}
        {chief.status === "working" && (
          <div className="mcp-msg from-chief draft" aria-live="polite">
            <div className="by"><b>Chief</b><span className="pulse" aria-hidden /><span className="faint">writing…</span></div>
            {chief.draft ? <MarkdownView md={chief.draft} openLink={openLink} /> : <p className="faint">Thinking…</p>}
          </div>
        )}
      </div>
      <div className="mcp-compose">
        <textarea className="textarea" aria-label="Message to the Chief" placeholder="Message the Chief" value={text}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => { if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) { e.preventDefault(); void send(); } }} />
        <div className="mcp-compose-acts">
          <button className="btn primary sm" onClick={() => void send()} disabled={!text.trim() || busy}>
            {busy ? <><LoaderCircle className="icon spin" />Sending…</> : <><SendHorizontal className="icon" />Send</>}
          </button>
          <span className="faint mcp-hint">{modKey()}+Enter sends{chief.status === "working" ? ": it waits until the Chief's answer is done" : ""}</span>
        </div>
        {error && <p className="mcp-error" role="alert">{error}</p>}
      </div>
    </div>
  );
}
