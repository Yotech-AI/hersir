// GA-340 QA: Settings → MCP servers → Claude Desktop in the browser mode. src/browser/fixtures.json has claude_desktop_status
// in GA-338's ClaudeDesktopStatus shape (written by crates/gizai-core/examples/fixtures.rs: not connected, Claude Desktop's
// settings file found without the Hersir entry, hersir-mcp found), and the fake backend in mock.ts answers Connect (a
// sample entry with a made-up key, new each time, added to the settings only with addToConfig) and Disconnect (the key
// ends, the settings keep the entry), as GA-338's commands do.
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ClaudeDesktopConnection, ClaudeDesktopStatus } from "../types";

vi.mock("@tauri-apps/api/event", () => ({ emit: async () => {} }));
vi.mock("./pictures", () => ({ screenshotPicture: async () => new ArrayBuffer(0) }));

const fixtures = (await import("./fixtures.json")).default as unknown as import("./mock").Fixtures;
const { answer, loadFixtures } = await import("./mock");

const CONFIG = "/home/you/.config/Claude/claude_desktop_config.json";
const SHIM = "/home/you/.local/lib/hersir/hersir-mcp";
const status = async () => (await answer("claude_desktop_status", {})) as ClaudeDesktopStatus;
const connect = async (addToConfig: boolean) => (await answer("claude_desktop_connect", { addToConfig })) as ClaudeDesktopConnection;
const disconnect = async () => (await answer("claude_desktop_disconnect", {})) as ClaudeDesktopStatus;
const keyOf = (c: ClaudeDesktopConnection) => (JSON.parse(c.entry) as { mcpServers: { hersir: { env: Record<string, string> } } }).mcpServers.hersir.env;

beforeEach(() => {
  vi.spyOn(console, "info").mockImplementation(() => {});
  vi.spyOn(console, "warn").mockImplementation(() => {});
  loadFixtures(structuredClone(fixtures));
});

describe("the fixture (fixtures.rs → fixtures.json)", () => {
  it("claude_desktop_status: not connected at first, the settings file found without the entry, hersir-mcp found", async () => {
    expect(await status()).toEqual({ connected: false, connectedAt: null, configPath: CONFIG, configHasHersir: false, shimPath: SHIM });
  });
});

describe("Connect", () => {
  it("without addToConfig: a sample entry to paste, with a made-up key, and connected from now", async () => {
    const before = Date.now();
    const c = await connect(false);
    expect(Object.keys(c).sort()).toEqual(["backupPath", "configError", "configPath", "entry", "status", "wroteConfig"]);
    expect(c).toMatchObject({ wroteConfig: false, configPath: CONFIG, backupPath: null, configError: null });
    const parsed = JSON.parse(c.entry) as { mcpServers: Record<string, { command: string }> };
    expect(Object.keys(parsed.mcpServers)).toEqual(["hersir"]);
    expect(parsed.mcpServers.hersir.command).toBe(SHIM);
    const env = keyOf(c);
    const keys = Object.values(env);
    expect(keys).toHaveLength(1);
    expect(keys[0]).toMatch(/^browser-demo-[0-9a-f]{32}$/);
    expect(c.status).toMatchObject({ connected: true, configPath: CONFIG, configHasHersir: false, shimPath: SHIM });
    expect(c.status.connectedAt).toBeGreaterThanOrEqual(before);
    expect(c.status.connectedAt).toBeLessThanOrEqual(Date.now());
    expect(await status()).toEqual(c.status);
  });

  it("makes a new key each time (Connect again)", async () => {
    const [a, b] = [await connect(false), await connect(false)];
    expect(Object.values(keyOf(a))[0]).not.toBe(Object.values(keyOf(b))[0]);
  });

  it("with addToConfig: says it added the entry to the settings, with a backup of the old file, and the settings have it now", async () => {
    const c = await connect(true);
    expect(c.wroteConfig).toBe(true);
    expect(c.configPath).toBe(CONFIG);
    expect(c.backupPath).toEqual(expect.any(String));
    expect(c.configError).toBeNull();
    expect(c.status).toMatchObject({ connected: true, configHasHersir: true });
    expect((await status()).configHasHersir).toBe(true);
  });

  it("answers copies: changing an answer doesn't change the next", async () => {
    const s = await status();
    s.connected = true;
    expect((await status()).connected).toBe(false);
  });
});

describe("Disconnect", () => {
  it("ends the connection, and the settings keep the entry", async () => {
    await connect(true);
    const s = await disconnect();
    expect(s).toEqual({ connected: false, connectedAt: null, configPath: CONFIG, configHasHersir: true, shimPath: SHIM });
    expect(await status()).toEqual(s);
  });

  it("after Disconnect, Connect works again", async () => {
    await connect(false);
    await disconnect();
    expect((await connect(false)).status.connected).toBe(true);
  });
});
