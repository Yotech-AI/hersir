// Chips beside the chat (GA-297; design/islands-chat → What opens, and when → Chips): on the Chat page a click on a card
// chip opens the card in a Card tab of the chat's side island (or shows its tab), Ctrl+click (Cmd+click on macOS) opens its
// page, and its hover card says "Click to open beside the chat". The Chat page gives this context; chips anywhere else get
// null and open the page, as before. On its own, with nothing but React, so every chip doesn't load the chat's code.
import { createContext, useContext } from "react";

/** Opens the card (by its id, GA-259) beside the chat; null outside the Chat page. */
export const BesideChat = createContext<((identifier: string) => void) | null>(null);
export const useBesideChat = () => useContext(BesideChat);

/** The hover card's last line. */
export const CLICK_TO_OPEN = "Click to open";
export const CLICK_TO_OPEN_BESIDE = "Click to open beside the chat";
