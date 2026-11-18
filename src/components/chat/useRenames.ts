// Rename on a chat on the left (GA-309): the new name shows at once; the backend's notice ("chat_threads") then reloads
// Recent, Pinned, the top bar and the Archive, and a rename the backend refuses shows the old name again and says why.
import { useEffect, useState } from "react";
import { renameChat } from "../../api";
import type { ChatThread } from "../../types";

/** The longest name a chat takes (the backend's chat::NAME_MAX). */
export const NAME_MAX = 80;

/** A name as the backend saves it: on one line, runs of whitespace one space, trimmed. */
export const chatName = (text: string) => text.split(/\s+/).filter(Boolean).join(" ");

export type Renames = {
  /** The chat's name: the one just given, while the lists still have its old title. */
  title: (t: ChatThread) => string;
  /** Saves `name` as the chat's name. An empty name, only spaces or the same name: nothing is saved. */
  rename: (t: ChatThread, name: string) => void;
  /** Why the last rename failed; "" when nothing failed. */
  error: string;
};

/** How long a failed rename's message shows (as a failed pin's). */
const ERROR_MS = 5000;

/** Per chat: the name given, and the title it had then. */
type Given = Record<string, { from: string; to: string }>;

export function useRenames(): Renames {
  const [given, setGiven] = useState<Given>({});
  const [error, setError] = useState("");
  useEffect(() => { if (!error) return; const t = setTimeout(() => setError(""), ERROR_MS); return () => clearTimeout(t); }, [error]);
  // Once the lists have another title (the new one, after the reload), theirs counts.
  const title = (t: ChatThread) => { const g = given[t.id]; return g && g.from === t.title ? g.to : t.title; };
  const rename = (t: ChatThread, name: string) => {
    const to = chatName(name);
    if (!to || to === title(t)) return;
    setGiven((was) => ({ ...was, [t.id]: { from: t.title, to } }));
    renameChat(t.id, to).catch((e) => {
      setGiven((was) => {
        if (was[t.id]?.to !== to) return was;
        const rest = { ...was };
        delete rest[t.id];
        return rest;
      });
      setError(`Couldn't rename the chat: ${String(e).replace(/^Error:\s*/, "")}`);
    });
  };
  return { title, rename, error };
}
