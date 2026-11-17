// GA-282 QA: Hersir's logs in the browser preview. src/browser/fixtures.json has get_logs_info (GA-281) with a logs folder
// and no unexpected end, so the preview shows Settings → General → Data → Hersir's logs with that folder and no notice
// above the page. Open log folder and Dismiss (open_logs_folder, dismiss_unexpected_end) have no fixture: they answer null,
// as a void command does, and don't fail.
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { LogsInfo } from "../types";

vi.mock("@tauri-apps/api/event", () => ({ emit: async () => {} }));
vi.mock("./pictures", () => ({ screenshotPicture: async () => new ArrayBuffer(0) }));

const fixtures = (await import("./fixtures.json")).default as unknown as import("./mock").Fixtures;
const { answer, loadFixtures } = await import("./mock");

beforeEach(() => {
  vi.spyOn(console, "info").mockImplementation(() => {});
  vi.spyOn(console, "warn").mockImplementation(() => {});
  loadFixtures(structuredClone(fixtures));
});
afterEach(() => { vi.restoreAllMocks(); });

describe("get_logs_info in the browser preview", () => {
  it("answers the logs folder and this start's log, in LogsInfo's shape, with no unexpected end (so no notice)", async () => {
    const info = (await answer("get_logs_info", {})) as LogsInfo;
    expect(Object.keys(info).sort()).toEqual(["current", "folder", "unexpectedEnd"]);
    expect(info.unexpectedEnd).toBeNull();
    expect(info.folder).toMatch(/\/logs$/);
    expect(info.current.startsWith(`${info.folder}/hersir-`)).toBe(true);
    expect(info.current).toMatch(/\.log$/);
  });
  it("Open log folder and Dismiss answer null without failing", async () => {
    await expect(answer("open_logs_folder", {})).resolves.toBeNull();
    await expect(answer("dismiss_unexpected_end", {})).resolves.toBeNull();
  });
});
