// GA-287 QA: the @ chip's hover card, kind by kind (src/components/ItemHoverCard.tsx), against GA-284's README
// (design/islands/README.md → The chip's hover card, and Every text, word for word). Rendered to HTML on the server; the
// portal on body is the card itself here (no DOM), and where it goes is src/lib/itemPreview.test.ts's.
import { describe, expect, it, vi } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import type { Client, Doc, Member, Person, Project, Task } from "../types";
import type { ItemKind } from "../lib/itemLinks";
import type { ItemPreview } from "../lib/itemPreview";

vi.mock("react-dom", async (orig) => {
  const real = await orig<typeof import("react-dom")>();
  const createPortal = (children: unknown) => children;
  return { ...real, default: { ...real, createPortal }, createPortal };
});
vi.mock("../api", async (orig) => {
  const real = await orig<Record<string, unknown>>();
  const out: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(real)) out[k] = typeof v !== "function" ? v : () => new Promise(() => {});
  return out;
});

const { ItemHoverCard } = await import("./ItemHoverCard");
const { ItemChip, LinkedText, MarkdownView } = await import("./MarkdownView");

const task = (o: Partial<Task> = {}) => ({ id: "t-12", identifier: "GA-284", title: "Design: Hersir in islands (1 of 4): the look, the window, and a sidebar that closes", projectName: "Hersir", projectColor: "#4c8dff",
  stateName: "In progress", stateCategory: "in_progress", assigneeName: "Design Agent", assigneeKind: "agent", hold: null, ...o }) as Task;
const project = (o: Partial<Project> = {}) => ({ id: "p-1", key: "KP", name: "Kade portal", status: "active", color: "#e5484d", clientName: "Kade", ...o }) as Project;
const agent = (o: Partial<Member> = {}) => ({ actorId: "a-1", name: "Design Agent", kind: "agent", roleKey: "design", status: "active", ...o }) as Member;
const person = (o: Partial<Person> = {}) => ({ id: "u-1", name: "Sanne de Vries", handle: "sanne", email: "sanne.de.vries@example.com", openTasks: 0, ...o }) as Person;
const client = (o: Partial<Client> = {}) => ({ id: "c-1", name: "Kade", status: "lead", ...o }) as Client;
const doc = (o: Partial<Doc> = {}) => ({ id: "d-1", projectId: "p-1", title: "Release checklist", bodyMd: "", currentVersion: 1, updatedAt: 0, ...o }) as Doc;

/** The card's HTML (`document.body`, the portal's place, is only named: the portal is the card itself here). */
const card = (kind: ItemKind, item: ItemPreview | undefined, text = "the chip's text") => {
  vi.stubGlobal("document", { body: {} });
  try {
    return renderToStaticMarkup(<ItemHoverCard id="hc1" kind={kind} text={text} item={item} anchor={() => undefined} onEnter={() => {}} onLeave={() => {}} />);
  } finally {
    vi.unstubAllGlobals();
  }
};
/** The card's words, tag by tag. */
const words = (html: string) => html.replace(/<title>[^<]*<\/title>/g, "").replace(/<[^>]+>/g, "|").split("|").map((s) => s.trim()).filter(Boolean);
/** The row labels, in order. */
const labels = (html: string) => [...html.matchAll(/<dt>([^<]*)<\/dt>/g)].map((m) => m[1]);
/** One row's value (what its dd holds). */
const row = (html: string, label: string) => html.match(new RegExp(`<dt>${label}</dt><dd>(.*?)</dd>`))?.[1] ?? "";
const FOOT = '<p class="t-caption chip-card-foot">Click to open</p>';
const endsWithFoot = (html: string) => html.endsWith(`${FOOT}</div>`);

describe("the card", () => {
  it("is a tooltip (role, id) with the chip-card look, not a button, and hidden until it is placed", () => {
    const html = card("task", { kind: "task", task: task() });
    expect(html).toMatch(/^<div id="hc1" role="tooltip" class="chip-card" style="visibility:hidden">/);
    expect(html).not.toMatch(/<button|tabindex=/);
  });
});

