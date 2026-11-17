// GA-300: a row of controls that folds to fit its width, such as the Tasks page's toolbar in a 900px window. Level 0 is the
// row as drawn; each level folds a little more, as the row's CSS says for its `data-fold` attribute ("1", "2"). The row
// takes the least level at which nothing sticks out, measured before it is drawn: when what it holds changes (`content`),
// when its size changes and once the fonts are in.
import { useLayoutEffect, useState, type RefObject } from "react";

/** Sets the row's `data-fold` to `level` (none at 0). */
function foldAt(el: HTMLElement, level: number) {
  if (level) el.dataset.fold = String(level);
  else delete el.dataset.fold;
}

/** The least level, from 0 to `most`, at which the row's controls fit in it; `most` when none does. */
export function fitLevel(el: HTMLElement, most: number): number {
  let level = 0;
  for (;;) {
    foldAt(el, level);
    if (level >= most || el.scrollWidth <= el.clientWidth) return level;
    level += 1;
  }
}

/** How far the row in `ref` folds (render it as its `data-fold`, none at 0). `content` is anything that changes what the
 *  row holds (a button more, a longer label): a change measures it again. */
export function useFold(ref: RefObject<HTMLElement | null>, most: number, content: string): number {
  const [fold, setFold] = useState(0);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    let gone = false;
    const fit = () => { if (!gone) setFold(fitLevel(el, most)); };
    fit();
    document.fonts?.ready.then(fit).catch(() => {});
    if (typeof ResizeObserver === "undefined") return () => { gone = true; };
    const watch = new ResizeObserver(fit);
    watch.observe(el);
    return () => { gone = true; watch.disconnect(); };
  }, [ref, most, content]);
  return fold;
}
