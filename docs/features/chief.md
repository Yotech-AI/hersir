# The Chief

The Chief is the agent you chat with: it looks after your board for you, and answers what it can of the other agents' questions before they reach you.

## What it can do

The Chief manages clients, projects, cards and agents with Hersir's own tools. In a chat it can:

- make and change clients, projects, cards and docs, and comment on and move cards;
- split bigger work into cards that build on each other, so they start in order by themselves ([below](#cards-that-build-on-others));
- start and continue agents' runs, and add or change agents and columns;
- attach a file you gave it to a card, project or client;
- read the files on a client, project or card, in a copy of each in its own folder: text, PDFs and images, not Word or Excel files, and not a file over 100 MB;
- read and keep notes in [memory](memory.md);
- update a project's linked folder, after your yes;
- merge a pull request that QA passed, in a project where you allow it ([The Chief may merge](testing.md#the-chief-may-merge)).

Its chat runs on Claude Code ([Bring your own CLI](cli.md)).

On a new install, the first start ends in a welcome chat: the Chief welcomes you, says in a few lines what it and the team can do, and asks whether you'd like help setting up agents, clients, projects, tasks or anything else ([First start](../install.md#first-start)).

## The chat

- **Runs on**, under the text box, moves one chat to another Claude Code account (Settings → Coding CLIs), for example when one is near its weekly limit. The new account gets the conversation handed over, and the chat notes the switch. An answer that hits a usage limit offers Answer on another account.
- **A queue:** messages you send while the Chief answers wait in a queue and go together when the answer is done. After a stopped or failed answer they wait for Send now.
- **Recent and Archive:** Recent shows the 30 chats with the newest activity. Archive, under the list, shows every chat, newest first, and searches their titles and the messages you and the Chief wrote. No chat is hidden or deleted: a new message in an old chat moves it back to the top of Recent.
- **Pinned:** Pin to the top keeps a chat under Pinned, above Recent, until you unpin it, also when it is older than the 30 in Recent: for a chat you want to come back to. The pin button is beside each chat on the left (when you point at it or tab to it), in a chat's top bar and on each row of Archive; on a pinned chat it says Unpin. The chat pinned last is at the top. A pinned chat isn't listed again under Recent; Unpin puts it back there when it is one of the 30 newest, else it is in Archive. Pins are kept with your data, so they stay after a restart and are in backups.
- **A long chat** opens and scrolls like a short one: only the messages in sight, and a few around them, are on the page. So the window's Find (Ctrl+F) only finds words in those; to find a message further up or down, search in Archive.
- **Files:** + next to the text box adds files to a message (Add files, or drop them on the Chat page) or links an item. A message's files are kept with it; the Chief reads a copy of each in its own folder and can attach one to a task, project or client.

## Link anything with @

@ in the chat and in every Markdown text box links a task, project, client, agent, person or doc.

- Only @ lists the kinds, `@task.` (and the others) one kind, and typing searches by ID, key or name.
- `@task.` shows the tasks under a heading per column, in the board's order.
- The link shows as a chip with the item's name, which opens the item (Ctrl+click while you edit).

## Approve with one click

Once a chat used a tool from outside Hersir (an MCP server, the web, the browser), what that tool returned stays in the chat's session and could steer the Chief. So the chat keeps a mark: for the rest of the chat, also in later messages, in a new session and after a restart of Hersir, what the Chief does there waits for your Approve. The chat says so once, when it is marked: "This chat used `<tool>`, a tool from outside Hersir: from now on, what the Chief does here waits for your Approve.", with the tool in words, like search on the otus server or fetching a web page. A new chat starts unmarked.

The Chief's calls that wait start or continue runs, change agents or columns, attach a file, update a linked folder, save to memory, merge a pull request, comment on a card, change a card's description, acceptance criteria, assignee or the cards it builds on (in a column that isn't Auto; in an Auto column the card waits on hold instead, [below](#cards-that-wait-for-your-ok)), or change a project's repository, folder, main branch or goal. A change to a project's name, colour, status, client or Test runs at once applies at once. Each call that waits shows under the answer that tried it:

- with a line about what it does and a preview: a note's new text and what changes, the card and agent, the settings that change, the file and where it goes, the comment's text, and each field of a card or project that changes, old and new;
- with **Approve** and **Discard**, and **Approve all** when there are several.

Approve runs exactly that call once, with every other rule in force; it isn't kept again, and a card it changes doesn't go on hold. Discard runs nothing. The chat notes what happened and the Chief hears it in its next answer. Approving doesn't start a new answer: once nothing waits and the Chief isn't answering, a last note says its answer is done, so send a message to go on. Until you decide, the chat waits for you at the top of [the Inbox](inbox.md), also after a restart.

Tools that only read never wait, and a chat that never used a tool from outside Hersir works as before.

### Cards that wait for your OK

In a marked chat, a card the Chief puts in an Auto column (its agents pick it up by themselves), or whose description, acceptance criteria, assignee or cards it builds on it changes there, is saved on hold: "Waiting for your OK: the Chief put this card here after using `<tool>`, a tool from outside Hersir. Check it, then take it off hold." No agent starts it.

Only you take that hold off: **Clear hold** on the card (or Continue, or a move by hand into To do or In progress), or **Approve** on a call of the Chief's that waited for you and takes it off, like a Continue (its preview says so). The Chief can't, from a chat or a board check: taking the hold off (`update_task` with `clear_hold`), putting another hold in its place, Continue and Run on that card are refused with "Only the user takes this hold off, on the card: it waits for their OK after a tool from outside Hersir." A board check leaves such a card alone. Other holds come off as before, from a chat and from a board check.

The rules behind it: [docs/agent-tools.md](../agent-tools.md#safety-rules).

## Cards that build on others

The Chief splits bigger work into cards that build on each other: a backend before its screens, a base before its follow-ups. A card waits until the cards it builds on are merged, then starts by itself ([Cards that build on others](board.md#cards-that-build-on-others)). Its starting instructions say: "When a card needs another card's code, set builds_on: it then starts by itself once that card is merged."

- `create_task` and `update_task` take `builds_on`: the identifiers of the cards it builds on, like `["GA-226"]`. On `update_task` it is the full new list, and an empty list clears it. A card building on itself, more than 20 cards and a loop are refused with why, and nothing changes.
- `get_task` shows `builds_on` (each card's identifier, title, column, and `merged` true or false) and `needed_by` (the cards that build on it).
- `list_tasks` shows `waits_for` on a card that waits: the cards it builds on that aren't merged yet.
- `start_agent_run` and `continue_agent_run` on a card that waits don't start it, and say which cards it waits for.
- `check_board` reports a card that waits for a card on hold or cancelled: kind `waiting`, code `builds_on_blocked`, like "GA-227 waits for GA-226, which is on hold: needs_decision". A card that waits for one still in work is no finding.

## Teams

With more than one team, each with its own board ([Several teams](board.md#several-teams)), the Chief's tools name the team, by its name or id. With one team, `team` may be left out, as before.

- `add_column`, `set_column` and `create_agent` take `team`. With more than one team, a call without it is refused with the teams' names: "Name the team: My team or Research team."
- `get_workflow` takes `team` for one team's columns; without it, each team's columns come under its name. `list_agents` takes `team` for one team's agents, and with more than one team says each agent's team.
- `create_project` takes `team` (the first team when left out). `update_project` with `team` moves the project and all its cards, as the project form does, with the same refusal while one of its cards has a run live or starting. A board check never moves a project, and neither does a chat that used a tool from outside Hersir.
- `get_project` and `list_projects` show each project's team.

## The board check

Agent settings → Chat → Check the board every 15 min, off until you turn it on; the minutes go from 5 to 1440. It looks for answered questions, held and stuck cards, and cards no agent will start.

Only something new starts it. It gets agents going again (Continue or Run), and asks you what it can't decide in a chat of its own, labelled Question or Approval, at the top of your Inbox.

## Agents ask the Chief first

An agent that ends its run asking for a decision (`needs_decision`) asks the Chief before you. Settings → Runs → Ask the Chief first switches this; it is on by default.

- The card stays on hold but out of your Inbox ("With the Chief").
- A short run of the Chief reads the question, and looks in memory, the card, the docs and its copies of the code.
- It answers with a comment on the card. The agent's session continues with that answer as its Continue note, and the answer is kept in memory, linked to the card.
- Or it asks you: a comment "Needs `<you>`: why, the options, my advice", and the card lands in the Inbox with "Chief escalated to you: why".

Money, scope, deadlines, client messages, security, deleting and anything it can't find always come to you. So does any error, timeout or run without an answer. At most one try per question and two per card, and the question right after the Chief's answer comes to you.

Run this for me requests and failed pushes skip the Chief. When you answer a question it escalated, the Chief keeps your answer in memory as the agent starts again. The Runs tab shows who answered and what the Chief's look cost (on its budget).

More: [docs/memory.md](../memory.md#agents-ask-the-chief-first-ga-70).

## It reads your code

The Chief reads its own read-only copy of each active project's code: `<data dir>/code/<KEY>`, a worktree without a branch. The copy is kept at the commit a new card starts from and refreshed before each answer. Your own checkouts are never passed to it.

## It updates your linked folders, after your yes

When a project's linked folder has dependencies behind main (`vendor/` or `node_modules/` missing, or a lock file that differs from main's), the Chief tells you what an update would do. After your yes it updates the folder:

- with uncommitted changes in tracked files, only if you agreed: put them in git's stash first (git stash pop brings them back);
- a fast-forward to main, switching branch only if you agreed;
- then `composer install` or `npm ci`;
- never a merge, a reset or the setup command.

## Limits per answer

Each of the Chief's answers has a time and a tool-call limit, and so do its board check and its look at an agent's question. Set them in Settings → Runs: Minutes and Tool calls per chat answer, 15 and 60 unless you change them (5 to 240 minutes, 20 to 500 tool calls).

A change counts from the next answer, and an answer that stops at a limit says which one.

## Read more

- [The Inbox](inbox.md): the Chief's questions and the calls that wait for your OK.
- [Memory](memory.md): the notes the Chief keeps.
- [docs/agent-tools.md](../agent-tools.md): the Chief's safety rules and how it merges pull requests.
- [docs/memory.md](../memory.md): the Chief's memory tools.
