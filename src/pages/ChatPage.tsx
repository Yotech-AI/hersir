// Chat with the Team Lead (design system: ChatMessage, ToolCard, ChatSetup): on the left the pinned chats (Pinned, GA-257)
// and the 30 newest chats (Recent) with Archive under them, the conversation or the Archive (#/chats) in the middle, and on
// the right the chat's terminal panel while it shows (GA-278: Open terminal in the top bar, components/chat/ChatTerminal.tsx).
// Without a Team Lead, a panel explains how to set one up.
import { Crown, History, MessagesSquare, Plus, Settings } from "lucide-react";
import { chatAgent, getChatThread, listChatThreads } from "../api";
import { go, href } from "../router";
import { useData } from "../lib/useData";
import { useDrawer } from "../lib/drawers";
import { useCurrentTeam } from "../lib/team";
import { relTime } from "../lib/format";
import { chatLabel, chatLabelTitle, chatWaits } from "../lib/inbox";
import { RECENT_MAX } from "../lib/chat";
import { Avatar } from "../components/Avatar";
import { ChatArchive } from "../components/chat/ChatArchive";
import { ChatTerminalPanel, ChatTerminalRail, OpenTerminalButton, useChatTerminal } from "../components/chat/ChatTerminal";
import { ChatThread } from "../components/chat/ChatThread";
import { PinButton } from "../components/chat/PinButton";
import { useChatLive } from "../components/chat/useChat";
import { usePins, type Pins } from "../components/chat/usePins";
import type { ChatStatus, ChatThread as Thread } from "../types";

function ChatSetup() {
  const open = useDrawer();
  const [teamId] = useCurrentTeam();
  return (
    <div className="chat-setup">
      <div className="panel setup-card">
        <span className="setup-icon"><MessagesSquare className="icon lg" /></span>
        <h2>Chat with your Chief</h2>
        <p className="muted">To start chatting, set up the Chief: an agent with Chat turned on. It runs Claude Code with Hersir's tools, so you can ask it to add clients, projects and tasks, set up agents, write docs and read your inbox.</p>
        <button className="btn primary" onClick={() => { go({ page: "team" }); open({ kind: "agent", teamId, preset: { name: "Chief", role: "lead", chat: true } }); }}>
          <Crown className="icon" />Set up the Chief</button>
        <p className="faint small">In chat it works in a folder of its own and can read your linked repositories. It doesn't edit code or run commands: it hands that work to your agents as tasks.</p>
      </div>
    </div>
  );
}

/** A chat on the left, under Pinned or Recent: its link, and beside it its pin button (on hover and focus, always once
 *  pinned). */
function ThreadRow({ t, on, live, pins }: { t: Thread; on: boolean; live: ChatStatus[]; pins: Pins }) {
  const pinned = pins.has(t.id);
  return (
    <div className={`th-row${pinned ? " pinned" : ""}`}>
      <a className={`th${on ? " on" : ""}`} href={href({ page: "chat", id: t.id })} aria-current={on ? "page" : undefined}>
        <span className="t">{live.some((l) => l.threadId === t.id) && <span className="pulse" />}
          {chatLabel(t) && <span className={`badge ${chatWaits(t) ? "needs" : "outline"}`} title={chatLabelTitle(t)}>{chatLabel(t)}</span>}
          <span className="ellipsis">{t.title}</span></span>
        <span className="when">{relTime(t.updatedAt)}</span>
      </a>
      <PinButton id={t.id} pinned={pinned} onToggle={() => pins.toggle(t)} />
    </div>
  );
}

