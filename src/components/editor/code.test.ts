// GA-366 QA: a file's language reaches its tab's state when its chunk has loaded, which can be well after the file shows
// (the first file of a language, a busy machine). setLanguage gives it to the view when the view shows that state (the
// view draws the colours itself, so nothing else has to redraw: it says false), else to the kept state (it says true, so
// the page redraws and the view takes that state when it shows the tab).
import { describe, expect, it } from "vitest";
import { language as languageOf } from "@codemirror/language";
import type { EditorState, TransactionSpec } from "@codemirror/state";
import type { EditorView } from "@codemirror/view";
import { codeState, setLanguage } from "./code";

const on = { current: { save: () => {}, update: () => {} } };
/** A stand-in for the page's one EditorView: it shows a state and takes transactions. */
function viewShowing(state: EditorState) {
  const v = { state, dispatched: 0, dispatch(spec: TransactionSpec) { v.dispatched++; v.state = v.state.update(spec).state; } };
  return v;
}
const asView = (v: ReturnType<typeof viewShowing>) => v as unknown as EditorView;

describe("a file's language arriving (GA-366)", () => {
  it("for the state the view shows: the view takes it, nothing is kept, and it says the page needn't redraw", async () => {
    const s = codeState("# Kade portal\n\nLine three\n", false, on);
    expect(s.facet(languageOf)).toBeNull();
    const v = viewShowing(s);
    const kept: EditorState[] = [];
    expect(await setLanguage("README.md", asView(v), () => s, (n) => kept.push(n))).toBe(false);
    expect(kept).toEqual([]);
    expect(v.dispatched).toBe(1);
    expect(v.state.facet(languageOf)?.name).toBe("markdown");
    expect(v.state.doc).toBe(s.doc);
  });

  it("for the state the view shows after typing (the page keeps the view's latest state): the typed text stays", async () => {
    const opened = codeState("export const one = 1;\n", false, on);
    const v = viewShowing(opened);
    v.dispatch({ changes: { from: opened.doc.length, insert: "x" } });
    let latest = v.state;
    expect(await setLanguage("app.ts", asView(v), () => latest, (n) => { latest = n; })).toBe(false);
    expect(v.dispatched).toBe(2);
    expect(v.state.doc.toString()).toBe("export const one = 1;\nx");
    expect(v.state.facet(languageOf)?.name).toBe("typescript");
  });

  it("for a kept state the view doesn't show (another tab, or before the view took it): it goes there, and it says so", async () => {
    const s = codeState("export const one = 1;\n", false, on);
    const v = viewShowing(codeState("# another tab\n", false, on));
    const kept: EditorState[] = [];
    expect(await setLanguage("app.ts", asView(v), () => s, (n) => kept.push(n))).toBe(true);
    expect(v.dispatched).toBe(0);
    expect(v.state.facet(languageOf)).toBeNull();
    expect(kept).toHaveLength(1);
    expect(kept[0]!.facet(languageOf)?.name).toBe("typescript");
    expect(kept[0]!.doc).toBe(s.doc);

    // no view yet: kept as well
    const none: EditorState[] = [];
    expect(await setLanguage("README.md", null, () => s, (n) => none.push(n))).toBe(true);
    expect(none[0]!.facet(languageOf)?.name).toBe("markdown");
  });

  it("a file without a language, or a tab closed before its language came: nothing changes, and it says no redraw", async () => {
    const s = codeState("plain\n", false, on);
    const v = viewShowing(s);
    const kept: EditorState[] = [];
    expect(await setLanguage("notes.unknown-type", asView(v), () => s, (n) => kept.push(n))).toBe(false);
    expect(await setLanguage("app.ts", asView(v), () => undefined, (n) => kept.push(n))).toBe(false);
    expect(await setLanguage("app.ts", null, () => undefined, (n) => kept.push(n))).toBe(false);
    expect(kept).toEqual([]);
    expect(v.dispatched).toBe(0);
    expect(v.state).toBe(s);
  });
});
