-- Docs (2026-11-20): a project can be listed in the sidebar's Docs section, with its own Docs page there, switched on in the
-- project form. Off for every project (the default), so every project works as before until a person turns it on.
ALTER TABLE projects ADD COLUMN dev_docs INTEGER NOT NULL DEFAULT 0;
