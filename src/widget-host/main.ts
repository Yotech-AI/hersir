// GA-343, GA-345, GA-349: the stand-in host (mcp-host.html, `npm run mcp-host`), for development and QA only: never in
// the app's build nor in the view's file. It plays Claude Desktop's side of MCP Apps' bridge, so the view can be seen to
// draw, poll, write and follow the theme without Claude Desktop:
// - it loads the view as built (src-tauri/mcp-app/mcp-app.html, at /built/mcp-app.html) into an iframe with
//   sandbox="allow-scripts" and srcdoc, so the view has an opaque origin and no server, as in Claude;
// - it answers ui/initialize with a host context (theme, display modes), then sends show's input and result: the board
//   ({"view": "board", "id": "GA"}, or no id for every project), a card or the Chief's thread, as "Show" picks;
// - it answers the view's tools/call of view_read and board_changes from a made-up board that changes every few seconds
//   (fakeBoard.ts), its cards (fakeCard.ts) and a made-up Chief (fakeChief.ts); and the writes the view makes with the
//   model's tools (move_task, comment_on_task, start_agent_run, stop_agent_run, ask_chief, chief_reply), each as Hersir
//   answers it, or as an error while "Writes fail" is on. While "Hersir is open" is off, every tool says "Open Hersir
//   first";
// - it sizes the frame from ui/notifications/size-changed, goes full screen on ui/request-display-mode, and notes the
//   links ui/open-link asks it to open;
// - Theme sends ui/notifications/host-context-changed; New tool call mounts a new view, as Claude does for every call.
// Every message both ways shows in the log.
import { FakeBoard, PROJECT, sentence } from "./fakeBoard";
import { FakeCards } from "./fakeCard";
import { FakeChief } from "./fakeChief";
import "./host.css";

const PROTOCOL = "2026-01-26";
const APP_URI = "ui://hersir/app.html";
const NOT_RUNNING = "Open Hersir first: Claude Desktop reaches your board through the Hersir app on this computer, and it isn't running.";
/** show as Hersir lists it to Claude Desktop (src-tauri/src/tools/apps.rs). */
const SHOW = {
  name: "show",
  description: "Shows the user Hersir's board, a card or the Chief's thread inside this chat. Use it when the user wants to see the board or a "
    + "card, and before you answer about a card, so they see exactly what it says.",
  inputSchema: {
    type: "object", required: ["view"],
    properties: { view: { type: "string", enum: ["board", "card", "chief"] }, id: { type: "string" } },
  },
  annotations: { readOnlyHint: true },
  _meta: { ui: { visibility: ["model", "app"], resourceUri: APP_URI }, "ui/resourceUri": APP_URI },
};
/** The model's tools the view writes with. */
const WRITES = ["move_task", "comment_on_task", "start_agent_run", "stop_agent_run", "ask_chief"];

type Msg = { jsonrpc: "2.0"; id?: string | number | null; method?: string; params?: Record<string, unknown>; result?: unknown; error?: unknown };
type Args = { view?: string; id?: string; since?: unknown; task?: string; column?: string; body_md?: string; agent?: string; message?: string;
  thread?: string; after?: string };

const $ = <T extends HTMLElement>(sel: string) => document.querySelector(sel) as T;
const board = new FakeBoard();
const cards = new FakeCards(board);
const chief = new FakeChief();
const state = {
  theme: (new URLSearchParams(location.search).get("theme") === "light" ? "light" : "dark") as "light" | "dark",
  mode: "inline" as "inline" | "fullscreen",
  open: true,
  failWrites: false,
  /** show's arguments for the next tool call (the Show select). */
  show: { view: "board", id: PROJECT.key } as Args,
  /** The card the view showed last (show's or view_read's), for "A comment from Hersir". */
  lastCard: null as string | null,
  reads: 0,
  changeCalls: 0,
  writes: 0,
  lastRead: null as Date | null,
  lastLink: null as string | null,
  height: 0,
  frame: null as HTMLIFrameElement | null,
  calls: 0,
};

