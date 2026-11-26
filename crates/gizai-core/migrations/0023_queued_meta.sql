-- A message from Claude Desktop (2026-11-24, GA-351). Claude Desktop's ask_chief hands a message to the Chief as yours, marked
-- {"from": "claude-desktop"} in its meta_json, and the Chief's answer to it has Claude Desktop's limits. A message that waits in
-- the chat's queue keeps that mark until it goes, also after a restart, so a queued message gets the details a sent one has.
ALTER TABLE chat_queue ADD COLUMN meta_json TEXT;  -- {"from": "claude-desktop"} for a message from Claude Desktop; NULL: one you typed
