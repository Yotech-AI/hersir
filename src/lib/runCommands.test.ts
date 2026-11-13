// GA-279 QA: what a Chief answer asks you to run, and the folders it names, for Open in terminal under it (rules 1 and 2 of
// the card). The message from the card's Why, the counter-examples of both rules, a ```sh block with and without $ prompts,
// each shell language and a few that aren't, and the folder a path resolves to: the Editor's project or root folder it is or
// lies in (the longest one), never a card's worktree, at most 3 in the order named.
import { describe, expect, it } from "vitest";
import type { EditorFolder } from "../types";
import { MOST_FOLDERS, SHELL_LANGUAGES, folderOf, namedPaths, runCommands, terminalFolders } from "./runCommands";

const HOME = "/home/you";
const folder = (id: string, more: Partial<EditorFolder> = {}): EditorFolder =>
  ({ id, path: id.replace(HOME, "~"), name: id.split("/").pop()!, kind: "root", projectId: null, taskId: null, readOnly: false, readOnlyWhy: null, ...more });
const DOQ = folder(`${HOME}/Herd/doq-studio`, { kind: "project", projectId: "p1" });
const SHOP = folder(`${HOME}/Herd/shop`);
const KADE = folder(`${HOME}/Herd/kade-portal`);
const API = folder(`${HOME}/Herd/invoice-api`);
const CARD = folder(`${HOME}/.local/share/gizai/worktrees/GA/GA-279`, { kind: "card", taskId: "t279" });
const FOLDERS = [DOQ, SHOP, KADE, API, CARD];

// The card's Why, as the Chief wrote it
const WHY = "Before you test, in `~/Herd/doq-studio`:\n\n1. Run `php artisan migrate`.\n2. Run `npm run build`.";

describe("the commands a message asks you to run (rule 1)", () => {
  it("finds the two commands of the card's Why, in order", () => {
    expect(runCommands(WHY)).toEqual(["php artisan migrate", "npm run build"]);
  });

  it("counts a code span after run, type, paste or execute, in any case", () => {
    expect(runCommands("Run `a`, then run `b`. Type `c`, PASTE `d` and Execute `e`.")).toEqual(["a", "b", "c", "d", "e"]);
    expect(runCommands("then run `php -S 127.0.0.1:8081 -t resources/embed-test`")).toEqual(["php -S 127.0.0.1:8081 -t resources/embed-test"]);
  });

  it("allows this, these, the command, the commands or a colon in between", () => {
    expect(runCommands("run this: `npm run build`")).toEqual(["npm run build"]);
    expect(runCommands("Run this `npm ci`")).toEqual(["npm ci"]);
    expect(runCommands("Run these: `composer install`")).toEqual(["composer install"]);
    expect(runCommands("Run the command `php artisan test`")).toEqual(["php artisan test"]);
    expect(runCommands("Run the commands: `git pull`")).toEqual(["git pull"]);
    expect(runCommands("Run: `npm test`")).toEqual(["npm test"]);
    expect(runCommands("**Run:** `npm test`")).toEqual(["npm test"]);
  });

  it("doesn't count a code span after other words", () => {
    expect(runCommands("QA ran `npm test` and it passed.")).toEqual([]);
    expect(runCommands("The change is in the file `src/api.ts`.")).toEqual([]);
    expect(runCommands("I'll rerun `npm test` later.")).toEqual([]);
    expect(runCommands("Run it with `npm test`.")).toEqual([]);
    expect(runCommands("While running `npm test`, the build broke.")).toEqual([]);
  });

  it("counts only the span right after the word: Run `a` and `b` asks for a", () => {
    expect(runCommands("Run `npm ci` and `npm run build`.")).toEqual(["npm ci"]);
  });

  it("finds nothing in plain text, or in a message without code", () => {
    expect(runCommands("Run npm test in ~/Herd/doq-studio, then run php artisan migrate.")).toEqual([]);
    expect(runCommands("")).toEqual([]);
    expect(runCommands("All done: GA-279 is in Review.")).toEqual([]);
  });

  it("counts each line of a ```sh block, with or without $ prompts", () => {
    const md = "In `~/Herd/doq-studio`:\n\n```sh\nphp artisan migrate\nnpm run build\n```";
    expect(runCommands(md)).toEqual(["php artisan migrate", "npm run build"]);
    const prompts = "```sh\n$ php artisan migrate\n$ npm run build\n```";
    expect(runCommands(prompts)).toEqual(["php artisan migrate", "npm run build"]);
    expect(runCommands("```sh\n> npm test\n```")).toEqual(["npm test"]);
  });

  it("leaves out empty lines and lines starting with #", () => {
    const md = "```bash\n# first the database\nphp artisan migrate\n\n   \n# then the assets\n$ npm run build\n```";
    expect(runCommands(md)).toEqual(["php artisan migrate", "npm run build"]);
  });

  it("counts a block in each shell language, in any case", () => {
    expect(SHELL_LANGUAGES).toEqual(["sh", "bash", "zsh", "shell", "console", "terminal", "fish", "powershell", "pwsh", "ps1", "cmd"]);
    for (const lang of SHELL_LANGUAGES) {
      expect(runCommands(`\`\`\`${lang}\nnpm test\n\`\`\``), lang).toEqual(["npm test"]);
      expect(runCommands(`\`\`\`${lang.toUpperCase()}\nnpm test\n\`\`\``), lang.toUpperCase()).toEqual(["npm test"]);
    }
  });

  it("doesn't count a code block in another language, or one without a language", () => {
    for (const lang of ["php", "js", "ts", "json", "text", "diff", "", "shellscript", "sql"]) {
      expect(runCommands(`\`\`\`${lang}\nnpm test\nphp artisan migrate\n\`\`\``), lang || "no language").toEqual([]);
    }
  });

  it("doesn't count Run before a code block in another language", () => {
    expect(runCommands("Run this:\n\n```php\nArtisan::call('migrate');\n```")).toEqual([]);
  });

  it("keeps the order of blocks and spans as written", () => {
    const md = "First run `composer install`.\n\n```sh\nphp artisan migrate\n```\n\nThen run `npm run build`.";
    expect(runCommands(md)).toEqual(["composer install", "php artisan migrate", "npm run build"]);
  });

  it("doesn't take a span inside a code block as a command of its own", () => {
    expect(runCommands("```php\n// run `php artisan migrate` first\n```")).toEqual([]);
  });
});