function log(dir: string, m: unknown) {
  const list = $("#log");
  const row = document.createElement("li");
  const text = JSON.stringify(m);
  row.textContent = `${new Date().toLocaleTimeString()}  ${dir}  ${text.length > 400 ? `${text.slice(0, 400)}…` : text}`;
  list.prepend(row);
  while (list.children.length > 200) list.lastElementChild?.remove();
}

function status() {
  $("#reads").textContent = String(state.reads);
  $("#change-calls").textContent = String(state.changeCalls);
  $("#writes").textContent = String(state.writes);
  $("#last-read").textContent = state.lastRead ? state.lastRead.toLocaleTimeString() : "never";
  $("#height").textContent = state.height ? `${state.height} px` : "not said yet";
  $("#mode").textContent = state.mode;
  $("#changes").textContent = `${board.changes} (seq ${board.seq})`;
  $("#link").textContent = state.lastLink ?? "none";
}

function post(m: Msg) {
  log("host → view", m);
  state.frame?.contentWindow?.postMessage(m, "*");
}

const notify = (method: string, params: unknown) => post({ jsonrpc: "2.0", method, params: params as Record<string, unknown> });

const failed = (text: string) => ({ content: [{ type: "text", text }], isError: true });
/** A model tool's answer: JSON as its text, no structuredContent (gizai_mcp's Reply::of). */
const answered = (value: unknown) => ({ content: [{ type: "text", text: JSON.stringify(value) }], isError: false });

/** show's or view_read's result as Hersir gives it, for the made-up board, cards and Chief. */
function result(tool: "show" | "view_read", args: Args) {
  if (!state.open) return failed(NOT_RUNNING);
  // view_read's seq is read before its data, as Hersir reads it
  const seq = board.seq;
  const out = (view: string, id: string | null, data: unknown, text: string) => ({
    content: [{ type: "text", text }], isError: false,
    structuredContent: tool === "view_read" ? { view, id, seq, data } : { view, id, data },
    ...(tool === "show" ? { _meta: { ui: { resourceUri: APP_URI }, "ui/resourceUri": APP_URI } } : {}),
  });
  const id = args.id ?? null;
  if (args.view === "board") {
    if (id !== null && id.toUpperCase() !== PROJECT.key) return failed(`No project called "${id}". Projects: ${PROJECT.name} (${PROJECT.key}).`);
    const data = board.data(id === null ? null : PROJECT.key);
    return out("board", data.project?.key ?? null, data, sentence(data));
  }
  if (args.view === "card") {
    if (!id) return failed("missing \"id\": the card's identifier, like GA-12");
    const data = cards.data(id);
    if (!data) return failed(`There is no card ${id} in Hersir.`);
    state.lastCard = data.identifier;
    const hold = data.hold ? `, on hold (${data.hold.replace(/_/g, " ")})` : "";
    return out("card", data.identifier, data, `${data.identifier} – ${data.title.replace(/\.$/, "")}, in ${data.column}${hold}.`);
  }
  if (args.view === "chief") {
    const data = chief.view(id);
    if (typeof data === "string") return failed(data);
    return out("chief", data.thread, data, chief.sentence(data));
  }
  return failed(`"view" is board, card or chief, not "${args.view ?? ""}"`);
}

/** board_changes' result as Hersir gives it: JSON text and structuredContent. */
function changes(args: Args) {
  if (!state.open) return failed(NOT_RUNNING);
  if (args.since !== undefined && typeof args.since !== "number") return failed("\"since\" must be a whole number");
  const c = board.since(args.since as number | undefined);
  return { content: [{ type: "text", text: JSON.stringify(c) }], structuredContent: c, isError: false };
}

