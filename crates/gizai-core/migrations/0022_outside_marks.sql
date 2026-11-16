-- The approve step after a tool from outside Hersir (2026-11-09, GA-261). A chat that used one (an MCP server of the
-- Chief's own, the web, the browser) stays marked: its session holds what that tool returned, so from then on what the
-- Chief does there waits for your Approve, also in its later messages, in a new session with a hand-over and after a
-- restart. A new chat starts unmarked.
ALTER TABLE chat_threads ADD COLUMN outside_tool TEXT;  -- the first tool from outside Hersir it used, like mcp__otus__search; NULL: none
-- A chat that used one before this upgrade is marked too, with its first tool call that wasn't Read, Glob, Grep or one of
-- Hersir's own (mcp__gizai__<tool>, as chat::is_outside_tool tells them).
UPDATE chat_threads SET outside_tool = (
    SELECT m.tool_name FROM chat_messages m
     WHERE m.thread_id = chat_threads.id AND m.role = 'tool' AND m.tool_name IS NOT NULL AND m.tool_name NOT IN ('Read', 'Glob', 'Grep')
       AND NOT (m.tool_name GLOB 'mcp__gizai__?*' AND substr(m.tool_name, 13, 1) <> '_' AND instr(substr(m.tool_name, 13), '__') = 0)
     ORDER BY m.rowid LIMIT 1);
-- Which hold a card is on, when that matters. waiting_for_ok: GA-101's hold (needs_decision, "Waiting for your OK: …"),
-- which the Chief's change after such a tool put on the card. Only you take that one off, on the card or by approving the
-- Chief's call that waited for you; the board, the Inbox and the card show it as any needs_decision hold. It comes off
-- with the hold, and stays when a run that was under way puts its own hold on the card.
ALTER TABLE tasks ADD COLUMN hold_kind TEXT;  -- waiting_for_ok, or NULL: any other hold, or none
UPDATE tasks SET hold_kind = 'waiting_for_ok'
 WHERE hold = 'needs_decision' AND hold_reason LIKE 'Waiting for your OK: the % put this card here after using %';
