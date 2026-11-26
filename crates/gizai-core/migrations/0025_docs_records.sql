-- Docs (2026-11-21): what a project's Docs page keeps besides the project's own files.
-- * projects.docs_off: the sections a person switched off in the project form (reference, guides, api, changelog,
--   decisions, mcp), comma-separated. Empty for every project, so every section shows as before.
-- * docs_changes: a card's line in the project's changelog, from the agent that built it: its kind (Added, Changed,
--   Deprecated, Removed, Fixed or Security) and one plain line. It shows once the card is merged.
-- * docs_decisions: the decision records the Chief keeps for a project, numbered per project like ADRs.
-- * docs_baselines: each project's declarations as they were at its last commit, for "Changed in the docs".
ALTER TABLE projects ADD COLUMN docs_off TEXT NOT NULL DEFAULT '';
CREATE TABLE docs_changes (
    task_id TEXT PRIMARY KEY REFERENCES tasks(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    text TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE TABLE docs_decisions (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    number INTEGER NOT NULL,
    title TEXT NOT NULL,
    status TEXT NOT NULL,
    context TEXT NOT NULL DEFAULT '',
    decision TEXT NOT NULL,
    consequences TEXT NOT NULL DEFAULT '',
    alternatives TEXT NOT NULL DEFAULT '',
    related_files TEXT NOT NULL DEFAULT '[]',
    task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
    supersedes INTEGER,
    decided_by TEXT NOT NULL DEFAULT '',
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE (project_id, number)
);
CREATE TABLE docs_baselines (
    project_id TEXT PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
    commit_id TEXT NOT NULL,
    taken_at INTEGER NOT NULL,
    entries TEXT NOT NULL
);
