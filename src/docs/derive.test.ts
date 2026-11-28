// The Docs page's routes and what it works out from the index: an address that holds on a reload, the Reference's
// filters, an endpoint's fields from its $refs, the search, and the sections a project switched off.
import { describe, expect, it } from "vitest";
import { docsPath, fresh, HOME, parseDocsPath } from "./route";
import { bodyOf, byMonth, dayLabel, fieldsOf, filtered, folderRow, isOff, monthLabel, railHeadings, search, searchDocs, snippet, tabs, withoutLeadHeading } from "./derive";
import { sampleIndex } from "./sampleIndex.test-util";

describe("a docs address", () => {
  it("names a section, its item with a line, and the filters, and writes them back the same", () => {
    const r = parseDocsPath("reference/app%2FUser.php%23User.owner@L6?kind=class,method&folder=app&described=no&changed=any&page=3");
    expect(r).toEqual({ tab: "reference", item: "app/User.php#User.owner", line: 6, kinds: ["class", "method"], folder: "app", described: "no", changed: "any", page: 3 });
    expect(parseDocsPath(docsPath(r, { page: 3, line: 6 }))).toEqual(r);
  });

  it("is Home when empty or unknown, and a change to the list goes back to page 1", () => {
    expect(parseDocsPath("")).toEqual(HOME);
    expect(parseDocsPath("nowhere/x").tab).toBe("home");
    const listed = parseDocsPath("reference?page=4");
    expect(docsPath(listed, { kinds: ["class"] })).toBe("reference?kind=class");
    expect(docsPath(listed, { page: 5 })).toBe("reference?page=5");
    expect(docsPath(listed, fresh("decisions", "0001"))).toBe("decisions/0001");
    expect(docsPath(listed, fresh("home"))).toBe("");
  });
});

describe("what the page works out", () => {
  it("filters the Reference by folder, kind, description and how it moved", () => {
    const index = sampleIndex();
    const names = (path: string) => filtered(index, parseDocsPath(path)).map((e) => e.name);
    expect(names("reference?folder=app/User.php")).toEqual(["User", "owner"]);
    expect(names("reference?kind=controller")).toEqual(["UserController"]);
    expect(names("reference?described=no")).toEqual(["owner", "UserController"]);
    expect(names("reference?changed=documented")).toEqual(["User"]);
    expect(folderRow(index, "app/User.php")?.label).toBe("User.php");
  });

  it("reads an endpoint's body and response fields through the spec's $refs", () => {
    const index = sampleIndex();
    const endpoint = index.api!.endpoints[0]!;
    expect(bodyOf(index, endpoint.requestBody)).toEqual({ type: "application/json", fields: [
      { name: "name", detail: "string", required: true, description: "Full name" },
      { name: "role", detail: "string · admin | member", required: false, description: "" }] });
    expect(fieldsOf(index, endpoint.responses[0]!.schema).map((f) => f.name)).toEqual(["name", "role"]);
    expect(fieldsOf(index, { $ref: "#/components/schemas/Nope" })).toEqual([]);
  });

  it("finds by name first, then path and body, and every word must match", () => {
    const docs = searchDocs(sampleIndex());
    expect(search(docs, "user")[0]?.title).toBe("User");
    expect(search(docs, "postgres").map((d) => d.section)).toEqual(["decisions"]);
    expect(search(docs, "csv invoices").map((d) => d.section)).toEqual(["changelog"]);
    expect(search(docs, "user zzz")).toEqual([]);
    expect(search(docs, "  ")).toEqual([]);
  });

  it("leaves a switched-off section out of the tabs and the search", () => {
    const index = sampleIndex({ switchedOff: [{ section: "decisions", key: "decisions" }] });
    expect(isOff(index, "decisions")).toBe(true);
    expect(tabs(index).map((t) => t.id)).toEqual(["home", "guides", "reference", "api", "changelog", "mcp"]);
    expect(search(searchDocs(index), "postgres")).toEqual([]);
  });

  it("cuts a description at its first sentence and drops a heading that repeats the title", () => {
    expect(snippet("A person who signs in. Owns invoices.")).toBe("A person who signs in.");
    expect(withoutLeadHeading('<h1 id="kade">Kade</h1><p>x</p>', "kade")).toBe("<p>x</p>");
    expect(withoutLeadHeading("<h1>Other</h1><p>x</p>", "Kade")).toBe("<h1>Other</h1><p>x</p>");
  });
});

describe("the decisions' dates", () => {
  it("names a month and a day the same way everywhere, and groups the decisions by month as they come", () => {
    expect(monthLabel("2026-11-25")).toBe("November 2026");
    expect(dayLabel("2026-11-05")).toBe("5 Nov");
    expect(dayLabel("2026-11-05", true)).toBe("5 November 2026");
    expect(monthLabel("")).toBe("No date");
    const d = (id: string, date: string) => ({ ...sampleIndex().decisions[0]!, id, date });
    expect(byMonth([d("a", "2026-11-25"), d("b", "2026-11-03"), d("c", "2026-10-30"), d("e", "")]).map(([m, ds]) => [m, ds.map((x) => x.id)]))
      .toEqual([["November 2026", ["a", "b"]], ["October 2026", ["c"]], ["No date", ["e"]]]);
  });
});

describe("a record's own heading", () => {
  it("is dropped when it repeats the title, whatever comes between its number and its words", () => {
    expect(withoutLeadHeading('<h1 id="a">0111 — CI op develop</h1><p>x</p>', "0111 · CI op develop")).toBe("<p>x</p>");
    expect(withoutLeadHeading("<h1>0110: De teams</h1><p>x</p>", "0110 · De teams")).toBe("<p>x</p>");
    expect(railHeadings([{ level: 1, text: "0111 — CI op develop" }, { level: 2, text: "Context" }], "0111 · CI op develop").map((h) => h.text)).toEqual(["Context"]);
  });
});