describe("a card's chip", () => {
  it("shows the id above the full title, then Project, Agent and Status, and Click to open last", () => {
    const html = card("task", { kind: "task", task: task() });
    expect(html).toContain('<div class="chip-card-head"><span class="t-id chip-card-id">GA-284</span><div class="chip-card-title"><span class="t-body-strong chip-card-name">Design: Hersir in islands (1 of 4): the look, the window, and a sidebar that closes</span></div></div>');
    expect(labels(html)).toEqual(["Project", "Agent", "Status"]);
    expect(endsWithFoot(html)).toBe(true);
  });

  it("Project: the project's colour dot and name", () => {
    expect(row(card("task", { kind: "task", task: task() }), "Project"))
      .toBe('<span class="chip-card-val"><span class="chip-card-dot" style="background:#4c8dff"></span><span>Hersir</span></span>');
  });

  it("Agent: whoever it is assigned to, an agent or a person, with an avatar", () => {
    expect(row(card("task", { kind: "task", task: task() }), "Agent")).toMatch(/^<span class="chip-card-val"><span class="avatar agent sm" title="Design Agent">DA<\/span><span>Design Agent<\/span><\/span>$/);
    expect(row(card("task", { kind: "task", task: task({ assigneeName: "Sanne de Vries", assigneeKind: "user" }) }), "Agent"))
      .toMatch(/^<span class="chip-card-val"><span class="avatar sm" title="Sanne de Vries">SV<\/span><span>Sanne de Vries<\/span><\/span>$/);
  });

  it("Agent: 'Not assigned', faint, when nobody is", () => {
    expect(row(card("task", { kind: "task", task: task({ assigneeName: null, assigneeKind: null }) }), "Agent"))
      .toBe('<span class="chip-card-val none"><span>Not assigned</span></span>');
  });

  it("Status: the column's glyph and name, and no hold line when it isn't held", () => {
    const html = card("task", { kind: "task", task: task() });
    expect(row(html, "Status")).toBe('<span class="chip-card-val"><svg class="st st-in_progress" viewBox="0 0 16 16" fill="none" role="img" aria-label="In progress"><title>In progress</title>' +
      row(html, "Status").match(/<title>In progress<\/title>(.*?)<\/svg>/)![1] + "</svg><span>In progress</span></span>");
    expect(html).not.toContain("On hold");
    for (const [category, name] of [["backlog", "Backlog"], ["ready", "To do"], ["testing", "Testing"], ["review", "Review"], ["deploy", "Deploy"], ["done", "Done"]]) {
      expect(row(card("task", { kind: "task", task: task({ stateCategory: category, stateName: name }) }), "Status")).toContain(`<svg class="st st-${category}"`);
    }
  });

  it("Status of a held card: the column's glyph still, and a second line 'On hold:' with the hold's name, in needs", () => {
    const held = card("task", { kind: "task", task: task({ hold: "needs_decision", holdReason: "Which colour for the dot?" }) });
    expect(row(held, "Status")).toMatch(/<svg class="st st-in_progress".*<span>In progress<\/span><\/span><span class="chip-card-hold">On hold: Needs your decision<\/span>$/);
    for (const [hold, name] of [["stalled", "Stalled"], ["merge_conflict", "Merge conflict"], ["waiting_approval", "Waiting for approval"], ["rate_limited", "Rate limited"], ["blocked", "Blocked"], ["something_new", "something_new"]]) {
      expect(card("task", { kind: "task", task: task({ hold }) })).toContain(`<span class="chip-card-hold">On hold: ${name}</span>`);
    }
  });

  it("with the Chief: 'On hold: With the Chief'", () => {
    expect(card("task", { kind: "task", task: task({ hold: "needs_decision", withLead: true }) })).toContain('<span class="chip-card-hold">On hold: With the Chief</span>');
  });

  it("a card without a project says None", () => {
    expect(row(card("task", { kind: "task", task: task({ projectName: null }) }), "Project")).toBe('<span class="chip-card-val none"><span>None</span></span>');
  });
});

