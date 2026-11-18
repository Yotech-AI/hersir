// Rename on a chat on the left (GA-309), the way a terminal's tab renames: the Rename button beside the pin button (it sits
// next to the chat's link, never inside it, so pressing it doesn't open the chat), and the input the title becomes. Enter or
// leaving the input keeps the name, Escape doesn't.
import { useRef, useState } from "react";
import { Pencil } from "lucide-react";
import { NAME_MAX } from "./useRenames";

export function RenameButton({ onRename }: { onRename: () => void }) {
  return (
    <button type="button" className="btn ghost sm icon-only rename-btn" aria-label="Rename" title="Rename"
      onClick={(e) => { e.preventDefault(); e.stopPropagation(); onRename(); }}>
      <Pencil className="icon" />
    </button>
  );
}

/** The chat's name, all selected. `onEnd`, once: the name to keep (Enter, or leaving the input), or null (Escape); `byKey`:
 *  Enter or Escape, so the focus can go back to the chat's link. */
export function ChatNameInput({ name, onEnd }: { name: string; onEnd: (name: string | null, byKey: boolean) => void }) {
  const [value, setValue] = useState(name);
  const done = useRef(false);
  const end = (keep: boolean, byKey: boolean) => {
    if (done.current) return;
    done.current = true;
    onEnd(keep ? value : null, byKey);
  };
  return (
    <input className="input name-in" value={value} maxLength={NAME_MAX} aria-label="Chat's name" autoFocus spellCheck={false}
      onFocus={(e) => e.currentTarget.select()} onChange={(e) => setValue(e.target.value)} onBlur={() => end(true, false)}
      onClick={(e) => e.stopPropagation()} onDoubleClick={(e) => e.stopPropagation()}
      onKeyDown={(e) => {
        e.stopPropagation();
        if (e.nativeEvent.isComposing) return;
        if (e.key === "Enter") { e.preventDefault(); end(true, true); }
        else if (e.key === "Escape") { e.preventDefault(); end(false, true); }
      }} />
  );
}
