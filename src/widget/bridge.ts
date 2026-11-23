// GA-343: the board view's side of MCP Apps' bridge to the host (Claude Desktop, or the stand-in host page in
// src/widget-host): JSON-RPC 2.0 over postMessage with the parent window, protocol 2026-01-26. Small and of its own:
// @modelcontextprotocol/ext-apps gives only its types here, as its App class brings the MCP client, zod and ajv.
//
// The view asks `ui/initialize` and says `ui/notifications/initialized`; the host then sends the tool's input and result
// (`ui/notifications/tool-input`, `ui/notifications/tool-result`) and changes of its context (theme, display mode). The
// view calls the server's tools through the host (`tools/call`), asks for another display mode, and says its size
// (`ui/notifications/size-changed`). No network: everything goes through the host.
import type { McpUiDisplayMode, McpUiHostContext, McpUiInitializeResult } from "@modelcontextprotocol/ext-apps";

/** The MCP Apps protocol version this view speaks (ext-apps 2.1.0's LATEST_PROTOCOL_VERSION). */
export const PROTOCOL = "2026-01-26";

/** A tool's result as the host passes it on (MCP's CallToolResult). */
export type ToolResult = {
  content?: { type: string; text?: string }[];
  structuredContent?: Record<string, unknown>;
  isError?: boolean;
  _meta?: Record<string, unknown>;
};

/** What the host tells the view. */
export type BridgeEvents = {
  toolInput?: (args: Record<string, unknown>) => void;
  toolResult?: (result: ToolResult) => void;
  toolCancelled?: (reason?: string) => void;
  /** The host's whole context so far: `ui/initialize`'s, with each change merged in. */
  context?: (ctx: McpUiHostContext) => void;
  /** The host is about to remove the view (`ui/resource-teardown`). */
  teardown?: () => void;
};

type Message = { jsonrpc?: string; id?: string | number | null; method?: string; params?: unknown; result?: unknown; error?: { code: number; message: string } };

/** An answer that is an error, from the host. */
export class BridgeError extends Error {
  code: number;
  constructor(code: number, message: string) {
    super(message);
    this.code = code;
  }
}

/** How long a request waits for the host's answer: Claude ends a view's tool call after 45 s. */
const WAIT_MS = 60_000;

export class Bridge {
  private nextId = 1;
  private waiting = new Map<number, { resolve: (v: unknown) => void; reject: (e: Error) => void; timer: ReturnType<typeof setTimeout> }>();
  private ctx: McpUiHostContext = {};

  /** `host`: the window the host speaks from (the view's parent); `self`: where its messages come in. */
  constructor(private host: Window, private events: BridgeEvents, private self: Window = window) {
    self.addEventListener("message", this.onMessage);
  }

  /** The host's context so far. */
  get context(): McpUiHostContext {
    return this.ctx;
  }

  /** `ui/initialize`, then `ui/notifications/initialized`: the host sends the tool's input and result after it. */
  async connect(appInfo: { name: string; version: string }): Promise<McpUiInitializeResult> {
    const result = (await this.request("ui/initialize", {
      appInfo,
      appCapabilities: { availableDisplayModes: ["inline", "fullscreen"] },
      protocolVersion: PROTOCOL,
    })) as McpUiInitializeResult;
    this.ctx = { ...(result?.hostContext ?? {}) };
    this.events.context?.(this.ctx);
    this.notify("ui/notifications/initialized", {});
    return result;
  }

  /** One of the server's tools, through the host. */
  callTool(name: string, args: Record<string, unknown>): Promise<ToolResult> {
    return this.request("tools/call", { name, arguments: args }) as Promise<ToolResult>;
  }

  /** Asks for another display mode; the host answers with the one it set. Fullscreen only after a click or a key. */
  async requestDisplayMode(mode: McpUiDisplayMode): Promise<McpUiDisplayMode> {
    const r = (await this.request("ui/request-display-mode", { mode })) as { mode?: McpUiDisplayMode };
    return r?.mode ?? mode;
  }

