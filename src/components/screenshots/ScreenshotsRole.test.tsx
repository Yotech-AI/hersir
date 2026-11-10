// GA-254 QA: a screenshot set's role icon is labelled with the role's name, so a screen reader says Chief for the Chief's
// set, not its key lead, and QA for a qa set. Rendered on the server: effects don't run, so nothing loads.
import { describe, expect, it, vi } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import type { ScreenshotSet, Task } from "../../types";

vi.mock("../../api", () => ({ agentCliTools: () => Promise.resolve({ browser: {} }), screenshotBytes: () => Promise.resolve(new Uint8Array()) }));

const { ScreenshotsView } = await import("./ScreenshotsView");

const task = { id: "t1", identifier: "GA-1", projectId: "p1", title: "Card", descriptionMd: "", stateId: "s1", stateName: "Review", stateCategory: "review",
  priority: 0, labels: [], bounceCount: 0, failCount: 0, sortKey: "a0", testing: true, createdAt: 0, updatedAt: 0, byHand: false, screenshots: null } as Task;
const set = (runId: string, agentName: string, role: string | null): ScreenshotSet => ({
  runId, agentId: `a-${runId}`, agentName, role, outcome: "qa_pass", status: "succeeded", endedAt: 1_700_000_000_000, commit: null, shots: [],
  leftOut: [], preview: null,
});
const render = (sets: ScreenshotSet[]) => renderToStaticMarkup(
  <ScreenshotsView task={task} sets={sets} error={null} project={null} agent={null} onShowProps={() => {}} onViewRun={() => {}} />,
);
const roleLabels = (html: string) => [...html.matchAll(/<svg[^>]*class="[^"]*\bicon role\b[^"]*"[^>]*aria-label="([^"]*)"/g)].map((m) => m[1]);

describe("a screenshot set's role icon", () => {
  it("is labelled Chief for the lead's set, not lead", () => {
    const html = render([set("r1", "Chief", "lead")]);
    expect(roleLabels(html)).toEqual(["Chief"]);
    expect(html).not.toContain('aria-label="lead"');
  });

  it("is labelled with each role's name", () => {
    expect(roleLabels(render([set("r1", "QA Agent", "qa"), set("r2", "Frontend Agent", "frontend")]))).toEqual(["QA", "Frontend"]);
  });

  it("isn't there for a set without a role", () => {
    expect(roleLabels(render([set("r1", "Someone", null)]))).toEqual([]);
  });
});