/** A write with one of the model's tools, as Hersir answers it; while "Writes fail" is on, Hersir's refusal. */
function write(name: string, a: Args) {
  if (!state.open) return failed(NOT_RUNNING);
  state.writes++;
  if (state.failWrites) return failed(`Hersir didn't do ${name}: the stand-in host's "Writes fail" is on (as when a column refuses a card, or a run can't start).`);
  const task = String(a.task ?? "");
  const why = name === "move_task" ? (a.column ? board.move(task, a.column) : "missing \"column\": the column's name, like In progress")
    : name === "comment_on_task" ? cards.comment(task, String(a.body_md ?? ""))
    : name === "start_agent_run" ? cards.start(task, a.agent)
    : name === "stop_agent_run" ? cards.stop(task)
    : null;
  if (name === "ask_chief") {
    const sent = chief.ask(String(a.message ?? ""), a.thread);
    return typeof sent === "string" ? failed(sent) : answered(sent);
  }
  if (why) return failed(why);
  const found = board.find(task);
  return answered({ task: found?.card.identifier ?? task, column: found?.column ?? null, done: name });
}

/** show's arguments for this tool call. */
const showArgs = (): Args => ({ ...state.show });

function hostContext() {
  const frame = $("#frame-box");
  return {
    theme: state.theme, displayMode: state.mode, availableDisplayModes: ["inline", "fullscreen"], platform: "desktop",
    locale: navigator.language, timeZone: Intl.DateTimeFormat().resolvedOptions().timeZone,
    containerDimensions: { width: frame.clientWidth, maxHeight: 4000 },
    toolInfo: { id: state.calls, tool: SHOW },
  };
}

function onRequest(m: Msg) {
  const id = m.id as string | number;
  const answer = (res: unknown) => post({ jsonrpc: "2.0", id, result: res });
  const fail = (code: number, message: string) => post({ jsonrpc: "2.0", id, error: { code, message } });
  switch (m.method) {
    case "ui/initialize":
      return answer({
        protocolVersion: PROTOCOL, hostInfo: { name: "Hersir stand-in host", version: "1" },
        hostCapabilities: { serverTools: {}, logging: {}, openLinks: {} }, hostContext: hostContext(),
      });
    case "tools/call": {
      const name = String(m.params?.name ?? "");
      const args = (m.params?.arguments ?? {}) as Args;
      let res: unknown;
      if (name === "board_changes") {
        state.changeCalls++;
        res = changes(args);
      } else if (name === "view_read" || name === "show") {
        if (name === "view_read") { state.reads++; state.lastRead = new Date(); }
        res = result(name, args);
      } else if (name === "chief_reply") {
        // it only reads: "Writes fail" doesn't touch it
        const r = !state.open ? NOT_RUNNING : chief.reply(args.thread, args.after);
        res = typeof r === "string" ? failed(r) : answered(r);
      } else if (WRITES.includes(name)) {
        res = write(name, args);
      } else {
        return fail(-32602, `unknown tool ${name}`);
      }
      status();
      return answer(res);
    }
    case "ui/request-display-mode": {
      const want = m.params?.mode === "fullscreen" ? "fullscreen" : "inline";
      setMode(want);
      return answer({ mode: want });
    }
    case "ui/open-link": {
      // as Claude does: it opens the link in the user's browser (here only noted)
      state.lastLink = String(m.params?.url ?? "");
      status();
      return answer({});
    }
    case "ping":
      return answer({});
    default:
      return fail(-32601, `method not found: ${m.method}`);
  }
}

function onNotification(m: Msg) {
  if (m.method === "ui/notifications/initialized") {
    // As Claude does once the view is ready: the tool's input, then its result.
    notify("ui/notifications/tool-input", { arguments: showArgs() });
    notify("ui/notifications/tool-result", result("show", showArgs()));
    status();
  } else if (m.method === "ui/notifications/size-changed") {
    const h = Number(m.params?.height);
    if (Number.isFinite(h) && h > 0) {
      state.height = Math.ceil(h);
      if (state.frame && state.mode === "inline") state.frame.style.height = `${state.height}px`;
      status();
    }
  }
}

