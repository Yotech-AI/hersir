// The Terminal kind of the Chat's side island (GA-278, GA-296; design/islands-chat → Terminal): its tab is the Editor
// panel's (TermTab, with its close question CloseAsk), and under the island's header the shown terminal's folder in mono,
// then the terminal, in the place { kind: "chat" }. The island itself, its strip and its keys are ChatSide.tsx's.
import { useEffect, useState } from "react";
import { editorFolders } from "../../api";
import type { EditorFolder, TerminalSession } from "../../types";
import { manyNote, useTerminals, type TerminalPlace } from "../../lib/terminals";
import { openChatTerminal } from "../../lib/chatTerminal";
import { onWindowFocus } from "../../lib/windowFocus";
import { FolderPicker } from "../editor/FolderPicker";
import { TerminalView } from "../terminal/TerminalView";

export const CHAT_PLACE: TerminalPlace = { kind: "chat" };

/** Under the header: the terminal's folder (its whole path the tooltip), then the terminal, with the focus when it was just
 *  opened, picked or shown. */
export function ChatTerminalBody({ s, focusNext }: { s: TerminalSession; focusNext: boolean }) {
  return (
    <>
      <div className="chat-term-where" title={s.folderPath}><bdi>{s.folderPath}</bdi></div>
      <TerminalView key={s.id} id={s.id} place={CHAT_PLACE} autoFocus={focusNext} />
    </>
  );
}

/** Why a terminal can't start in the folder now (an agent works on the card's worktree; the backend refuses it too), or
 *  null; asked again when the window gets the focus, as the Terminal page asks. */
export function useStartOff(folder: string | null): string | null {
  const [found, setFound] = useState<EditorFolder>();
  useEffect(() => {
    if (!folder) return;
    let alive = true;
    const load = () => { editorFolders().then((fs) => { if (alive) setFound(fs.find((f) => f.id === folder)); }).catch(() => {}); };
    load();
    const stop = onWindowFocus(load);
    return () => { alive = false; stop(); };
  }, [folder]);
  return found && found.id === folder && found.readOnly ? `${found.readOnlyWhy ?? "An agent is working on it"}: a terminal there waits until its run ends.` : null;
}

/** The folder picker titled "Open terminal in" (New terminal with no terminal in the chat yet, and New terminal in…), with
 *  the note from 20 open terminals; a pick starts a terminal there in the chat's side island, and one that can't start says
 *  why under the folder's row, with the picker still open. */
export function OpenTerminalPicker({ at, you, onClose }: { at: { x: number; y: number }; you: string; onClose: () => void }) {
  const open = useTerminals().length;
  return (
    <FolderPicker at={at} title="Open terminal in" you={you} note={manyNote(open)} onClose={onClose}
      onPick={(f: EditorFolder) => openChatTerminal(f.id, false)} />
  );
}
