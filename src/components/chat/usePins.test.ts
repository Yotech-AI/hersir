// GA-257 QA: usePins, the Chat page's pinned chats. Pin to the top puts the chat at the top of the list at once and asks the
// backend (pin_chat); Unpin takes it out at once (unpin_chat); a pin or unpin the backend refuses is undone (the list is
// read again) and says why. React's hooks are stand-ins here: useData hands over the list and records setData and
// reload, useState keeps the error, useMemo runs its function.
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ChatThread } from "../../types";

const h = vi.hoisted(() => ({
  data: null as ChatThread[] | null,
  setData: [] as ((was: ChatThread[] | null) => ChatThread[])[],
  reloads: 0,
  error: "",
  calls: [] as [string, string][],
  answer: Promise.resolve() as Promise<void>,
}));
vi.mock("react", async (orig) => {
  const R = await orig<typeof import("react")>();
  const useState = (init: unknown) => [h.error || init, (v: string) => { h.error = v; }];
  const useMemo = (f: () => unknown) => f();
  const useEffect = () => {};
  return { ...R, default: { ...R, useState, useMemo, useEffect }, useState, useMemo, useEffect };
});
vi.mock("../../lib/useData", () => ({
  useData: () => ({ data: h.data, error: null, reload: () => { h.reloads++; }, setData: (f: (was: ChatThread[] | null) => ChatThread[]) => { h.setData.push(f); } }),
}));
vi.mock("../../api", () => ({
  pinnedChats: () => new Promise(() => {}),
  pinChat: (id: string) => { h.calls.push(["pinChat", id]); return h.answer; },
  unpinChat: (id: string) => { h.calls.push(["unpinChat", id]); return h.answer; },
}));

const { usePins } = await import("./usePins");

const chat = (n: number): ChatThread => ({ id: `c${n}`, agentId: "lead", title: `Chat ${n}`, createdAt: n, updatedAt: n, costUsdMicros: 0, inputTokens: 0, outputTokens: 0 });
const settle = () => new Promise((r) => setTimeout(r, 0));

beforeEach(() => {
  h.data = null;
  h.setData = [];
  h.reloads = 0;
  h.error = "";
  h.calls = [];
  h.answer = Promise.resolve();
});

describe("usePins", () => {
  it("is empty until the backend answers", () => {
    const p = usePins();
    expect(p.list).toEqual([]);
    expect(p.has("c1")).toBe(false);
    expect(p.error).toBe("");
  });

  it("lists the pinned chats as the backend gives them and knows which are pinned", () => {
    h.data = [chat(3), chat(1)];
    const p = usePins();
    expect(p.list.map((t) => t.id)).toEqual(["c3", "c1"]);
    expect(p.has("c1")).toBe(true);
    expect(p.has("c2")).toBe(false);
  });

  it("Pin to the top: the chat goes to the top of the list at once, and the backend is asked", async () => {
    h.data = [chat(3), chat(1)];
    usePins().toggle(chat(7));
    expect(h.calls).toEqual([["pinChat", "c7"]]);
    expect(h.setData).toHaveLength(1);
    expect(h.setData[0](h.data).map((t) => t.id)).toEqual(["c7", "c3", "c1"]);
    // also before the first answer came
    expect(h.setData[0](null).map((t) => t.id)).toEqual(["c7"]);
    await settle();
    expect(h.reloads).toBe(0);
    expect(h.error).toBe("");
  });

  it("Unpin: the chat leaves the list at once, the others keep their order", async () => {
    h.data = [chat(3), chat(7), chat(1)];
    usePins().toggle(chat(7));
    expect(h.calls).toEqual([["unpinChat", "c7"]]);
    expect(h.setData[0](h.data).map((t) => t.id)).toEqual(["c3", "c1"]);
    await settle();
    expect(h.error).toBe("");
  });

  it("a pin the backend refuses is undone (the list is read again) and says why", async () => {
    h.data = [chat(3)];
    h.answer = Promise.reject("not found: chat c9");
    h.answer.catch(() => {});
    usePins().toggle(chat(9));
    await settle();
    expect(h.reloads).toBe(1);
    expect(h.error).toBe("Couldn't pin the chat: not found: chat c9");
    expect(usePins().error).toBe("Couldn't pin the chat: not found: chat c9");
  });

  it("an unpin the backend refuses is undone and says so", async () => {
    h.data = [chat(3)];
    h.answer = Promise.reject("database is locked");
    h.answer.catch(() => {});
    usePins().toggle(chat(3));
    await settle();
    expect(h.reloads).toBe(1);
    expect(h.error).toBe("Couldn't unpin the chat: database is locked");
  });
});
