// GA-349, the stand-in host (development and QA only): a made-up chat with Hersir's Chief, answering ask_chief,
// chief_reply and view_read {view: "chief"} as Hersir does (src-tauri/src/tools/desktop.rs, apps.rs). The first message
// makes the chat From Claude Desktop; each message starts a turn of about 8 s, whose reply comes in bit by bit as the
// draft and then is the Chief's message. A message typed in Hersir's chat (`fromHersir`) starts one too.
import type { ChiefData, ChiefMessage } from "../widget/views";

/** The chat From Claude Desktop's thread id, once the first message made it. */
export const DESKTOP_THREAD = "chat-from-claude-desktop";

/** How long a turn takes, and how long before its first words. */
const TURN_MS = 8000;
const THINK_MS = 1500;

const iso = (ms: number) => new Date(ms).toISOString().replace(/\.\d{3}Z$/, "Z");

const unknown = (id: string) => `There is no chat ${id} in Hersir: give a thread id ask_chief answered with, or leave thread out for the chat From Claude Desktop.`;

export class FakeChief {
  thread: string | null = null;
  private messages: ChiefMessage[] = [];
  private turn: { started: number; answer: string } | null = null;
  private n = 0;

  constructor(private now: () => number = () => Date.now()) {}

  /** ask_chief: the message goes to the chat (made the first time). Its answer, or why it can't. */
  ask(message: string, thread?: string): { thread: string; message: string; status: "working" } | string {
    if (!message.trim()) return "missing \"message\": the message for the Chief";
    if (thread && thread !== this.thread) return unknown(thread);
    this.thread ??= DESKTOP_THREAD;
    const id = this.say("you", message);
    this.start(message);
    return { thread: this.thread, message: id, status: "working" };
  }

  /** A message you typed in Hersir's chat: the view shows it, and the Chief's turn, at its next read. */
  fromHersir(message: string) {
    this.thread ??= DESKTOP_THREAD;
    this.say("you", message);
    this.start(message);
  }

  /** view_read's chief: the chat as it is now (the chat From Claude Desktop without a thread). Why not, for an unknown one. */
  view(thread: string | null): ChiefData | string {
    if (thread && thread !== this.thread) return unknown(thread);
    if (!this.thread) return { thread: null, status: "answered", messages: [], draft: null };
    return this.now_(null);
  }

  /** chief_reply: the messages after `after` (all without it), the status and, while the Chief works, its draft. */
  reply(thread: string | undefined, after?: string): (ChiefData & { thread: string }) | string {
    if (!thread) return "missing \"thread\": the thread id ask_chief answered with";
    if (thread !== this.thread) return unknown(thread);
    return this.now_(after ?? null) as ChiefData & { thread: string };
  }

  /** A sentence for the model, as Hersir writes it. */
  sentence(d: ChiefData): string {
    const n = d.messages.length;
    const status = d.status === "working" ? "the Chief is working on it" : d.status === "waiting for you" ? "it waits for the user in Hersir" : "answered";
    return n === 0 ? "The user sees the Chief's chat \"From Claude Desktop\" from Hersir in this chat: no messages yet."
      : `The user sees the Chief's chat "From Claude Desktop" from Hersir in this chat: ${n === 1 ? "1 message" : `${n} messages`}, ${status}.`;
  }

  private say(from: ChiefMessage["from"], text: string): string {
    const id = `msg-${++this.n}`;
    this.messages.push({ id, from, time: iso(this.now()), text });
    return id;
  }

  private start(message: string) {
    const answer = `You asked: *${message.trim().slice(0, 80)}*\n\nHere is what I see on the board:\n\n- **GA-109** waits for your decision on its shortcut.\n`
      + "- Two cards are in Review, one with a pull request.\n- The Frontend Agent works on GA-106.\n\nThe plan is in [the roadmap](https://example.com/roadmap).";
    // a message sent while the Chief works waits for its turn to end; this stand-in answers the newest one
    this.turn = { started: this.now(), answer };
  }

  /** The chat now: a turn that is done adds the Chief's message. */
  private now_(after: string | null): ChiefData {
    if (this.turn && this.now() - this.turn.started >= TURN_MS) {
      this.say("Chief", this.turn.answer);
      this.turn = null;
    }
    const from = after ? this.messages.findIndex((m) => m.id === after) + 1 : 0;
    const out: ChiefData = { thread: this.thread, status: this.turn ? "working" : "answered", messages: this.messages.slice(from).slice(-20).map((m) => ({ ...m })), draft: null };
    if (this.turn) {
      const share = Math.max(0, (this.now() - this.turn.started - THINK_MS) / (TURN_MS - THINK_MS));
      out.draft = this.turn.answer.slice(0, Math.floor(this.turn.answer.length * Math.min(1, share)));
    }
    return out;
  }
}
