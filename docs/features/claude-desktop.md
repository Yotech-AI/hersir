# Claude Desktop

Use your Hersir board from the Claude Desktop app on the same computer. Claude gets the Chief's tools and works by its rules: in its own chat it reads and changes your cards, clients and projects as the Chief does in Hersir's chat, can ask Hersir's Chief, and can show the board, a card or the Chief's thread right in the chat. Hersir must be open.

Claude Desktop runs on macOS and Windows; its Linux build is a beta.

## Connect

1. Open Claude Desktop once, so its settings folder is there. Hersir looks for its settings file at `~/Library/Application Support/Claude/claude_desktop_config.json` on macOS, `%APPDATA%\Claude\claude_desktop_config.json` on Windows and `~/.config/Claude/claude_desktop_config.json` on Linux (`$XDG_CONFIG_HOME/Claude/…` when that is set).
2. In Hersir, go to Settings → MCP servers → **Claude Desktop**. Leave **Add it to Claude Desktop's settings** on and press **Connect**.
3. Hersir says "Added to Claude Desktop's settings (`<the file>`)." When the file was there already, it adds "The old file is saved as `<the copy>`." Then: "Quit and reopen Claude Desktop, then ask Claude: "What's on my Hersir board?""
4. Quit Claude Desktop and open it again: it reads its settings only when it starts.
5. Ask Claude "What's on my Hersir board?". Claude Desktop asks whether Claude may use each Hersir tool the first time, with its own Allow prompt.

The section then says **Connected since** and the date and time, like "Connected since 23 Nov 2026, 14:02". It saves at once: Save settings has nothing to do with it.

What Connect does: it makes a key for Claude Desktop, as you, and puts a server named `hersir` in the file's `mcpServers`. That server is Hersir's `hersir-mcp` program (next to Hersir itself) with `--desktop`, Hersir's socket and the key. Hersir copies the file first (`claude_desktop_config.json.hersir-backup-<date>-<time>`, in the same folder), keeps every other server and setting as it was, and writes the file whole, readable only by you on macOS and Linux.

### Paste the entry yourself

When Hersir doesn't find Claude Desktop's settings, the box is off and greyed, with "Claude Desktop's settings weren't found on this computer: you'll paste the entry yourself." You can also switch the box off yourself. Then Connect shows the entry, with **Copy**, and:

> In Claude Desktop: Settings → Developer → Edit Config. Paste this into the file (add the "hersir" server next to any servers already there), save it, then quit and reopen Claude Desktop.

Hersir shows the entry only then. When Hersir was to add it but couldn't, it says why above the entry, like "Claude Desktop's settings file can't be read as JSON with an mcpServers list, so Hersir left it as it is. Paste the entry yourself: Claude Desktop → Settings → Developer → Edit Config."

While connected, a settings file without the entry shows "Claude Desktop's settings don't have the Hersir entry." Hersir checks again when you come back to its window, so it goes once you pasted the entry and saved the file.

When Hersir can't find its `hersir-mcp` program, Connect is greyed, with "Hersir can't find its hersir-mcp program next to Hersir: reinstall Hersir."

### Connect again and Disconnect

- **Connect again** makes a new key, and the old one stops working. With the box on, it puts the new entry in Claude Desktop's settings in place of the old one. Quit and reopen Claude Desktop, so it reads the new key.
- **Disconnect** asks first: "Disconnect Claude Desktop?", with "Claude Desktop can no longer use your board until you connect it again. Its settings keep the entry; Claude will say that Hersir disconnected it." Press **Disconnect** (or **Cancel**). The key stops working at once, also for a chat that is open. From then on Claude gets this answer for every Hersir tool: "Hersir disconnected Claude Desktop. To connect it again: in Hersir, Settings → MCP servers → Claude Desktop → Connect, then quit and reopen Claude Desktop."

Disconnect leaves Claude Desktop's settings as they are. To take Hersir out of Claude Desktop too, remove the `hersir` server in Claude Desktop → Settings → Developer → Edit Config.

## Open Hersir first

Claude Desktop reaches your board through the Hersir app on this computer. Claude Desktop starts `hersir-mcp` when it starts, and keeps it running; Hersir itself must be open.

