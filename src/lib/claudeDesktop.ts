// Settings → MCP servers → Claude Desktop (GA-340, on GA-338's contract): the section's words, and which way Connect asks.
// GA-341: Claude gets the Chief's tools and rules, so the section, the key's warning and Disconnect's question say so.
import type { ClaudeDesktopStatus } from "../types";

export const CD_TITLE = "Claude Desktop";
export const CD_TEXT = "Use your Hersir board from the Claude Desktop app on this computer: Claude gets the Chief's tools and works by its rules. Hersir must be open.";
export const CD_ADD = "Add it to Claude Desktop's settings";
export const CD_NO_CONFIG = "Claude Desktop's settings weren't found on this computer: you'll paste the entry yourself.";
export const CD_NO_SHIM = "Hersir can't find its hersir-mcp program next to Hersir: reinstall Hersir.";
export const CD_REOPEN = "Quit and reopen Claude Desktop, then ask Claude: \"What's on my Hersir board?\"";
export const CD_PASTE = "In Claude Desktop: Settings → Developer → Edit Config. Paste this into the file (add the \"hersir\" server next to any servers already there), save it, then quit and reopen Claude Desktop.";
export const CD_KEY = "This entry holds a key with the Chief's powers over your board: any program running as you could use it. Hersir shows it only now; Connect again makes a new one and ends this one.";
export const CD_NO_ENTRY = "Claude Desktop's settings don't have the Hersir entry.";
export const CD_ASK_TITLE = "Disconnect Claude Desktop?";
export const CD_ASK_TEXT = "Claude Desktop can no longer use your board until you connect it again. Its settings keep the entry; Claude will say that Hersir disconnected it.";

/** "Added to Claude Desktop's settings (/home/you/.config/Claude/claude_desktop_config.json)." */
export const addedText = (configPath: string | null) => `Added to Claude Desktop's settings${configPath ? ` (${configPath})` : ""}.`;

/** "The old file is saved as …", or null without a backup. */
export const backupText = (backupPath: string | null) => (backupPath ? `The old file is saved as ${backupPath}.` : null);

/** A moment in the app's usual date and time: "23 Nov 2026, 14:02". */
export const dateTime = (ms: number) => new Date(ms).toLocaleString("en-GB", { dateStyle: "medium", timeStyle: "short" });

/** "Connected since 23 Nov 2026, 14:02" ("Connected" when the status has no time). */
export const connectedText = (s: Pick<ClaudeDesktopStatus, "connectedAt">) => (s.connectedAt != null ? `Connected since ${dateTime(s.connectedAt)}` : "Connected");

/** Whether the settings file is there but has no Hersir entry: only then does the section say so. */
export const missingEntry = (s: Pick<ClaudeDesktopStatus, "configPath" | "configHasHersir">) => s.configPath != null && !s.configHasHersir;

/** What Connect sends as addToConfig: the checkbox (`ticked`; null = untouched, so on) when the settings file was found, else false. */
export const addToConfig = (s: Pick<ClaudeDesktopStatus, "configPath">, ticked: boolean | null) => s.configPath != null && (ticked ?? true);
