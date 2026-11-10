// Chat → Pinned (GA-257): the pinned chats, the most recently pinned first, and Pin to the top / Unpin. A pin or an unpin
// shows at once; the backend's notice ("chat_pins") then reloads the list, and one that fails is undone and says why.
import { useEffect, useMemo, useState } from "react";
import { pinChat, pinnedChats, unpinChat } from "../../api";
import { useData } from "../../lib/useData";
import type { ChatThread } from "../../types";

export type Pins = {
  /** The pinned chats, the most recently pinned first. */
  list: ChatThread[];
  /** Whether the chat is pinned. */
  has: (id: string) => boolean;
  /** Pins the chat at the top of Pinned, or unpins it when it is pinned. */
  toggle: (t: ChatThread) => void;
  /** Why the last pin or unpin failed; "" when nothing failed. */
  error: string;
};

/** How long a failed pin's message shows. */
const ERROR_MS = 5000;

export function usePins(): Pins {
  const { data, setData, reload } = useData(() => pinnedChats());
  const [error, setError] = useState("");
  useEffect(() => { if (!error) return; const t = setTimeout(() => setError(""), ERROR_MS); return () => clearTimeout(t); }, [error]);
  const list = useMemo(() => data ?? [], [data]);
  const ids = useMemo(() => new Set(list.map((t) => t.id)), [list]);
  const toggle = (t: ChatThread) => {
    const pin = !ids.has(t.id);
    setData((was) => [...(pin ? [t] : []), ...(was ?? []).filter((x) => x.id !== t.id)]);
    (pin ? pinChat(t.id) : unpinChat(t.id)).catch((e) => { reload(); setError(`Couldn't ${pin ? "pin" : "unpin"} the chat: ${e}`); });
  };
  return { list, has: (id) => ids.has(id), toggle, error };
}
