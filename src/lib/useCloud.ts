import { useEffect, useRef, useState } from "react";
import { cloudStatus, onCloudChanged } from "../api";
import type { CloudStatus } from "../types";

/** Hersir Cloud's status (GA-360): read once, then as `cloud-changed` says it. `answer` runs a command that answers a status
 *  and takes that answer, unless an event came while it ran: the event is newer (a backup's step can come before Back up
 *  now's answer). A command's refusal rejects, for the caller to show. */
export function useCloud(): { status: CloudStatus | null; error: string | null; answer: (f: () => Promise<CloudStatus>) => Promise<CloudStatus> } {
  const [status, setStatus] = useState<CloudStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const events = useRef(0);
  const alive = useRef(true);
  useEffect(() => {
    alive.current = true;
    let live = true;
    let un: (() => void) | undefined;
    onCloudChanged((s) => {
      if (!live) return;
      events.current++;
      if (s) { setStatus(s); setError(null); }
    }).then((f) => (live ? (un = f) : f())).catch(() => {});
    const before = events.current;
    cloudStatus().then((s) => { if (live && s && events.current === before) { setStatus(s); setError(null); } })
      .catch((e) => { if (live) setError(String(e)); });
    return () => { live = false; alive.current = false; un?.(); };
  }, []);
  const answer = async (f: () => Promise<CloudStatus>) => {
    const before = events.current;
    const s = await f();
    if (alive.current && s && events.current === before) setStatus(s);
    return s;
  };
  return { status, error, answer };
}