describe("a project's chip", () => {
  it("shows the colour dot, name and key, then Client and Status", () => {
    const html = card("project", { kind: "project", project: project() });
    expect(html).toContain('<div class="chip-card-title"><span class="chip-card-dot" style="background:#e5484d"></span><span class="t-body-strong chip-card-name">Kade portal<span class="t-id chip-card-key">KP</span></span></div>');
    expect(labels(html)).toEqual(["Client", "Status"]);
    expect(row(html, "Client")).toBe('<span class="chip-card-val"><span>Kade</span></span>');
    expect(row(html, "Status")).toBe('<span class="chip-card-val"><span>Active</span></span>');
    expect(endsWithFoot(html)).toBe(true);
  });

  it("Client None for a project without one; each status as the app names it", () => {
    expect(row(card("project", { kind: "project", project: project({ clientName: null }) }), "Client")).toBe('<span class="chip-card-val none"><span>None</span></span>');
    for (const [status, name] of [["planned", "Planned"], ["active", "Active"], ["paused", "Paused"], ["done", "Done"], ["archived", "Archived"]]) {
      expect(row(card("project", { kind: "project", project: project({ status }) }), "Status")).toBe(`<span class="chip-card-val"><span>${name}</span></span>`);
    }
  });
});

describe("an agent's chip", () => {
  it("shows the avatar and name, then Role and Status: Active", () => {
    const html = card("agent", { kind: "agent", agent: agent(), workingOn: undefined });
    expect(html).toContain('<div class="chip-card-title"><span class="avatar agent sm" title="Design Agent">DA</span><span class="t-body-strong chip-card-name">Design Agent</span></div>');
    expect(labels(html)).toEqual(["Role", "Status"]);
    expect(row(html, "Role")).toBe('<span class="chip-card-val"><span>Design</span></span>');
    expect(row(html, "Status")).toBe('<span class="chip-card-val"><span>Active</span></span>');
    expect(endsWithFoot(html)).toBe(true);
  });

  it("Status: Paused", () => {
    expect(row(card("agent", { kind: "agent", agent: agent({ status: "paused" }), workingOn: undefined }), "Status")).toBe('<span class="chip-card-val"><span>Paused</span></span>');
  });

  it("Status: the live dot and 'Working on GA-12', in live", () => {
    expect(row(card("agent", { kind: "agent", agent: agent(), workingOn: "GA-12" }), "Status"))
      .toBe('<span class="chip-card-val live"><span class="chip-card-live"></span><span>Working on GA-12</span></span>');
  });

  it("Role: each role by its name", () => {
    expect(row(card("agent", { kind: "agent", agent: agent({ roleKey: "qa" }), workingOn: undefined }), "Role")).toMatch(/<span>(QA|Qa)[^<]*<\/span>/);
    expect(row(card("agent", { kind: "agent", agent: agent({ roleKey: "frontend" }), workingOn: undefined }), "Role")).toBe('<span class="chip-card-val"><span>Frontend</span></span>');
  });
});

describe("a client's chip", () => {
  it("shows its name and Status: Lead, Active or Inactive", () => {
    const html = card("client", { kind: "client", client: client() });
    expect(html).toContain('<span class="t-body-strong chip-card-name">Kade</span>');
    expect(labels(html)).toEqual(["Status"]);
    expect(endsWithFoot(html)).toBe(true);
    for (const [status, name] of [["lead", "Lead"], ["active", "Active"], ["inactive", "Inactive"]]) {
      expect(row(card("client", { kind: "client", client: client({ status }) }), "Status")).toBe(`<span class="chip-card-val"><span>${name}</span></span>`);
    }
  });
});

