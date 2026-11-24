# The Editor and Terminal, built in

Open a project or a card's worktree inside Hersir, see its git status and edit it, with your own shell in a panel under the code or on its own page.

## Open a folder

The sidebar has an Editor section above Agents, and a Terminal section above Editor. Both open the same folders:

- a project's linked folder;
- a folder in Settings → General → Editor folders (`~/Herd` unless you change it);
- a card's worktree.

\+ on either section opens the folder picker.

The Editor section lists the folders you opened, at most 8. Each stays where it is when you open it again, and a new one joins at the bottom, like a new terminal. With a ninth, the folder you opened longest ago leaves the list.

## The Editor

A small code editor with syntax colours. It has:

- a file tree with git's status, tabs, and Ctrl+S to save;
- Go to file (Ctrl+P) and search in the folder (Ctrl+Shift+F);
- changes made outside Hersir, which show within a few seconds;
- **Open in**, which opens the folder in VS Code, Zed or PhpStorm when they're installed;
- a terminal panel under the code ([In the Editor](#in-the-editor)).

## Behind main

The top bar says how the folder stands against main on its remote, GitHub or another. It is fetched when the folder opens, every 10 minutes and on Fetch.

When the folder is behind, **Update** brings main up to date:

- with a fast-forward only, never over uncommitted changes and never switching branch;
- running composer install or npm ci only when a lock file changed.

The sidebar says "3 behind" on the folder's row.

## Terminals

Your own shell in any of those folders, as many as you like. Each terminal runs until you close it or quit Hersir, and closing the window keeps them running.

### In the Editor

The **Terminal** button in the Editor's top bar, between Fetch and Open in, shows a terminal panel under the code. When the folder has no terminal yet, it starts one in the folder's root.

- The panel's tabs are the folder's terminals, with **New terminal** (+), **Own page** and **Hide** on the right.
- Drag the gap between the code and the panel to change its height, which is the same in every folder.
- Ctrl+\` shows or hides the panel, and Ctrl+Shift+\` starts a terminal in it.

### In the Chat

**Open** in the Chat page's top bar, right before New chat, has **New terminal**: it starts a terminal beside the chat, in the side panel on the right of the conversation, so you can run what the Chief asks for without leaving the chat. With no terminal in the chat yet it asks for the folder: a project's folder, a folder in ~/Herd or a card's worktree. With one, it starts in the shown terminal's folder, and **New terminal in…** asks for another folder.

- The side panel's tabs are what you opened in this chat, in the order you opened them, each with its kind's icon: terminals, named after their folder, and cards, named by their id ([Cards beside the chat](chief.md#cards-beside-the-chat)). **+** has the same menu as Open, **Own page** moves the shown terminal to its own page (its tab closes here) and **Hide** hides the side panel. Under the tabs is the folder the shown terminal runs in.
- Each chat has its own tabs. Another chat shows its own, and a chat's terminals keep running while you are in another one. New chat has its own too, and the chat its first message makes keeps them.
- While the side panel shows, the conversation stays centred, with the text box and the Archive, as it is without the side panel. Drag the gap left of the side panel to change its width, which is the same in every chat and kept on this computer; a double-click gives the first width back. In a narrow window the chat list hides first, so the conversation keeps at least 400 px.
- **The tool strip** along the window's right edge shows while the chat has anything open, whether the side panel shows or not: an icon for each kind open, with how many. The kind on show has its icon selected. A click on an icon shows that kind's last tab, and a click on the selected one hides the side panel; the shells keep running. Its tooltip says the kind, the count and the key (**Terminal, 2 open**, Ctrl+`). A right click (or the context menu key) has **Show** or **Hide**, and **Close all**, which asks first while a program runs in one of them.
- Keys: Ctrl+` and Alt+1 show or hide the terminals, and Ctrl+Shift+` starts a terminal in the shown terminal's folder. In the side panel, Ctrl+PgDn and Ctrl+PgUp switch tabs. Alt+4 (Option+4 on macOS) shows or hides the cards; Alt+2 and Alt+3 are kept for the Editor and the board beside the chat, which come later. In a terminal, Alt with a number goes to the shell.
- The chat's terminals are in the sidebar too, and run until you close them or quit Hersir.
- **Open in terminal** under a Chief answer that asks you to run commands (in code after Run, or in a sh code block) opens a terminal in the folder the answer names, in the chat's side panel: a project's folder or a folder in your Editor folders, also when the answer names a folder or a file inside it, so `~/Herd/shop/resources` opens one in `~/Herd/shop`. A card's worktree doesn't count. With more folders named, each has a button, at most 3. A second click shows the same terminal while it waits for a command (one of this chat's); while a program runs in it, or after it ended, it starts another one there. When the answer names no such folder, **Open in terminal…** asks for the folder, as New terminal does. It never types or runs anything for you: the terminal starts in the folder and waits for you.

### In the sidebar and on its own page

Each terminal has a row in the sidebar, with what runs in it on the right, or "ended". It also has its own page:

- its folder's terminals as tabs, with + for another one there, and Ctrl+PgDn and Ctrl+PgUp to switch;
- find (Ctrl+Shift+F);
- Rename (F2 or a double-click);
- Show in Editor.
- **Layout** (right of +): the folder's terminals side by side, 2, 3 or 4 in a row, or 2 × 2. It starts the terminals it needs in the folder; a click in a terminal, or on its tab, picks the one you type in. Fewer panes close nothing: the others stay open as tabs.

### Closing a terminal

- Closing a terminal that a program runs in asks first.
- `exit` closes it.
- A shell that ends with another code says so, with **Start again**.

## A card's worktree

- While an agent works in a card's worktree, the Editor shows it read-only, and a new terminal there waits until the run ends.
- A card that isn't pinned to you stays read-only in the Editor until **Edit anyway**, which puts your changes in its worktree, uncommitted, for the agent's next run. A shell can change files too: the folder picker asks for Edit anyway before it starts a terminal there, and the Editor's terminal panel stays off until you press it.

### Work on a card yourself

A card assigned to you has **Open in editor** in its top bar. Hersir makes the card's worktree, prepared as for an agent, and pins the card to you, so agents leave it alone; from Backlog or To do it moves to In progress.

In the Editor, a bar above the code shows the card, its branch and how far it is, with three buttons:

- **Commit…**: a small box with the message and the files it commits. Like all of Hersir's git, it runs none of the repository's hooks; a `git commit` in the terminal does;
- **Push**;
- **Hand over**: to Testing, or to Review when the card's Testing is off.

When QA sends the card back, the bar says so until you open the card.

## A card's changes

Every card has three tabs at the top: Card, Changes and Screenshots ([Screenshots](screenshots.md) has the third).

- **Changes** lists the files the card changed under their folders on the left, and shows every file's diff on the right, one below the other.
- The changes come from the card's worktree, uncommitted files included, since its branch left main.
- A submodule is left out, with the changes inside it and a new commit of it: Hersir's git never looks inside one, whose own settings could start programs ([Agent tools](../agent-tools.md#a-nested-repository-in-a-worktree-ga-350)).
- In the Runs tab, a run that committed has **Show changes**: only what that run committed.
- Keys: n and p for the next and previous file, Alt+↓ and Alt+↑ for the next and previous change, [ folds the list, and Esc goes back to Card.

After a merge, the changes stay readable. When a card's pull request is merged, Hersir first fetches main and keeps the card's range in two refs of your repository, `refs/gizai/merged/<ID>/base` and `/head`. So what the card changed can still be read after a merge commit, a squash or a rebase, offline, once its branch is gone; Hersir never pushes these refs.

## Free

The Editor, the terminals and a card's changes are free, like the rest of the app. [docs/PAID-FEATURES.md](../PAID-FEATURES.md) lists what stays free.

## Read more

- [Tested before you see it](testing.md): what happens after Hand over.
- [The board decides who works](board.md#a-worktree-for-every-card): how a card's worktree is made.
- [docs/PLATFORMS.md](../PLATFORMS.md): how the Editor reads files and finds VS Code, Zed and PhpStorm, and which shell a terminal starts and how it ends, on Linux, macOS and Windows.