window.addEventListener("message", (e) => {
  if (!state.frame || e.source !== state.frame.contentWindow) return;
  const m = e.data as Msg;
  if (!m || m.jsonrpc !== "2.0") return;
  log("view → host", m);
  if (m.method && m.id !== undefined && m.id !== null) onRequest(m);
  else if (m.method) onNotification(m);
});

function setMode(mode: "inline" | "fullscreen") {
  state.mode = mode;
  document.body.classList.toggle("fullscreen", mode === "fullscreen");
  if (state.frame) state.frame.style.height = mode === "fullscreen" ? "" : state.height ? `${state.height}px` : "";
  notify("ui/notifications/host-context-changed", { displayMode: mode });
  status();
}

/** A new view for a new tool call, as Claude mounts one for every call. */
async function mount() {
  const box = $("#frame-box");
  box.replaceChildren();
  state.calls++;
  state.height = 0;
  if (state.mode === "fullscreen") {
    state.mode = "inline";
    document.body.classList.remove("fullscreen");
  }
  $("#call").textContent = `show ${JSON.stringify(showArgs())}`;
  const res = await fetch(`/built/mcp-app.html?t=${Date.now()}`);
  const html = await res.text();
  if (!res.ok) {
    const p = document.createElement("p");
    p.className = "host-error";
    p.textContent = html;
    box.append(p);
    return;
  }
  $("#size").textContent = `${(new Blob([html]).size / 1024).toFixed(0)} KB`;
  const frame = document.createElement("iframe");
  frame.setAttribute("sandbox", "allow-scripts");
  frame.title = "Hersir's view";
  frame.srcdoc = html;
  state.frame = frame;
  box.append(frame);
  status();
}

/** The Show select's value → show's arguments: "board", "board:GA", "card:GA-109", "chief". */
function showOf(value: string): Args {
  const [view, id] = value.split(":");
  return id ? { view, id } : { view };
}

$("#theme").addEventListener("click", () => {
  state.theme = state.theme === "dark" ? "light" : "dark";
  document.documentElement.dataset.theme = state.theme;
  $("#theme").textContent = `Theme: ${state.theme}`;
  notify("ui/notifications/host-context-changed", { theme: state.theme });
});
$<HTMLInputElement>("#open").addEventListener("change", (e) => { state.open = (e.target as HTMLInputElement).checked; });
$<HTMLInputElement>("#fail").addEventListener("change", (e) => { state.failWrites = (e.target as HTMLInputElement).checked; });
$<HTMLSelectElement>("#show").addEventListener("change", (e) => {
  state.show = showOf((e.target as HTMLSelectElement).value);
  void mount();
});
$("#change").addEventListener("click", () => { board.change(); status(); });
$("#hersir-comment").addEventListener("click", () => {
  const id = state.lastCard ?? "GA-109";
  cards.comment(id, "A comment written in Hersir: **the view shows it within a few seconds**.", "Jefsev");
  status();
});
$("#hersir-chat").addEventListener("click", () => { chief.fromHersir("A message typed in Hersir's chat: what is left for 0.20.0?"); status(); });
$("#remount").addEventListener("click", () => void mount());
$("#exit").addEventListener("click", () => setMode("inline"));
$<HTMLSelectElement>("#width").addEventListener("change", (e) => { $("#frame-box").style.width = (e.target as HTMLSelectElement).value; });

// The board changes every 6 s while Auto change is on, as cards move in Hersir.
setInterval(() => { if ($<HTMLInputElement>("#auto").checked) { board.change(); status(); } }, 6000);

document.documentElement.dataset.theme = state.theme;
$("#theme").textContent = `Theme: ${state.theme}`;
const first = new URLSearchParams(location.search).get("show");
if (first) {
  state.show = showOf(first);
  const select = $<HTMLSelectElement>("#show");
  if ([...select.options].some((o) => o.value === first)) select.value = first;
}
void mount();