describe("the folders a message names (rule 2)", () => {
  it("finds the folder of the card's Why, without the colon after it", () => {
    expect(namedPaths(WHY)).toEqual(["~/Herd/doq-studio"]);
  });

  it("finds paths in code and in the text, starting with ~/ or /, in the order named", () => {
    expect(namedPaths("In ~/Herd/shop and `/home/you/Herd/kade-portal`, then ~/Herd/invoice-api")).toEqual(
      ["~/Herd/shop", "/home/you/Herd/kade-portal", "~/Herd/invoice-api"]);
  });

  it("leaves off a trailing /, ., ,, : or ;", () => {
    expect(namedPaths("Go to ~/Herd/shop/.")).toEqual(["~/Herd/shop"]);
    expect(namedPaths("In ~/Herd/a, ~/Herd/b: ~/Herd/c; ~/Herd/d/ and ~/Herd/e.")).toEqual(
      ["~/Herd/a", "~/Herd/b", "~/Herd/c", "~/Herd/d", "~/Herd/e"]);
  });

  it("doesn't count a URL's path or a path after other characters", () => {
    expect(namedPaths("Open http://127.0.0.1:8081/page.html in the browser.")).toEqual([]);
    expect(namedPaths("Load `<APP_URL>/embed/v1/loader.js` on the page.")).toEqual([]);
    expect(namedPaths("See https://github.com/Yotech-AI/hersir/pull/226.")).toEqual([]);
    expect(namedPaths("The test page is in resources/embed-test.")).toEqual([]);
  });

  it("names each path once", () => {
    expect(namedPaths("In `~/Herd/shop`: run `a`. Back in ~/Herd/shop, run `b`.")).toEqual(["~/Herd/shop"]);
  });
});

