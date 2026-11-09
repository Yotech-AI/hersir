# Settings

Where each setting lives, what a backup holds and how to restore one, how Hersir looks, what the screenshot settings do, and what happens when you close its window.

## The tabs

| Tab | What it holds |
|---|---|
| General | Quit, Updates, the Editor's folders, Data: where Hersir keeps your data, and the worktrees of finished cards; and Backups: a folder for copies of your data, how often, how many to keep, Back up now, the backups there and Restore |
| Appearance | The font, text sizes, theme, density and the mountains |
| Notifications | A switch for each kind of notification |
| Agents and runs | Coding CLIs; Runs: limits, runs at once, memory for agent runs, Pause all agents, memory and Ask the Team Lead first; and Screenshots: what Hersir keeps of agents' screenshots, and the previews' ports |
| MCP servers | The outside services agents can use, and the built-in browser |
| GitHub and Bitbucket | gh and how pushes go, Check connection, and your Bitbucket login |

Each feature's doc says which settings it uses: [Updates](install.md#updates), [Backups](#backups), [the Editor's folders](features/editor-and-terminal.md#open-a-folder), [Data](features/board.md#a-worktree-for-every-card), [Notifications](features/inbox.md#notifications), [Coding CLIs](features/cli.md), [Runs](features/board.md#when-agents-start) and [their limits](features/board.md#runs), [Screenshots](features/screenshots.md), [MCP servers](features/tools.md#mcp-servers) and [GitHub and Bitbucket](features/testing.md#review-on-github).

## Backups

Settings → General → Backups copies your data to a folder you choose: a USB disk, or a folder that Dropbox or Syncthing keeps. Inside it Hersir makes a folder named `Hersir backups` and backs up there, unless the folder you chose is such a folder already (it holds `gizai-backups.json`). Each change saves at once; a value that can't be saved, like Keep 0, shows why under its field and nothing is saved.

- **What a backup holds:** the database and the files Hersir stores (attachments and screenshots).
- **What it never holds:** keys, passwords and sign-ins, which stay in your system's keychain; nor worktrees, run and chat logs, or the source of an update.
- **What it never does:** change your data, unless you restore a backup ([Restore](#restore)), or remove another computer's backups or one made before a restore.
- **Not encrypted:** whoever can open the folder can read the data in it. A folder that Dropbox, Syncthing or another service keeps gets your data unencrypted, on every device it syncs to (and on Dropbox's servers). Choose such a folder only if you trust it with your data.
- **Several computers can share one folder.** Each adds and removes only its own backups, and the stored files are shared by all of them. The list shows another computer's backups with "from" and its name ("from another computer" when it has none).
- **The format**, file by file, is in [docs/backup-format.md](backup-format.md).

| Setting | Default | Range | What it does |
|---|---|---|---|
| Folder | not set: no backups | a full path to a folder Hersir can write in | Browse… picks it with your system's folder picker, Turn off clears it. A folder that can't be used now (a USB disk that isn't plugged in) shows why under it, and backups wait for it. |
| When | Daily | Daily, Weekly or Only when I ask | Daily and Weekly back up when this computer's newest backup in the folder is a day or a week old. Hersir looks 2 minutes after it starts, then every hour. |
| Also when I quit Hersir completely | off | on or off | Quit Hersir completely backs up first and waits for it, usually a few seconds and at most 5 minutes. Closing the window, logging out and shutting down don't back up. |
| Keep | 10 | 1 to 100 | How many of this computer's backups stay in the folder: after each backup, older ones are removed. One made before a restore is never removed, and neither are other computers' backups. |

Back up now makes one at once, in the background, and shows its step while it runs. It can't be pressed without a folder or while a backup runs. Above it the section shows the last backup with its size (its database file, compressed), when the next one is due, and a backup that failed, in red with its time and why. The list shows each backup in the folder, newest first: its time, why it was made (Daily, Weekly, By hand, On quit or Before a restore), its size (the database and its files), the computer it came from when it is another one, and Restore, or why one can't be restored.

### Restore

Each backup that can be restored has Restore, which can't be pressed while a backup or another restore runs. One that can't be restored, such as one a newer Hersir made or one with missing files, says why instead. Below the list, Restore from another folder… opens your system's folder picker and lists that folder's backups, other computers' too: a USB disk on a new computer, say. Choose the `Hersir backups` folder or the folder it is in.

1. **The question.** Restore says which backup it is (why it was made, its time and its computer) and where today's data goes: the backups folder set above, or else the folder you restore from. Cancel changes nothing.
2. **The steps.** Hersir checks the backup, saves today's data as a backup and copies what the restart needs, in the background, and shows each step here and in the sidebar. You can go on working. Cancel restore stops it. If something is wrong, such as a backup from a newer Hersir or a file that is missing or has changed, the reason shows and your data stays as it is.
3. **Ready.** "Restore ready" and the backup's time show here and above Company in the sidebar, with Restart now and Cancel restore. Until the restart today's data keeps working, but what you change is not kept. Restart now stops agents at work first, and asks once more when there are any. Cancel restore removes what was copied, and the next start is a normal one; the backup of today's data stays in its folder.
4. **After the restart.** Hersir shows your data as it was then, and a notice here and in the sidebar says how it went, until Dismiss. Agents are paused: resume them in Settings → Agents and runs. If the restore failed, the notice says why, and Hersir started with today's data.

A restore never touches keys and passwords: they stay as they are on this computer, in your system's keychain. This computer also keeps its own identity and its own settings, such as its programs, the Editor's folders, these backup settings and Runs at once. Everything else comes from the backup, also projects' linked folders and agents' folders: on a new computer, change those that are elsewhere there. [docs/backup-format.md](backup-format.md#restore) has it step by step.

## Appearance

- **Font:** one font for the whole app: Atkinson Hyperlegible, JetBrains Mono, Inter, Geist or Hack, all bundled.
- **Text size:** one for the chat, one for the interface, and one for tasks and docs. Reading text grows, headings half as much, and IDs and labels stay put; the chat column gets wider with it.
- **Theme and density**, also with the t and d keys.
- **Mountains:** whether Hersir's wireframe mountains show along the bottom of the window, behind text and controls. They are on by default.

Changes show at once and are kept on this computer.

## Screenshots

Agents take [screenshots of what they built](features/screenshots.md), and the card shows them on its Screenshots tab. Their settings live in four places. A run takes screenshots only when its agent's browser and Take screenshots of what it built are on, the card's Screenshots before Review is on (until you set it, its project's Screenshots), and the project has a Preview command.

Each value has a default and a range. A value outside its range is refused when you save, with why, like "Screenshots per run must be from 1 to 30."

### Settings → Agents and runs → Screenshots

What Hersir keeps of the screenshots, for every project, and the ports the projects' previews get.

| Setting | Default | Range | What it does |
|---|---|---|---|
| Screenshots per run | 12 | 1 to 30 | Hersir keeps a run's first ones by file name and says in the run what it left out. The agent is told the limit. |
| Largest screenshot (MB) | 10 | 1 to 50 | A bigger file is left out, and the run says so. |
| Keep per card (sets) | 3 | 1 to 20 | A card keeps its newest sets with pictures. Older sets go, and so do older runs that took none. |
| Delete a finished card's screenshots after (days) | 30 | 0 to 365 | Counted from Done or Cancelled. 0 keeps them. |
| Preview ports | 47100 to 47999 | from 1024 to 65535, the lower port first | A run's preview gets a free port from this range and the nine after it, so the range must hold ten ports for each run at once (Runs → Runs at once): 30 with the default 3. |

Old sets go when Hersir starts and once a day, not the moment a newer set arrives ([Old screenshots go](features/screenshots.md#old-screenshots-go)).

### The project's form → Preview

Each project's own (its page → Edit → Preview): whether its cards get screenshots, the preview that serves a card's branch, and the sizes and themes the agents take.

| Setting | Default | Range | What it does |
|---|---|---|---|
| Screenshots: Take screenshots of this project's screens | on | on or off | Off: agents take none on its cards, except on a card whose own switch is on. |
| Preview command | empty | a command | Serves the card's branch with demo data while an agent that takes screenshots works on it. Empty: no preview, so no screenshots. Hersir runs it with `bash -c` (Git Bash on Windows) in the card's worktree, on a free port in `HERSIR_PREVIEW_PORT` (and `PORT`), with `HERSIR_PREVIEW_DIR` for a throwaway database, never your `.env` one (the `GIZAI_PREVIEW_*` names are set too), and ends it with the run ([The preview](features/screenshots.md#the-preview)). |
| Start time (seconds) | 120 | 10 to 600 | The agent starts once the page answers, or after this long without it, and then takes none. |
| Themes: Both themes, dark and light | off | on or off | On: each changed screen in the first theme, and the card's main screen in the other one too. Off: the theme the page opens in. |
| Sizes | Desktop 1280 × 800 and Mobile 390 × 844 | 1 to 4 sizes, each 320 to 3840 wide and 320 to 2400 high | Each changed screen is taken at every size. A size's name has at most 40 characters. |
| Whole page: The whole length of each page | off | on or off | Off: one window's height at each size. |

For Hersir's own project, use Window 1440 × 900 and Small 900 × 600 (its window's default and smallest size), with both themes on.

### The agent's form → Tools → Browser

| Setting | Default | Range | What it does |
|---|---|---|---|
| Take screenshots of what it built | on for Frontend, Design and QA agents (also ones made before it existed), off for the rest | on or off, only with the browser on | At the end of a run that changed a screen, the agent saves screenshots of it for the card's Screenshots tab, at the project's sizes. With the browser off it is off and can't be switched. |

The browser works on Claude Code only, so Codex and Gemini agents take none. Once you have listed the browser's tools, its `take_screenshot` must be on too.

### The card's Properties

| Setting | Default | Range | What it does |
|---|---|---|---|
| Screenshots: Screenshots before Review | its project's Screenshots switch | on or off | Off: the card's agents take none. Until you set it, it follows the project's switch; once set, Follow `<project>` under it goes back to that. |

## The window and the tray

Hersir keeps running when you close its window (Super+W on Omarchy, or the X button): the agents, the board check and the chat go on.

- A tray icon (top right in Omarchy's Waybar, the menu bar on macOS) has Open Hersir and Quit Hersir completely.
- Starting Hersir again also brings the window back.
- Settings → Quit quits too, and says how many runs and chat answers it stops.
