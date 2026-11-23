// GA-343, GA-345: the stand-in host (mcp-host.html, `npm run mcp-host`), for development and QA only: never in the app's
// build nor in the view's file. It plays Claude Desktop's side of MCP Apps' bridge, so the view can be seen to draw, poll
// and follow the theme without Claude Desktop:
// - it loads the view as built (src-tauri/mcp-app/mcp-app.html, at /built/mcp-app.html) into an iframe with
//   sandbox="allow-scripts" and srcdoc, so the view has an opaque origin and no server, as in Claude;
// - it answers ui/initialize with a host context (theme, display modes), then sends show's input and result
//   ({"view": "board", "id": "GA"}, or no id for every project);
// - it answers the view's tools/call of view_read (the board) and board_changes from a made-up board that changes every
//   few seconds (fakeBoard.ts), or with Hersir's "Open Hersir first" error while "Hersir is open" is off. Only the board:
//   the card and the Chief's thread come with GA-349;
// - it sizes the frame from ui/notifications/size-changed, and goes full screen on ui/request-display-mode;
// - Theme sends ui/notifications/host-context-changed; New tool call mounts a new view, as Claude does for every call.
// Every message both ways shows in the log.
import { FakeBoard, PROJECT, sentence } from "./fakeBoard";
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

type Msg = { jsonrpc: "2.0"; id?: string | number | null; method?: string; params?: Record<string, unknown>; result?: unknown; error?: unknown };
type ViewArgs = { view?: string; id?: string };

const $ = <T extends HTMLElement>(sel: string) => document.querySelector(sel) as T;
const board = new FakeBoard();
const state = {
  theme: (new URLSearchParams(location.search).get("theme") === "light" ? "light" : "dark") as "light" | "dark",
  mode: "inline" as "inline" | "fullscreen",
  open: true,
  /** show's id: the project's key, or null for every project on the board. */
  project: PROJECT.key as string | null,
  reads: 0,
  changeCalls: 0,
  lastRead: null as Date | null,
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
  $("#last-read").textContent = state.lastRead ? state.lastRead.toLocaleTimeString() : "never";
  $("#height").textContent = state.height ? `${state.height} px` : "not said yet";
  $("#mode").textContent = state.mode;
  $("#changes").textContent = `${board.changes} (seq ${board.seq})`;
}

function post(m: Msg) {
  log("host → view", m);
  state.frame?.contentWindow?.postMessage(m, "*");
}

const notify = (method: string, params: unknown) => post({ jsonrpc: "2.0", method, params: params as Record<string, unknown> });

const failed = (text: string) => ({ content: [{ type: "text", text }], isError: true });

/** show's or view_read's result as Hersir gives it, for the made-up board. */
function result(tool: "show" | "view_read", args: ViewArgs) {
  if (!state.open) return failed(NOT_RUNNING);
  if (args.view !== "board") {
    return failed(args.view === "card" || args.view === "chief"
      ? `The stand-in host has only the board: the ${args.view} view comes with GA-349.`
      : `"view" is board, card or chief, not "${args.view ?? ""}"`);
  }
  const id = args.id ?? null;
  if (id !== null && id.toUpperCase() !== PROJECT.key) return failed(`No project called "${id}". Projects: ${PROJECT.name} (${PROJECT.key}).`);
  // view_read's seq is read before its data, as Hersir reads it
  const seq = board.seq;
  const data = board.data(id === null ? null : PROJECT.key);
  return {
    content: [{ type: "text", text: sentence(data) }], isError: false,
    structuredContent: tool === "view_read" ? { view: "board", id: data.project?.key ?? null, seq, data } : { view: "board", id: data.project?.key ?? null, data },
    ...(tool === "show" ? { _meta: { ui: { resourceUri: APP_URI }, "ui/resourceUri": APP_URI } } : {}),
  };
}

/** board_changes' result as Hersir gives it: JSON text and structuredContent. */
function changes(args: { since?: unknown }) {
  if (!state.open) return failed(NOT_RUNNING);
  if (args.since !== undefined && typeof args.since !== "number") return failed("\"since\" must be a whole number");
  const c = board.since(args.since as number | undefined);
  return { content: [{ type: "text", text: JSON.stringify(c) }], structuredContent: c, isError: false };
}

/** show's arguments for this tool call. */
const showArgs = (): ViewArgs => (state.project ? { view: "board", id: state.project } : { view: "board" });

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
        hostCapabilities: { serverTools: {}, logging: {} }, hostContext: hostContext(),
      });
    case "tools/call": {
      const name = String(m.params?.name ?? "");
      const args = (m.params?.arguments ?? {}) as ViewArgs & { since?: unknown };
      if (name === "board_changes") {
        state.changeCalls++;
        status();
        return answer(changes(args));
      }
      if (name !== "view_read" && name !== "show") return fail(-32602, `unknown tool ${name}`);
      if (name === "view_read") { state.reads++; state.lastRead = new Date(); status(); }
      return answer(result(name, args));
    }
    case "ui/request-display-mode": {
      const want = m.params?.mode === "fullscreen" ? "fullscreen" : "inline";
      setMode(want);
      return answer({ mode: want });
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

$("#theme").addEventListener("click", () => {
  state.theme = state.theme === "dark" ? "light" : "dark";
  document.documentElement.dataset.theme = state.theme;
  $("#theme").textContent = `Theme: ${state.theme}`;
  notify("ui/notifications/host-context-changed", { theme: state.theme });
});
$<HTMLInputElement>("#open").addEventListener("change", (e) => { state.open = (e.target as HTMLInputElement).checked; });
$<HTMLInputElement>("#whole").addEventListener("change", (e) => { state.project = (e.target as HTMLInputElement).checked ? null : PROJECT.key; });
$("#change").addEventListener("click", () => { board.change(); status(); });
$("#remount").addEventListener("click", () => void mount());
$("#exit").addEventListener("click", () => setMode("inline"));
$<HTMLSelectElement>("#width").addEventListener("change", (e) => { $("#frame-box").style.width = (e.target as HTMLSelectElement).value; });

// The board changes every 6 s while Auto change is on, as cards move in Hersir.
setInterval(() => { if ($<HTMLInputElement>("#auto").checked) { board.change(); status(); } }, 6000);

document.documentElement.dataset.theme = state.theme;
$("#theme").textContent = `Theme: ${state.theme}`;
void mount();