While Hersir is closed, Claude has one Hersir tool, `hersir_status`, and every Hersir call answers: "Open Hersir first: Claude Desktop reaches your board through the Hersir app on this computer, and it isn't running." A view in the chat says the same, and keeps trying.

Open Hersir, and within a few seconds Claude Desktop gets Hersir's tools, without a restart. If Claude still has only `hersir_status`, that answers "Hersir is open and connected. If its tools aren't listed, quit and reopen Claude Desktop.": do that.

## Claude works like the Chief

- **The Chief's tools.** Claude gets every tool of the Chief's chat, so it can do what the [Chief](chief.md#what-it-can-do) does: make and change clients, projects and cards, comment on and move cards, start and stop agents' runs, read and keep notes in [memory](memory.md), and more. Two are left out, and answer why when Claude calls them anyway: `start_chat` ("start_chat isn't available in Claude Desktop: ask the user here, in this chat.") and `update_checkout` ("update_checkout isn't available in Claude Desktop: it works only in Hersir's chat, where its result comes as a message.").
- **The Chief's rules.** Claude also gets `get_chief_brief`: a line on how to work from Claude Desktop ("You are working as Hersir's Chief for `<you>`, from Claude Desktop. …"), then the Chief's instructions from its agent form, or the Chief's standard ones when that is empty.
- **As you, at once.** Claude's calls run at once, as you: a card's activity and comments show your name, and a card Claude makes is yours.
- **Claude Desktop's Allow prompt is the check.** Hersir can't see what else Claude used in its chat, like the web or another MCP server, so Hersir's rule that makes the Chief's changes wait for your Approve after a tool from outside Hersir doesn't apply here ([Safety](tools.md#safety)). Read what Claude wants to do before you allow a Hersir tool that changes something, above all in a chat where Claude read a web page or another server's answer.
- **Files.** `attach_file` takes only a file inside the Chief's copies of the projects' code.
- **Merging.** `merge_pull_request` keeps every rule it has in Hersir ([The Chief may merge](testing.md#the-chief-may-merge)).

## What stays yours, in Hersir

Some things only you do, and only in Hersir, also when Claude has the Chief's tools:

- **Approving kept calls.** Calls kept for your Approve, like the Chief's changes after a tool from outside Hersir, wait under the Chief's answer in Hersir's chat ([Approve with one click](chief.md#approve-with-one-click)). Only you approve or discard them, there: no tool can, Claude's neither.
- **Holds that wait for your OK.** A card on hold that waits for your OK after a tool from outside Hersir comes off hold only on the card, in Hersir (Clear hold, Continue, or a move by hand). When Claude tries, it gets "Only the user takes this hold off, on the card: it waits for their OK after a tool from outside Hersir.", and nothing changes. A card Claude moves keeps its hold.
- **New commands for an agent.** When Claude gives an agent a command it doesn't have yet, beyond the few safe ones in Hersir's default list, the change isn't made: in Hersir's chat it would wait for your Approve, and Claude Desktop has no Hersir chat to keep it in. Claude hears so, and that you can add it yourself in the agent form (Permissions → Allowed commands).
- **Tools and servers.** As for the Chief: only you add, sign in to and switch on MCP servers, the web tools, the browser and built-in tools ([Tools per agent](tools.md)).

## Ask the Chief

Claude can hand a message to Hersir's own Chief, which answers with its memory, its copies of the code and the board. Ask Claude something like "Ask the Chief in Hersir which cards are stuck".

- **Your message, in Hersir's chat.** The message is yours, as if you typed it in Hersir's chat: it starts the Chief's answer, or waits in the chat's queue while the Chief answers. It goes to the chat **From Claude Desktop** in Hersir, made the first time and the same one after that, also after you rename it. You can carry on with it in Hersir.
- **The answer.** Claude waits for the Chief's answer and gives it in its chat. The Chief can take longer than Claude waits for one tool, so Claude asks again until it is done. Claude sees whether the Chief is working, has answered, or waits for you: calls of its answer wait for your Approve, or your message waits for Send now in Hersir.
- **The Chief's changes** run at once, unless the Chief itself uses a tool from outside Hersir in that chat: then they wait for your Approve in Hersir's chat, as always.
- **Long texts** over 8,000 characters are cut, with "…(cut: read the rest in Hersir)".
- **Costs.** Each message uses Claude twice: once in Claude Desktop, and once for the Chief's answer in Hersir, on the Chief's Claude Code account.

## The board, a card and the Chief's thread in the chat

When Claude Desktop shows MCP Apps (views from an MCP server inside the chat), Claude can show part of Hersir in its chat. Ask "Show me the board of project GA in Hersir", "Show me GA-12" or "Show me the Chief's thread". Each view keeps itself up to date while it is shown, and says when: "Updated" and the time. Its colours follow Claude's light or dark theme.

- **The board.** In the chat, a summary: each column with its number of cards, then **Needs you** and **Agents working**, with up to 8 cards each ("And 4 more: Open board shows them all."). **Open board** shows the whole board, full screen: drag a card to another column to move it there, at the bottom. A click on a card opens it in the view, and **Back** returns to the board.
- **A card.** Its column, assignee, priority, labels, pull request and hold, its description and acceptance criteria, its last 20 comments and last 5 runs, and, while an agent works on it, "`<agent>` is working" with the run's output as it comes. **Comment** opens a box ("Write a comment. Markdown works.") with **Post comment** (or Ctrl+Enter, Cmd+Enter on macOS) and **Cancel**. **Move to** lists the other columns. **Run** starts the card's agent (the assigned one, else the column's first); while one works, it is **Stop**.
- **The Chief's thread.** The chat From Claude Desktop, with whether the Chief is working ("The Chief is working"), waits for you ("Waits for you in Hersir": approve its calls, or send your message, in Hersir's chat) or has answered ("Answered"). Write in **Message the Chief** and press **Send** (or Ctrl+Enter); the Chief's answer comes in as it writes.
- **Full screen.** Open board, and **Open full screen** on a card or the Chief's thread, show the view full screen. **Back to chat** returns. On the board, **Show the summary** goes back to the summary.

The view changes things with the same tools Claude uses (move, comment, run, stop and ask the Chief), with the same rules: when Hersir refuses, its reason shows by the button. The view itself reaches nothing on the internet: everything it needs is inside it, and its data comes from Hersir through Claude.

A Claude Desktop that doesn't show MCP Apps gets no views; Claude answers in words, with the same tools.

## The key

The entry in Claude Desktop's settings holds a key with the Chief's powers over your board: any program running as you could use it. Treat it like a password.

- Hersir shows the key only when you connect, and keeps only a fingerprint of it, never the key itself.
- The key lasts until you press Disconnect or Connect again.
- When Hersir writes Claude Desktop's settings file, only you can read it on macOS and Linux. Every program you run can still read it.
- If you think another program got the key, press **Connect again** (a new key, the old one ends) or **Disconnect**.

## One-click setup with a Desktop Extension: not yet

Claude Desktop can also install a server from a Desktop Extension, an `.mcpb` file you double-click. We tried one for Hersir (GA-346): it can be built, but Connect stays the way to set up Claude Desktop.

- **Connect is one click already** when Claude Desktop's settings are found, and works on macOS, Windows and Linux. Claude's documentation for extensions names only macOS and Windows.
- **An extension still needs the key.** Its install form would ask you to paste the key and Hersir's socket from Hersir, or the key would sit inside the `.mcpb` file.
- **A copy of `hersir-mcp`** goes inside the extension, and wouldn't update with Hersir.
- **Whether views show for an extension is unconfirmed.** Agents can't run Claude Desktop to check, and an open report about Claude Desktop says views from local servers don't always show.

[docs/research/claude-desktop-mcp.md](../research/claude-desktop-mcp.md#a-desktop-extension-mcpb-ga-346) has what we tried, and how to check it on a Mac.

## Read more

- [The Chief](chief.md): what it can do, and the rules it works by.
- [docs/agent-tools.md](../agent-tools.md#safety-rules): every tool and rule, Claude Desktop's included.
- [Settings](../settings.md): where each setting lives.
- [docs/PLATFORMS.md](../PLATFORMS.md): where Claude Desktop's settings are on each system.
