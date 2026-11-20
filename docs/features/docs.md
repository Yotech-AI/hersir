# Docs

Each project can have a Docs page of its own, in the sidebar's Docs section below Agents: its Changelog, its Decisions and its Commits. Docs is off for every project until you turn it on.

## Turning it on

- **Project form → Docs → Show this project under Docs.** Off by default, also for every project from before this version.
- **On:** the project shows under Docs in the sidebar, with its colour dot, and its name opens its Docs page.
- **Off:** the project leaves the Docs section. Its Docs page says Docs is off, with Edit project to turn it on.

## The sidebar's Docs section

- It lists the projects with Docs on, and marks the one whose page is open.
- With none on, it says No project has Docs on, with a link to Projects.
- It folds with the caret at the end of its heading, like Agents and Memory, and stays folded after a restart.

## Changelog

The page opens on Changelog: the project's merged cards, newest first, by month, each with its day, its identifier (a link to the card) and its title.

- A card counts as merged once it is in a Deploy or Done column. Archived cards stay in the changelog.
- Its day is when it reached Done. A card in a Deploy column shows when it last changed.
- It keeps itself up to date: when an agent's card reaches main, it shows here, with nothing to write.

## Decisions

The shared notes in Memory about the project's decisions, newest first, each linking to the note in Memory.

- A note shows here when it is in `Decisions/`, or has `type: decision`, and its `project:` is the project's key or name, in any case.
- The Chief keeps them. Its answers to the agents' questions go in `Decisions/<project name>` ([Agents ask the Chief first](memory.md)), and its Memory block tells it that a decision about one project goes in `Decisions/` with the project's key as `project:`. The project's agents get those notes in their runs too.
- An agent's own notes and the Chief's own aren't shared, so they never show here.

## Commits

The newest 50 commits on main, newest first: each with its short id, its first line, who made it and when.

- Main is main as last fetched, as a card's changes measure from it: origin/main for a project linked to GitHub or Bitbucket, else the local main branch. Hersir doesn't fetch for it.
- A merge into main is one commit, with its pull request's changes.
- A commit opens to the files it changed against the commit before it (a first commit: all its files, as added), each in the same box as a card's Changes tab. A file's diff loads once its box comes near the screen.
- A project without a git repository says how to link one.

## Not to be confused with

A project's documents (project page → Docs, and the Doc page) are separate: notes and specs you and the Chief write. The Docs section is the project's developer docs.
