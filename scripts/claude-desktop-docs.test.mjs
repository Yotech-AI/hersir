// GA-346 QA: the docs for Claude Desktop. docs/features/claude-desktop.md covers each point of the card (Connect and
// Disconnect, Open Hersir first, Claude working like the Chief, what stays yours in Hersir, asking the Chief, the views and
// the key), and docs/install.md has a Claude Desktop section that links to it. Every text the doc quotes from the app (the
// settings' words and buttons, hersir-mcp's and Hersir's answers, the views' buttons) is the app's own, read from the
// source, so a change of wording there fails here until the doc follows. The .mcpb trial is written down in
// docs/research/claude-desktop-mcp.md: it builds, views are unconfirmed, and the docs recommend Connect. The links in the
// docs this card changed reach a file and its heading.
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { CD_ADD, CD_ASK_TEXT, CD_ASK_TITLE, CD_NO_CONFIG, CD_NO_ENTRY, CD_NO_SHIM, CD_PASTE, CD_REOPEN, addedText, backupText,
  connectedText } from "../src/lib/claudeDesktop.ts";

const ROOT = fileURLToPath(new URL("..", import.meta.url));
const read = (path) => readFileSync(join(ROOT, path), "utf8");
const DOC = "docs/features/claude-desktop.md";
const doc = read(DOC);

/** A Markdown file's `## title` section, up to the next heading of that level or higher. */
function section(md, title, level = 2) {
  const head = `${"#".repeat(level)} ${title}\n`;
  const at = md.indexOf(`\n${head}`);
  if (at < 0) return null;
  const rest = md.slice(at + head.length + 1);
  const next = rest.search(new RegExp(`^#{1,${level}} `, "m"));
  return next < 0 ? rest : rest.slice(0, next);
}