describe("a person's chip", () => {
  it("shows the avatar and name, then Email, which breaks anywhere", () => {
    const html = card("person", { kind: "person", person: person() });
    expect(html).toContain('<div class="chip-card-title"><span class="avatar sm" title="Sanne de Vries">SV</span><span class="t-body-strong chip-card-name">Sanne de Vries</span></div>');
    expect(labels(html)).toEqual(["Email"]);
    expect(row(html, "Email")).toBe('<span class="chip-card-val anywhere"><span>sanne.de.vries@example.com</span></span>');
    expect(endsWithFoot(html)).toBe(true);
  });

  it("Email None when there is none", () => {
    expect(row(card("person", { kind: "person", person: person({ email: null }) }), "Email")).toBe('<span class="chip-card-val none"><span>None</span></span>');
  });
});

describe("a doc's chip", () => {
  it("shows its title, then Project with the colour dot, or None", () => {
    const html = card("doc", { kind: "doc", doc: doc(), project: project() });
    expect(html).toContain('<span class="t-body-strong chip-card-name">Release checklist</span>');
    expect(labels(html)).toEqual(["Project"]);
    expect(row(html, "Project")).toBe('<span class="chip-card-val"><span class="chip-card-dot" style="background:#e5484d"></span><span>Kade portal</span></span>');
    expect(endsWithFoot(html)).toBe(true);
    expect(row(card("doc", { kind: "doc", doc: doc({ projectId: null }), project: undefined }), "Project")).toBe('<span class="chip-card-val none"><span>None</span></span>');
  });
});

describe("a chip whose item doesn't exist", () => {
  it("shows the chip's text and 'Not found': no rows and no Click to open", () => {
    const html = card("task", { kind: "missing" }, "GA-999");
    expect(words(html)).toEqual(["GA-999", "Not found"]);
    expect(html).toContain('<p class="t-small chip-card-note">Not found</p>');
    expect(html).not.toContain("<dl");
    expect(html).not.toContain("Click to open");
  });
});

describe("while the item is on its way", () => {
  it("shows the chip's text and its kind, and Click to open", () => {
    for (const [kind, name] of [["task", "Task"], ["project", "Project"], ["client", "Client"], ["agent", "Agent"], ["person", "Person"], ["doc", "Doc"]] as const) {
      expect(words(card(kind, undefined, "GA-12 - Fix the login"))).toEqual(["GA-12 - Fix the login", name, "Click to open"]);
    }
  });
});

describe("every text, word for word", () => {
  it("uses only the README's row labels", () => {
    const all = [
      card("task", { kind: "task", task: task() }), card("project", { kind: "project", project: project() }),
      card("agent", { kind: "agent", agent: agent(), workingOn: undefined }), card("client", { kind: "client", client: client() }),
      card("person", { kind: "person", person: person() }), card("doc", { kind: "doc", doc: doc(), project: project() }),
    ].flatMap(labels);
    expect(new Set(all)).toEqual(new Set(["Project", "Agent", "Status", "Client", "Role", "Email"]));
  });
});

describe("the chip", () => {
  it("has no title, so the browser's 'Open task' no longer shows, and no card until the pointer rests on it", () => {
    for (const kind of ["task", "project", "client", "agent", "person", "doc"] as const) {
      const html = renderToStaticMarkup(<ItemChip kind={kind} itemKey="K-1">The item</ItemChip>);
      expect(html).toBe(`<a class="item-chip kind-${kind}" href="gizai:${kind}/K-1">The item</a>`);
    }
  });

  it("in a message, a description or a note: no title on any chip", () => {
    const html = renderToStaticMarkup(<MarkdownView md="See [GA-12 - Fix](gizai:task/GA-12), [Kade](gizai:client/c-1) and [Doc](gizai:doc/d-1)" />)
      + renderToStaticMarkup(<LinkedText text="and [Design Agent](gizai:agent/a-1)" />);
    expect(html.match(/class="item-chip/g)).toHaveLength(4);
    expect(html).not.toMatch(/title="Open /);
    expect(html).not.toContain("chip-card");
  });
});