/** `archive`: Chat → Archive (#/chats) instead of a conversation. `youId`: you, for Open terminal's folder picker. */
export function ChatPage({ id, archive = false, youId = "" }: { id?: string; archive?: boolean; youId?: string }) {
  const agent = useData(() => chatAgent().then((a) => ({ a })));
  const threads = useData(() => listChatThreads(RECENT_MAX));
  const pins = usePins();
  const live = useChatLive();
  const open = useDrawer();
  const lead = agent.data?.a ?? null;
  const newest = threads.data ?? [];
  // A pinned chat is under Pinned only, also when it is one of the 30 newest.
  const recent = newest.filter((t) => !pins.has(t.id));
  const listed = id ? newest.find((t) => t.id === id) : undefined;
  // A chat older than the 30 newest (opened from the Archive or Pinned) is asked for on its own: its title, its Runs on.
  // Pinned has it meanwhile, and the answer stays when it is unpinned.
  const older = useData(() => (id && !listed ? getChatThread(id) : Promise.resolve(null)), [id, !listed]);
  const thread = listed ?? (older.data && older.data.id === id ? older.data : undefined)
    ?? (id ? pins.list.find((t) => t.id === id) : undefined);
  const working = live.length > 0;
  const term = useChatTerminal(!!lead);
  return (
    <>
      <div className="topbar">
        <div className="crumbs"><a href={href({ page: "chat" })}>Chat</a>
          {archive ? <><span className="sep">/</span><span>Archive</span></>
            : thread && <><span className="sep">/</span><span className="ellipsis">{thread.title}</span>
              <PinButton id={thread.id} pinned={pins.has(thread.id)} onToggle={() => pins.toggle(thread)} /></>}</div>
        {lead && (
          <div className="actions">
            <a className="lead-chip" href={href({ page: "agent", id: lead.actorId })} title={`${lead.name}: open its page`}>
              <Avatar name={lead.name} kind="agent" size="sm" role={lead.roleKey} /><span>{lead.name}</span>
              {working ? <span className="live-tag"><span className="pulse" />working</span> : lead.status !== "active" ? <span className="faint">paused</span> : null}
            </a>
            <button className="btn ghost sm icon-only" title={`${lead.name} settings`} aria-label={`${lead.name} settings`} onClick={() => open({ kind: "agent", id: lead.actorId })}><Settings className="icon" /></button>
            <OpenTerminalButton youId={youId} />
            <button className="btn sm" onClick={() => go({ page: "chat" })}><Plus className="icon" />New chat</button>
          </div>
        )}
      </div>
      {agent.error && <div className="error-banner">{agent.error}</div>}
      {agent.data && !lead && <ChatSetup />}
      {lead && (
        <div ref={term.chatRef} className={term.className} style={term.style}>
          <aside className="chat-threads" aria-label="Chats">
            <div className="chat-threads-list" data-pins="">
              <a className={`th${!id && !archive ? " on" : ""}`} href={href({ page: "chat" })}><span className="t"><Plus className="icon sm" />New chat</span></a>
              {pins.list.length > 0 && <div className="nav-label">Pinned</div>}
              {pins.list.map((t) => <ThreadRow key={t.id} t={t} on={t.id === id} live={live} pins={pins} />)}
              {recent.length > 0 && <div className="nav-label">Recent</div>}
              {recent.map((t) => <ThreadRow key={t.id} t={t} on={t.id === id} live={live} pins={pins} />)}
            </div>
            {(newest.length > 0 || pins.list.length > 0) && (
              <div className="chat-threads-foot">
                <a className={`th${archive ? " on" : ""}`} href={href({ page: "chats" })} aria-current={archive ? "page" : undefined}
                  title="Every chat, with a search through their titles and messages"><span className="t"><History className="icon sm" />Archive</span></a>
              </div>
            )}
          </aside>
          {archive ? <ChatArchive live={live} pins={pins} /> : <ChatThread threadId={id ?? null} thread={thread} agent={lead} />}
          {term.visible ? <ChatTerminalPanel p={term} /> : term.tabs.length > 0 && <ChatTerminalRail p={term} />}
        </div>
      )}
      {pins.error && <div className="toast" role="status">{pins.error}</div>}
    </>
  );
}
