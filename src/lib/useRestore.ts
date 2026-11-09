import { useEffect, useState } from "react";
import { onRestoreChanged, restoreStatus } from "../api";
import type { RestoreStatus } from "../types";

/** What a restore is doing, and how the last one went at the start: read once, then as `restore-changed` says it. The
 *  commands' own answers aren't used: each change goes out in the event too, and an answer can be older than an event that
 *  came first (a small snapshot can be ready before start_restore's answer arrives). */
export function useRestore(): RestoreStatus | null {
  const [status, setStatus] = useState<RestoreStatus | null>(null);
  useEffect(() => {
    let alive = true;
    let heard = false;
    let un: (() => void) | undefined;
    onRestoreChanged((s) => { if (alive) { heard = true; setStatus(s); } }).then((f) => (alive ? (un = f) : f())).catch(() => {});
    restoreStatus().then((s) => { if (alive && !heard) setStatus(s); }).catch(() => {});
    return () => { alive = false; un?.(); };
  }, []);
  return status;
}