  /** The view's size in pixels, so the host sizes its frame. */
  sizeChanged(width: number, height: number) {
    this.notify("ui/notifications/size-changed", { width, height });
  }

  /** Stops listening; requests still waiting fail. */
  close() {
    this.self.removeEventListener("message", this.onMessage);
    for (const [, w] of this.waiting) { clearTimeout(w.timer); w.reject(new Error("the view closed")); }
    this.waiting.clear();
  }

  private send(msg: Message) {
    this.host.postMessage(msg, "*");
  }

  private notify(method: string, params: unknown) {
    this.send({ jsonrpc: "2.0", method, params });
  }

  private request(method: string, params: unknown): Promise<unknown> {
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.waiting.delete(id);
        reject(new Error(`no answer to ${method} in ${WAIT_MS / 1000} s`));
      }, WAIT_MS);
      this.waiting.set(id, { resolve, reject, timer });
      this.send({ jsonrpc: "2.0", id, method, params });
    });
  }

  private onMessage = (e: MessageEvent) => {
    // Only the host's messages: anything else in the page (another frame, an extension) is ignored.
    if (e.source !== this.host) return;
    const m = e.data as Message;
    if (!m || typeof m !== "object" || m.jsonrpc !== "2.0") return;
    if (m.method === undefined) {
      // An answer to one of the view's requests.
      const w = typeof m.id === "number" ? this.waiting.get(m.id) : undefined;
      if (!w) return;
      this.waiting.delete(m.id as number);
      clearTimeout(w.timer);
      if (m.error) w.reject(new BridgeError(m.error.code, m.error.message));
      else w.resolve(m.result);
      return;
    }
    if (m.id !== undefined && m.id !== null) return this.answer(m.id, m.method);
    this.onNotification(m.method, m.params);
  };

  /** A request from the host: `ui/resource-teardown` and `ping` get an empty answer; anything else isn't known. */
  private answer(id: string | number, method: string) {
    if (method === "ui/resource-teardown") {
      this.events.teardown?.();
      return this.send({ jsonrpc: "2.0", id, result: {} });
    }
    if (method === "ping") return this.send({ jsonrpc: "2.0", id, result: {} });
    this.send({ jsonrpc: "2.0", id, error: { code: -32601, message: `method not found: ${method}` } });
  }

  private onNotification(method: string, params: unknown) {
    const p = (params ?? {}) as Record<string, unknown>;
    switch (method) {
      case "ui/notifications/tool-input":
        this.events.toolInput?.((p.arguments as Record<string, unknown>) ?? {});
        break;
      case "ui/notifications/tool-result":
        this.events.toolResult?.(p as ToolResult);
        break;
      case "ui/notifications/tool-cancelled":
        this.events.toolCancelled?.(typeof p.reason === "string" ? p.reason : undefined);
        break;
      case "ui/notifications/host-context-changed":
        this.ctx = { ...this.ctx, ...(p as McpUiHostContext) };
        this.events.context?.(this.ctx);
        break;
      // ui/notifications/tool-input-partial and the rest: nothing to do for the board.
    }
  }
}

/** Tells the host the view's size whenever it changes, as ext-apps' App does: once a frame at most, the document's
 *  height as its content makes it, the window's width. Returns a function that stops it. */
export function reportSize(bridge: Bridge, doc: Document = document, win: Window = window): () => void {
  let queued = false, lastW = 0, lastH = 0;
  const measure = () => {
    if (queued) return;
    queued = true;
    win.requestAnimationFrame(() => {
      queued = false;
      const html = doc.documentElement;
      const before = html.style.height;
      html.style.height = "max-content";
      const height = Math.ceil(html.getBoundingClientRect().height);
      html.style.height = before;
      const width = Math.ceil(win.innerWidth);
      if (width !== lastW || height !== lastH) {
        lastW = width;
        lastH = height;
        bridge.sizeChanged(width, height);
      }
    });
  };
  measure();
  const watch = new ResizeObserver(measure);
  watch.observe(doc.documentElement);
  watch.observe(doc.body);
  return () => watch.disconnect();
}
