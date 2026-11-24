// GA-331: a project's page has two Island tabs in its island's header, Overview (the page as it was) and Time tracking.
// Rendered to HTML on the server: useData hands over the project at once; the Time tracking tab is a stand-in here
// (ProjectTime.test.tsx tests it); a tab's click goes to its address, so Back and Forward go between them.
import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import type { Project } from "../types";

const state = vi.hoisted(() => ({ went: [] as unknown[], tabs: null as null | Record<string, unknown>, PROJECT: Symbol("project") }));
vi.mock("../api", async (orig) => ({ ...(await orig<typeof import("../api")>()),
  getProject: () => state.PROJECT, listDocs: () => [], listTeams: () => [] }));
vi.mock("../lib/useData", () => ({
  useData: (load: () => unknown) => {
    const got = load();
    return { data: got === state.PROJECT ? project : got, error: null, reload: () => {}, setData: () => {} };
  },
}));
vi.mock("../lib/drawers", () => ({ useDrawer: () => () => {} }));
vi.mock("../router", async (orig) => ({ ...(await orig<typeof import("../router")>()), go: (r: unknown) => { state.went.push(r); } }));
vi.mock("../components/FileDrop", () => ({ FileDrop: () => <div className="file-drop-stand-in" /> }));
vi.mock("../components/ProjectTime", () => ({ ProjectTime: ({ projectId }: { projectId: string }) => <div className="time-tab">Time of {projectId}</div> }));
vi.mock("../components/Tabs", async (orig) => {
  const m = await orig<typeof import("../components/Tabs")>();
  return { ...m, Tabs: (p: Parameters<typeof m.Tabs>[0]) => { state.tabs = p as unknown as Record<string, unknown>; return m.Tabs(p); } };
});

const project: Project = {
  id: "p-kade", number: "P-1", key: "KADE", name: "Kade portal", status: "active", defaultBranch: "main", openTasks: 3, doneTasks: 1, updatedAt: 0,
  aiCostUsdMicros: 0, aiUnknownCostRuns: 0, worktreeCopy: [], worktreeInstall: false, goalMd: "Ship the portal.",
  screenshots: { on: true, sizes: [], bothThemes: false, fullPage: false }, previewCommand: null, previewStartSecs: 120,
};

const { ProjectPage } = await import("./ProjectPage");
const { href } = await import("../router");

const text = (html: string) => html.replace(/<[^>]+>/g, "|").replace(/\|+/g, "|");

beforeEach(() => { state.went = []; state.tabs = null; });

describe("a project's tabs", () => {
  it("are Overview and Time tracking, Island tabs in the island's header under the top bar, and no toolbar", () => {
    const html = renderToStaticMarkup(<ProjectPage id="p-kade" />);
    const head = html.slice(html.indexOf('<div class="island-head project-head">'), html.indexOf('<div class="content">'));
    expect(html.indexOf('<div class="topbar">')).toBeLessThan(html.indexOf('<div class="island-head project-head">'));
    expect(head.startsWith('<div class="island-head project-head"><div class="tabs island-tabs" role="tablist" aria-label="Project">')).toBe(true);
    expect(head.match(/<button class="tab" role="tab"/g)).toHaveLength(2);
    expect(text(head)).toContain("|Overview|Time tracking|");
    expect(head).toMatch(/aria-selected="true"><svg[^>]*lucide-layout-dashboard[^>]*>[\s\S]*?<\/svg>Overview<\/button>/);
    expect(head).toMatch(/aria-selected="false"><svg[^>]*lucide-timer[^>]*>[\s\S]*?<\/svg>Time tracking<\/button>/);
    expect(html).not.toContain('class="toolbar');
  });

  it("opens on Overview: the page as it was, and no Time tracking list", () => {
    const html = renderToStaticMarkup(<ProjectPage id="p-kade" />);
    const t = text(html);
    for (const s of ["|Kade portal|", "|Open tasks|", "|New task|", "|Edit|", "|Goal|", "|Ship the portal.|", "|Docs|", "|Files|", "|Tasks|", "|1 / 4|",
      "|Repository|", "|Budget|"]) expect(t).toContain(s);
    expect(html).toContain('class="file-drop-stand-in"');
    expect(html).not.toContain("time-tab");
  });

  it("on Time tracking shows the project's list instead of the Overview, the same top bar and tabs above it", () => {
    const html = renderToStaticMarkup(<ProjectPage id="p-kade" view="time" />);
    expect(html).toContain('<div class="time-tab">Time of p-kade</div>');
    expect(html).toMatch(/aria-selected="true"><svg[^>]*lucide-timer[^>]*>[\s\S]*?<\/svg>Time tracking<\/button>/);
    expect(text(html)).toContain("|Projects|/|Kade portal|");
    for (const s of ["|Goal|", "|Repository|", "|Budget|", "|New task|"]) expect(text(html)).not.toContain(s);
  });

  it("a tab goes to its address: #/project/<id>/time for Time tracking, #/project/<id> for Overview", () => {
    renderToStaticMarkup(<ProjectPage id="p-kade" />);
    const onOpen = state.tabs!.onOpen as (t: string) => void;
    onOpen("time");
    onOpen("overview");
    expect(state.went.map((r) => href(r as Parameters<typeof href>[0]))).toEqual(["#/project/p-kade/time", "#/project/p-kade"]);
  });
});
