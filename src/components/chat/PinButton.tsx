// Pin to the top, or Unpin on a pinned chat (GA-257): beside each chat on the left, in a conversation's top bar and on the
// Archive's rows. It sits next to the chat's link, never inside it, so pressing it, by mouse or by key, doesn't open the chat.
import { Pin, PinOff } from "lucide-react";

export function PinButton({ id, pinned, onToggle }: { id: string; pinned: boolean; onToggle: () => void }) {
  const label = pinned ? "Unpin" : "Pin to the top";
  return (
    <button type="button" className={`btn ghost sm icon-only pin-btn${pinned ? " on" : ""}`} aria-label={label} title={label} data-pin={id}
      onClick={(e) => {
        e.preventDefault();
        e.stopPropagation();
        const box = e.currentTarget.closest<HTMLElement>("[data-pins]");
        const focused = document.activeElement === e.currentTarget;
        onToggle();
        // On the left the chat moves between Pinned and Recent: the focus goes with it, to its button there.
        if (focused && box) {
          requestAnimationFrame(() => {
            if (!box.contains(document.activeElement)) box.querySelector<HTMLButtonElement>(`.pin-btn[data-pin="${CSS.escape(id)}"]`)?.focus();
          });
        }
      }}>
      <Pin className="icon" /><PinOff className="icon" />
    </button>
  );
}