describe("the Editor folder a path is in", () => {
  it("is the folder whose path (with ~) or id (absolute) it is", () => {
    expect(folderOf("~/Herd/doq-studio", FOLDERS)).toBe(DOQ);
    expect(folderOf(`${HOME}/Herd/doq-studio`, FOLDERS)).toBe(DOQ);
    expect(folderOf("~/Herd/shop", FOLDERS)).toBe(SHOP);
  });

  it("is the folder it lies inside: a subfolder or a file opens the terminal in the folder", () => {
    expect(folderOf("~/Herd/doq-studio/resources/embed-test", FOLDERS)).toBe(DOQ);
    expect(folderOf(`${HOME}/Herd/doq-studio/app/Http/Kernel.php`, FOLDERS)).toBe(DOQ);
  });

  it("is the longest one: a project inside a root folder is the project", () => {
    const HERD = folder(`${HOME}/Herd`);
    expect(folderOf("~/Herd/doq-studio/resources", [HERD, DOQ])).toBe(DOQ);
    expect(folderOf("~/Herd/doq-studio/resources", [DOQ, HERD])).toBe(DOQ);
    expect(folderOf("~/Herd/other", [DOQ, HERD])).toBe(HERD);
  });

  it("is never a folder whose name only starts the same", () => {
    expect(folderOf("~/Herd/doq-studio-old", FOLDERS)).toBeNull();
    expect(folderOf("~/Herd/shopping/app", FOLDERS)).toBeNull();
  });

  it("is never a card's worktree, and any other path is none", () => {
    expect(folderOf(`${HOME}/.local/share/gizai/worktrees/GA/GA-279`, FOLDERS)).toBeNull();
    expect(folderOf("~/.local/share/gizai/worktrees/GA/GA-279/src", FOLDERS)).toBeNull();
    expect(folderOf("/etc/hosts", FOLDERS)).toBeNull();
    expect(folderOf("~/Downloads", FOLDERS)).toBeNull();
    expect(folderOf("~/Herd/doq-studio", [])).toBeNull();
  });
});

describe("the folders Open in terminal opens", () => {
  it("is ~/Herd/doq-studio for the card's Why", () => {
    expect(terminalFolders(WHY, FOLDERS)).toEqual([DOQ]);
  });

  it("is ~/Herd/doq-studio for a subfolder of it, and for its absolute path", () => {
    expect(terminalFolders("In `~/Herd/doq-studio/resources/embed-test`: run `php -S 127.0.0.1:8081`", FOLDERS)).toEqual([DOQ]);
    expect(terminalFolders(`In \`${HOME}/Herd/doq-studio\`: run \`npm test\``, FOLDERS)).toEqual([DOQ]);
  });

  it("is each folder once, also when the message names it twice or by a subfolder", () => {
    const md = "In `~/Herd/doq-studio`: run `a`. Then in ~/Herd/doq-studio/resources/embed-test run `b`, and in /home/you/Herd/doq-studio run `c`.";
    expect(terminalFolders(md, FOLDERS)).toEqual([DOQ]);
  });

  it("is two folders in the order named", () => {
    const md = "In `~/Herd/shop`: run `npm run build`. Then in `~/Herd/doq-studio`: run `php artisan migrate`.";
    expect(terminalFolders(md, FOLDERS)).toEqual([SHOP, DOQ]);
    const other = "In `~/Herd/doq-studio`: run `php artisan migrate`. Then in `~/Herd/shop`: run `npm run build`.";
    expect(terminalFolders(other, FOLDERS)).toEqual([DOQ, SHOP]);
  });

  it("is never more than 3", () => {
    expect(MOST_FOLDERS).toBe(3);
    const md = "Run `npm ci` in ~/Herd/shop, ~/Herd/kade-portal, ~/Herd/invoice-api and ~/Herd/doq-studio.";
    expect(terminalFolders(md, FOLDERS)).toEqual([SHOP, KADE, API]);
  });

  it("skips paths that resolve to nothing, and counts the next ones", () => {
    const md = "Run `npm ci` in ~/Downloads, the worktree ~/.local/share/gizai/worktrees/GA/GA-279, /etc and ~/Herd/shop.";
    expect(terminalFolders(md, FOLDERS)).toEqual([SHOP]);
  });

  it("is none for a URL's path, a path after other characters, or no path", () => {
    expect(terminalFolders("Run `npm run dev` and open http://127.0.0.1:8081/Herd/doq-studio", FOLDERS)).toEqual([]);
    expect(terminalFolders("Run `npm test`.", FOLDERS)).toEqual([]);
  });
});