/** A Rust `&str` const as the program has it: a `\` at a line's end drops the line break and the next line's indent. */
function rustConst(path, name) {
  const m = read(path).match(new RegExp(`pub(?:\\([a-z]+\\))? const ${name}: &str =\\s*"((?:[^"\\\\]|\\\\[\\s\\S])*)";`));
  if (!m) throw new Error(`${name} isn't a &str const in ${path}`);
  return m[1].replace(/\\\n\s*/g, "").replace(/\\"/g, '"');
}

describe("the helpers", () => {
  it("section() stops at the next heading of its level, and rustConst() joins a continued line", () => {
    expect(section("# A\n\n## B\ntext\n### C\nmore\n## D\nlast\n", "B")).toBe("text\n### C\nmore\n");
    expect(rustConst("crates/gizai-mcp/src/desktop.rs", "REFUSED")).toBe(
      "Hersir disconnected Claude Desktop. To connect it again: in Hersir, Settings → MCP servers → Claude Desktop → Connect, then quit and reopen Claude Desktop.");
  });
});

describe("docs/features/claude-desktop.md covers each point of the card", () => {
  it("exists, titled Claude Desktop, with a section for each point", () => {
    expect(existsSync(join(ROOT, DOC))).toBe(true);
    expect(doc.split("\n")[0]).toBe("# Claude Desktop");
    for (const title of ["Connect", "Open Hersir first", "Claude works like the Chief", "What stays yours, in Hersir", "Ask the Chief",
      "The board, a card and the Chief's thread in the chat", "The key"]) expect(section(doc, title), title).not.toBeNull();
  });

  it("connecting in Settings → MCP servers → Claude Desktop, and Disconnect", () => {
    const connect = section(doc, "Connect");
    expect(connect).toContain("Settings → MCP servers → **Claude Desktop**");
    expect(connect).toContain("press **Connect**");
    expect(section(connect, "Connect again and Disconnect", 3)).toMatch(/\*\*Disconnect\*\* asks first/);
  });

  it("that Claude works like the Chief, with its tools and rules (GA-341)", () => {
    const like = section(doc, "Claude works like the Chief");
    for (const part of ["**The Chief's tools.**", "**The Chief's rules.**", "`get_chief_brief`", "`start_chat`", "`update_checkout`", "as you"]) expect(like).toContain(part);
  });

  it("what stays yours in Hersir: approving kept calls, holds that wait for your OK, and new commands for an agent", () => {
    const yours = section(doc, "What stays yours, in Hersir");
    for (const part of ["**Approving kept calls.**", "**Holds that wait for your OK.**", "**New commands for an agent.**"]) expect(yours).toContain(part);
  });

  it("asking the Chief (GA-342), and the views", () => {
    expect(section(doc, "Ask the Chief")).toContain("the chat **From Claude Desktop**");
    const views = section(doc, "The board, a card and the Chief's thread in the chat");
    for (const part of ["**The board.**", "**A card.**", "**The Chief's thread.**", "**Full screen.**"]) expect(views).toContain(part);
  });

  it('"Open Hersir first"', () => {
    expect(section(doc, "Open Hersir first")).toContain(rustConst("crates/gizai-mcp/src/desktop.rs", "NOT_RUNNING"));
  });

  it("that the entry holds a key with the Chief's powers, which any program running as you could use", () => {
    expect(section(doc, "The key")).toMatch(/^The entry in Claude Desktop's settings holds a key with the Chief's powers over your board: any program running as you could use it\./m);
  });
});

describe("docs/install.md has a Claude Desktop section that links to the feature doc", () => {
  const install = read("docs/install.md");
  const heads = install.split("\n").filter((l) => l.startsWith("## "));

  it("after First start and before Updates", () => {
    expect(heads.slice(heads.indexOf("## First start"), heads.indexOf("## First start") + 3)).toEqual(["## First start", "## Claude Desktop", "## Updates"]);
  });

  it("with the Settings path, Connect, Open Hersir first, Disconnect and the key, and a link to features/claude-desktop.md", () => {
    const s = section(install, "Claude Desktop");
    expect(s).toContain("Settings → MCP servers → **Claude Desktop**");
    expect(s).toContain(`leave **${CD_ADD}** on and press **Connect**`);
    expect(s).toContain('"Open Hersir first"');
    expect(s).toContain("**Disconnect**");
    expect(s).toContain("a key with the Chief's powers over your board, which any program running as you could use");
    expect(s).toContain("](features/claude-desktop.md)");
  });

  it("Read more and the docs' indexes link to it too", () => {
    expect(section(install, "Read more")).toContain("- [Claude Desktop](features/claude-desktop.md)");
    expect(read("README.md").split("\n## Documentation\n")[1]).toContain("[Claude Desktop](docs/features/claude-desktop.md)");
    expect(read("docs/settings.md")).toContain("[Claude Desktop](features/claude-desktop.md)");
  });
});

describe("the doc names the Settings path, buttons and messages as the app shows them", () => {
  it("Settings → MCP servers → Claude Desktop: its words, Connect's answers, Copy and the paste text", () => {
    const connect = section(doc, "Connect");
    expect(connect).toContain(`Leave **${CD_ADD}** on`);
    expect(connect).toContain(`Hersir says "${addedText("`<the file>`")}" When the file was there already, it adds "${backupText("`<the copy>`")}" Then: "${CD_REOPEN}"`);
    expect(connect).toContain(`like "${connectedText({ connectedAt: new Date(2026, 10, 23, 14, 2).getTime() })}"`);
    expect(connect).toContain(`with "${CD_NO_CONFIG}"`);
    expect(connect).toContain("Then Connect shows the entry, with **Copy**, and:");
    expect(connect).toContain(`\n> ${CD_PASTE}\n`);
    expect(connect).toContain(`like "${rustConst("src-tauri/src/claude_desktop.rs", "NOT_JSON")}"`);
    expect(connect).toContain(`shows "${CD_NO_ENTRY}"`);
    expect(connect).toContain(`Connect is greyed, with "${CD_NO_SHIM}"`);
    // the component's buttons: Connect, Connect again, Disconnect, Cancel and Copy
    const ui = read("src/components/ClaudeDesktopSettings.tsx");
    for (const label of ['"Connect"', '"Connect again"', '"Disconnect"', ">Cancel<", "/>Copy<"]) expect(ui).toContain(label);
  });

  it("Disconnect's question, its buttons, and what Claude hears afterwards", () => {
    const off = section(doc, "Connect again and Disconnect", 3);
    expect(off).toContain(`"${CD_ASK_TITLE}", with "${CD_ASK_TEXT}" Press **Disconnect** (or **Cancel**).`);
    expect(off).toContain(`this answer for every Hersir tool: "${rustConst("crates/gizai-mcp/src/desktop.rs", "REFUSED")}"`);
  });

  it("hersir-mcp's answers while Hersir is closed, and hersir_status's", () => {
    const open = section(doc, "Open Hersir first");
    const mcp = "crates/gizai-mcp/src/desktop.rs";
    expect(open).toContain(`one Hersir tool, \`${rustConst(mcp, "STATUS_TOOL")}\`, and every Hersir call answers: "${rustConst(mcp, "NOT_RUNNING")}"`);
    expect(open).toContain(`that answers "${rustConst(mcp, "CONNECTED")}"`);
    // the stand-in host's view says the same
    expect(read("src/widget-host/main.ts")).toContain(rustConst(mcp, "NOT_RUNNING"));
  });

  it("the Chief's tools Claude Desktop isn't offered, get_chief_brief's line, and the hold only you take off", () => {
    const tools = "src-tauri/src/tools/mod.rs";
    const like = section(doc, "Claude works like the Chief");
    expect(like).toContain(`\`start_chat\` ("${rustConst(tools, "START_CHAT_IN_CLAUDE_DESKTOP")}")`);
    expect(like).toContain(`\`update_checkout\` ("${rustConst(tools, "UPDATE_CHECKOUT_IN_CLAUDE_DESKTOP")}")`);
    expect(like).toContain("(\"You are working as Hersir's Chief for `<you>`, from Claude Desktop. …\")");
    expect(read(tools)).toContain('format!("You are working as Hersir\'s Chief for {you}, from Claude Desktop. ');
    const yours = section(doc, "What stays yours, in Hersir");
    expect(yours).toContain(`it gets "${rustConst(tools, "ONLY_THE_USER")}"`);
    expect(yours).toContain("(Permissions → Allowed commands)");
    expect(read(tools)).toContain("in the agent form (Permissions → Allowed commands)");
  });

  it("asking the Chief: the thread's name and the cut", () => {
    const ask = section(doc, "Ask the Chief");
    const desktop = "src-tauri/src/tools/desktop.rs";
    expect(read(desktop)).toContain("pub const MAX_TEXT: usize = 8000;");
    expect(ask).toContain(`**Long texts** over 8,000 characters are cut, with "${rustConst(desktop, "CUT")}"`);
    expect(read(desktop)).toContain("the chat From Claude Desktop");
  });

  it("the views: their buttons, words and limits", () => {
    const views = section(doc, "The board, a card and the Chief's thread in the chat");
    const app = read("src/widget/App.tsx"), board = read("src/widget/BoardView.tsx"), card = read("src/widget/CardView.tsx");
    const chief = read("src/widget/ChiefView.tsx"), apps = read("src-tauri/src/tools/apps.rs");
    // [what the doc says, the source that shows it]
    const pairs = [
      ['"Updated" and the time', [app, "Updated {updated.toLocaleTimeString()}"]],
      ["then **Needs you** and **Agents working**, with up to 8 cards each", [board, ">Needs you <"], [board, ">Agents working <"], [board, "const LIST_MAX = 8;"]],
      ['("And 4 more: Open board shows them all.")', [board, "And {cards.length - LIST_MAX} more: Open board shows them all."]],
      ["**Open board** shows the whole board", [app, "/>Open board</button>"]],
      ["**Back** returns to the board", [app, "/>Back</button>"]],
      ["its last 20 comments and last 5 runs", [apps, "pub const COMMENTS: usize = 20;"], [apps, "pub const RUNS: usize = 5;"]],
      ['"`<agent>` is working"', [card, '{agent || "An agent"} is working']],
      ['**Comment** opens a box ("Write a comment. Markdown works.") with **Post comment** (or Ctrl+Enter, Cmd+Enter on macOS) and **Cancel**',
        [card, "/>Comment\n"], [card, 'placeholder="Write a comment. Markdown works."'], [card, '"Post comment"'], [card, '(e.ctrlKey || e.metaKey)'], [card, ">Cancel</button>"]],
      ["**Move to** lists the other columns", [card, "/>}Move to\n"]],
      ["**Run** starts the card's agent", [card, "/>Run</>"]],
      ["it is **Stop**", [card, "/>Stop</>"]],
      ['("The Chief is working")', [chief, "/>The Chief is working</span>"]],
      ['("Waits for you in Hersir"', [chief, ">Waits for you in Hersir</span>"]],
      ['("Answered")', [chief, '<span className="badge">Answered</span>']],
      ["Write in **Message the Chief** and press **Send** (or Ctrl+Enter)", [chief, 'placeholder="Message the Chief"'], [chief, "/>Send</>"], [chief, "(e.ctrlKey || e.metaKey)"]],
      ["**Open full screen** on a card or the Chief's thread", [app, "/>Open full screen</button>"]],
      ["**Back to chat** returns", [app, "/>Back to chat</button>"]],
      ["**Show the summary** goes back to the summary", [app, ">Show the summary</button>"]],
    ];
    for (const [said, ...shown] of pairs) {
      expect(views, said).toContain(said);
      for (const [src, text] of shown) expect(src, `${said}: ${text}`).toContain(text);
    }
  });
});

describe("the .mcpb result is written down", () => {
  const research = read("docs/research/claude-desktop-mcp.md");
  const mcpb = section(research, "A Desktop Extension (`.mcpb`, GA-346)");

  it("docs/research/claude-desktop-mcp.md: views unconfirmed, Connect recommended, and a checklist to confirm it", () => {
    expect(mcpb).not.toBeNull();
    expect(mcpb).toContain("**Result: an `.mcpb` for Hersir builds and validates; whether Claude Desktop shows its views is not confirmed. The docs\nrecommend Connect (the settings entry), and Hersir offers no `.mcpb`.**");
    expect(mcpb).toContain("| Views | Shown when Claude Desktop shows MCP Apps (GA-343's checklist) | Unconfirmed |");
    expect(section(mcpb, "Jefsev's checklist for the `.mcpb` (Mac)", 3)).toContain("| Does the board view show inline? | |");
  });

  it("its manifest is JSON whose server is hersir-mcp --desktop with the variables the shim reads, and the key a sensitive setting", () => {
    const manifest = JSON.parse(mcpb.match(/^```json\n([\s\S]*?)^```$/m)[1]);
    expect(manifest.manifest_version).toBe("0.3");
    expect(manifest.server).toEqual({ type: "binary", entry_point: "server/hersir-mcp", mcp_config: {
      command: "${__dirname}/server/hersir-mcp", args: ["--desktop"], env: { HERSIR_SOCKET: "${user_config.socket}", HERSIR_TOKEN: "${user_config.key}" } } });
    expect(manifest.user_config.key).toMatchObject({ type: "string", sensitive: true, required: true });
    expect(manifest.user_config.key.description).toContain(`Settings → MCP servers → Claude Desktop → Connect, with ${CD_ADD} off.`);
    // the entry Connect writes, and the shim, use the same flag and names
    const entry = read("crates/gizai-agents/src/claude_desktop.rs");
    for (const part of ['args: vec!["--desktop".into()]', '#[serde(rename = "HERSIR_SOCKET")]', '#[serde(rename = "HERSIR_TOKEN")]']) expect(entry).toContain(part);
    expect(read("crates/gizai-mcp/src/bin/gizai-mcp.rs")).toContain('a == "--desktop"');
  });

  it("the feature doc and install.md say Connect is the way, and why there is no .mcpb", () => {
    const oneClick = section(doc, "One-click setup with a Desktop Extension: not yet");
    expect(oneClick).toContain("Connect stays the way to set up Claude Desktop.");
    expect(oneClick).toContain("**Whether views show for an extension is unconfirmed.**");
    expect(oneClick).toContain("](../research/claude-desktop-mcp.md#a-desktop-extension-mcpb-ga-346)");
    expect(section(read("docs/install.md"), "Claude Desktop")).toContain("Connect is the way to set it up: there is no Desktop Extension (`.mcpb`) for Hersir.");
  });
});

// GitHub's heading anchors (as in community-files.test.mjs): lower case, without punctuation, a hyphen for each space.
const slug = (heading) => heading.trim().toLowerCase().replace(/[^\p{L}\p{N}\s_-]/gu, "").replace(/\s/g, "-");
const withoutCode = (md) => md.replace(/^ *```[\s\S]*?^ *```/gm, (block) => block.replace(/[^\n]/g, ""));
const anchors = (path) => withoutCode(read(path)).split("\n").filter((l) => /^#{1,6} /.test(l)).map((l) => slug(l.replace(/^#+ /, "")));

describe("the links in the docs this card changed", () => {
  it.each([DOC, "docs/install.md", "docs/settings.md", "docs/agent-tools.md", "docs/research/claude-desktop-mcp.md"])("every link in %s reaches a file and its heading", (path) => {
    const broken = [];
    for (const [, target] of withoutCode(read(path)).matchAll(/\]\(([^)\s]+)\)/g)) {
      if (/^[a-z]+:/.test(target)) continue;
      const [file, anchor] = target.split("#");
      const to = file ? normalize(join(dirname(path), file)) : path;
      if (!existsSync(join(ROOT, to)) || (anchor && !anchors(to).includes(anchor))) broken.push(target);
    }
    expect(broken).toEqual([]);
  });
});
