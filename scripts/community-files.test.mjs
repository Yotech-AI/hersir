// Tests for the repository's community files and one name in the docs (GA-201): the README's first screen, word for
// word where it counts; the issue forms in .github/ISSUE_TEMPLATE keep the rules of GitHub's issue forms check
// (docs.github.com, "Syntax for issue forms",
// "Syntax for GitHub's form schema" and "Common validation errors when creating issue forms"), read with the YAML parser
// the Editor already has (@codemirror/lang-yaml), as GA has no other; CONTRIBUTING.md, SECURITY.md, the pull request
// template and release.yml exist, and their links reach real files and headings; docs/RELEASING.md labels the bump and
// release pull requests and makes the release with the card's notes; and the docs say Hersir (GA-234), with "Gizai"
// left only in the names the code keeps, while the names installs and updates find GizAi by stay as they are (GA-233:
// but for productName and the window title, which are Hersir). GA-234 also checks the installers' messages,
// Cargo.toml's descriptions and browser.html's title.
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { dirname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";
import { yamlLanguage } from "@codemirror/lang-yaml";
import { describe, expect, it } from "vitest";

const ROOT = fileURLToPath(new URL("..", import.meta.url));
const read = (path) => readFileSync(join(ROOT, path), "utf8");
const git = (...args) => spawnSync("git", args, { cwd: ROOT, encoding: "utf8", maxBuffer: 64 * 1000 * 1000 });
// The address the docs and the issue forms link to: Yotech-AI/hersir since GA-234 (GitHub forwards Yotech-AI/gizai).
const REPO = "https://github.com/Yotech-AI/hersir";

// YAML to a plain value, for the YAML these files use: block and flow mappings and sequences, plain, quoted and block
// scalars. A plain yes, no, on, off, true or false is a boolean, as GitHub reads it.
function parseYaml(text) {
  const tree = yamlLanguage.parser.parse(text);
  const errors = [];
  tree.iterate({ enter: (n) => { if (n.type.isError) errors.push(text.slice(0, n.from).split("\n").length); } });
  if (errors.length) throw new Error(`YAML syntax error on line ${[...new Set(errors)].join(", ")}`);
  // a node's children without its comments and punctuation
  const parts = (node) => {
    const out = [];
    for (let c = node.firstChild; c; c = c.nextSibling) if (!["Comment", "Key", ":", "-", "[", "]", ",", "{", "}"].includes(c.name)) out.push(c);
    return out;
  };
  const value = (node) => {
    if (!node) return null;
    const src = text.slice(node.from, node.to);
    switch (node.name) {
      case "BlockMapping":
      case "FlowMapping": {
        const out = {};
        for (const pair of node.getChildren("Pair")) {
          const key = scalar(pair.getChild("Key").firstChild);
          if (Object.hasOwn(out, key)) throw new Error(`the key ${key} twice`);
          out[key] = value(parts(pair)[0]);
        }
        return out;
      }
      case "BlockSequence":
      case "FlowSequence":
        return node.getChildren("Item").map((item) => value(parts(item)[0]));
      case "Literal":
      case "QuotedLiteral":
        return scalar(node);
      case "BlockLiteral": {
        const header = text.slice(node.firstChild.from, node.firstChild.to);
        const lines = src.split("\n").slice(1);
        const indent = Math.min(...lines.filter((l) => l.trim()).map((l) => l.match(/^ */)[0].length));
        const body = lines.map((l) => l.slice(indent)).join("\n").replace(/\n+$/, "");
        return header.startsWith(">") ? body.replace(/([^\n])\n(?=[^\n])/g, "$1 ") : body;
      }
      default:
        throw new Error(`no YAML ${node.name} in these files: ${src}`);
    }
  };
  const scalar = (node) => {
    const src = text.slice(node.from, node.to);
    if (node.name === "QuotedLiteral") return src.startsWith("'") ? src.slice(1, -1).replace(/''/g, "'") : JSON.parse(src);
    if (/^(y|yes|n|no|true|false|on|off)$/i.test(src)) return /^(y|yes|true|on)$/i.test(src);
    if (/^(null|~)$/.test(src)) return null;
    if (/^-?\d+$/.test(src)) return Number(src);
    return src.replace(/\s*\n\s*/g, " ");
  };
  return value(parts(tree.topNode.getChild("Document"))[0]);
}

// What GitHub's issue forms check refuses, as a list of problems (empty when the form passes).
const TOP_KEYS = ["name", "description", "title", "labels", "assignees", "projects", "type", "body"];
const ELEMENT_KEYS = ["type", "id", "attributes", "validations"];
const ATTRIBUTES = {
  markdown: ["value"],
  textarea: ["label", "description", "placeholder", "value", "render"],
  input: ["label", "description", "placeholder", "value"],
  dropdown: ["label", "description", "multiple", "options", "default"],
  checkboxes: ["label", "description", "options"],
  upload: ["label", "description"],
};
const VALIDATIONS = { markdown: [], textarea: ["required", "min_length"], input: ["required", "min_length"], dropdown: ["required"], checkboxes: ["required"], upload: ["required", "accept"] };
const isText = (v) => typeof v === "string" && v.trim() !== "";
const isMap = (v) => v !== null && typeof v === "object" && !Array.isArray(v);
const parameterize = (s) => s.toLowerCase().replace(/[^a-z0-9_-]+/g, "-").replace(/^-|-$/g, "");

function formProblems(form) {
  const problems = [];
  if (!isMap(form)) return ["the form isn't a mapping"];
  for (const key of Object.keys(form)) if (!TOP_KEYS.includes(key)) problems.push(`${key} is not a permitted key`);
  for (const key of ["name", "description"]) if (!isText(form[key])) problems.push(`${key} must be a string`);
  if (form.labels !== undefined && !isText(form.labels) && !(Array.isArray(form.labels) && form.labels.every(isText))) problems.push("labels must be strings");
  if (!Array.isArray(form.body) || form.body.length === 0) return [...problems, "body cannot be empty"];
  const ids = [];
  const labels = [];
  form.body.forEach((el, i) => {
    const at = `body[${i}]`;
    if (!isMap(el)) return problems.push(`${at} isn't a mapping`);
    for (const key of Object.keys(el)) if (!ELEMENT_KEYS.includes(key)) problems.push(`${at}: ${key} is not a permitted key`);
    if (!(el.type in ATTRIBUTES)) return problems.push(`${at}: ${el.type} is not a valid input type`);
    const attrs = el.attributes;
    if (!isMap(attrs)) return problems.push(`${at}: attributes are missing`);
    for (const key of Object.keys(attrs)) if (!ATTRIBUTES[el.type].includes(key)) problems.push(`${at}: ${key} is not a permitted attribute`);
    for (const key of Object.keys(el.validations ?? {})) if (!VALIDATIONS[el.type].includes(key)) problems.push(`${at}: ${key} is not a permitted validation`);
    if (el.validations?.required !== undefined && typeof el.validations.required !== "boolean") problems.push(`${at}: required must be a boolean`);
    if (el.type === "markdown") {
      if (!isText(attrs.value)) problems.push(`${at}: required attribute key value is missing`);
      if (el.id !== undefined) problems.push(`${at}: markdown takes no id`);
      return;
    }
    if (el.id !== undefined) {
      if (!/^[A-Za-z0-9_-]+$/.test(String(el.id))) problems.push(`${at}: id can only contain numbers, letters, -, _`);
      ids.push(String(el.id));
    }
    if (!isText(attrs.label)) problems.push(`${at}: label must be a string`);
    else {
      labels.push(attrs.label);
      if (/password/i.test(attrs.label) && ["input", "textarea"].includes(el.type)) problems.push(`${at}: label contains forbidden word`);
    }
    if (el.type === "dropdown") {
      const options = attrs.options;
      if (!Array.isArray(options) || options.length === 0) return problems.push(`${at}: options cannot be empty`);
      if (options.some((o) => typeof o === "boolean")) problems.push(`${at}: options must not include booleans`);
      if (options.some((o) => !isText(o))) problems.push(`${at}: options must be strings`);
      if (new Set(options).size !== options.length) problems.push(`${at}: options must be unique`);
      if (options.some((o) => typeof o === "string" && o.trim().toLowerCase() === "none")) problems.push(`${at}: options must not include the reserved word, none`);
      if (attrs.multiple !== undefined && typeof attrs.multiple !== "boolean") problems.push(`${at}: multiple must be a boolean`);
      if (attrs.default !== undefined) {
        if (!Number.isInteger(attrs.default) || attrs.default < 0 || attrs.default >= options.length) problems.push(`${at}: default must be an option's index`);
        if (options.some((o) => typeof o === "string" && o.trim().toLowerCase() === "n/a")) problems.push(`${at}: with a default, options must not include n/a`);
      }
    }
    if (el.type === "checkboxes") {
      if (!Array.isArray(attrs.options) || attrs.options.length === 0) return problems.push(`${at}: options cannot be empty`);
      attrs.options.forEach((o, j) => {
        if (!isMap(o) || !isText(o.label)) return problems.push(`${at}: options[${j}] needs a label`);
        for (const key of Object.keys(o)) if (!["label", "required"].includes(key)) problems.push(`${at}: options[${j}]: ${key} is not a permitted key`);
        if (o.required !== undefined && typeof o.required !== "boolean") problems.push(`${at}: options[${j}]: required must be a boolean`);
        labels.push(o.label);
      });
    }
  });
  if (!form.body.some((el) => isMap(el) && el.type !== "markdown")) problems.push("body must contain at least one non-markdown field");
  if (new Set(ids).size !== ids.length) problems.push("body must have unique ids");
  if (new Set(labels).size !== labels.length) problems.push("body must have unique labels");
  else if (new Set(labels.map(parameterize)).size !== labels.length) problems.push("labels are too similar");
  return problems;
}

const TEMPLATES = ".github/ISSUE_TEMPLATE";
const FORMS = { bug: "bug.yml", feature: "feature.yml", install: "install.yml" };
const form = (name) => parseYaml(read(`${TEMPLATES}/${FORMS[name]}`));
const field = (f, id) => f.body.find((el) => el.id === id);
const required = (f) => f.body.filter((el) => el.validations?.required).map((el) => el.id);

describe("the issue forms check, as these tests read it", () => {
  const good = { name: "A", description: "B", body: [{ type: "textarea", id: "a", attributes: { label: "A" } }] };

  it("passes a small good form, and reads YAML's booleans, quotes and folded text as GitHub does", () => {
    expect(formProblems(good)).toEqual([]);
    expect(parseYaml("a: yes\nb: 'it''s'\nc: \"x: y\"\nd: >-\n  one\n  two\ne: [x, \"yes\", no]\n")).toEqual({ a: true, b: "it's", c: "x: y", d: "one two", e: ["x", "yes", false] });
    expect(() => parseYaml("a: [x\nb: y\n")).toThrow(/YAML syntax error/);
  });

  it("refuses what GitHub refuses", () => {
    const el = (type, attributes, more = {}) => ({ type, attributes, ...more });
    const with_ = (...body) => ({ ...good, body });
    expect(formProblems({ description: "B", body: good.body })).toContain("name must be a string");
    expect(formProblems({ ...good, input: 1 })).toContain("input is not a permitted key");
    expect(formProblems({ ...good, body: [] })).toContain("body cannot be empty");
    expect(formProblems(with_(el("markdown", { value: "x" })))).toContain("body must contain at least one non-markdown field");
    expect(formProblems(with_(el("input", { label: "A" }, { id: "x" }), el("input", { label: "B" }, { id: "x" })))).toContain("body must have unique ids");
    expect(formProblems(with_(el("input", { label: "A" }), el("textarea", { label: "A" })))).toContain("body must have unique labels");
    expect(formProblems(with_(el("input", { label: "A b" }), el("textarea", { label: "a-b" })))).toContain("labels are too similar");
    expect(formProblems(with_(el("input", { label: "A" }, { id: "a b" })))).toContain("body[0]: id can only contain numbers, letters, -, _");
    expect(formProblems(with_(el("input", { label: "A", options: ["x"] })))).toContain("body[0]: options is not a permitted attribute");
    expect(formProblems(with_(el("dropdown", { label: "A", options: ["x", "None"] })))).toContain("body[0]: options must not include the reserved word, none");
    expect(formProblems(with_(el("dropdown", { label: "A", options: ["x", true] })))).toContain("body[0]: options must not include booleans");
    expect(formProblems(with_(el("dropdown", { label: "A", options: ["x", "x"] })))).toContain("body[0]: options must be unique");
    expect(formProblems(with_(el("checkboxes", { label: "A", options: [{ required: true }] })))).toContain("body[0]: options[0] needs a label");
    expect(formProblems(with_(el("input", { label: "Your password" })))).toContain("body[0]: label contains forbidden word");
    expect(formProblems(with_(el("select", { label: "A" })))).toContain("body[0]: select is not a valid input type");
  });
});

describe("the issue forms in .github/ISSUE_TEMPLATE", () => {
  it("are bug.yml, feature.yml and install.yml, with config.yml", () => {
    expect(readdirSync(join(ROOT, TEMPLATES)).sort()).toEqual(["bug.yml", "config.yml", "feature.yml", "install.yml"]);
  });

  it.each(Object.keys(FORMS))("%s passes GitHub's issue forms check, with a unique name", (name) => {
    expect(formProblems(form(name))).toEqual([]);
    expect(new Set(Object.keys(FORMS).map((n) => form(n).name)).size).toBe(3);
  });

  it("the bug report: label bug; what happened, the steps, the version and the system required; the CLIs; logs as text; a required checkbox for private data", () => {
    const f = form("bug");
    expect([f.name, f.labels]).toEqual(["Bug report", ["bug"]]);
    expect(required(f)).toEqual(expect.arrayContaining(["what-happened", "steps", "version", "system"]));
    expect(field(f, "version").attributes.description).toMatch(/Settings → Updates.*`gizai --version`/);
    expect(field(f, "system").attributes.options).toEqual(["Linux", "macOS (Apple silicon)", "Windows 11"]);
    expect(field(f, "cli").attributes.options).toEqual(["Claude Code", "Codex CLI", "Gemini CLI", "Another CLI"]);
    expect(field(f, "logs").attributes.render).toBe("text");
    expect(field(f, "private").attributes.options).toEqual([{ label: "I removed tokens, keys, client names and private code from what I pasted", required: true }]);
  });

  it("the feature request: label enhancement; the problem required; who it helps; the eight features, install and other; the free app's line", () => {
    const f = form("feature");
    expect([f.name, f.labels]).toEqual(["Feature request", ["enhancement"]]);
    expect(required(f)).toEqual(["problem"]);
    expect(field(f, "who").attributes.options).toEqual(["A freelancer", "A team", "Both"]);
    const features = read("README.md").split("\n## Features\n")[1].split("\n## ")[0].match(/^- \*\*/gm);
    expect(field(f, "part").attributes.options).toHaveLength(features.length + 2);
    expect(field(f, "part").attributes.options.slice(-2)).toEqual(["Install and updates", "Other"]);
    expect(f.body[0].attributes.value).toMatch(/The app stays free; the paid plan is hosted services only/);
    expect(f.body[0].attributes.value).toMatch(/docs\/PAID-FEATURES\.md/);
  });

  it("the install or update problem: label bug; the system, how it was installed and the install check required; the update log, Rust, Node.js and WebKitGTK", () => {
    const f = form("install");
    expect([f.name, f.labels]).toEqual(["Install or update problem", ["bug"]]);
    expect(required(f)).toEqual(["system", "system-version", "how", "check"]);
    expect(field(f, "how").attributes.options).toHaveLength(4);
    expect(field(f, "how").attributes.options.join("\n")).toMatch(/one-line install[\s\S]*install\.sh[\s\S]*install\.ps1[\s\S]*update from inside Hersir/);
    expect(field(f, "check").attributes.description).toMatch(/`\.\/install\.sh --check`[\s\S]*`\.\\install\.ps1 -Check`/);
    expect(field(f, "output").attributes.description).toMatch(/update\/update\.log/);
    expect(field(f, "tools").attributes.description).toMatch(/`rustc --version` and `node --version`/);
    expect(field(f, "webkitgtk").attributes.label).toMatch(/WebKitGTK/);
  });

  it("the one-line install's check, as the install form gives it, is an option get.sh and get.ps1 take", () => {
    const check = field(form("install"), "check").attributes.description;
    expect(check).toContain("curl -fsSL https://hersir.dev/install.sh | sh -s -- --check");
    expect(read("scripts/get.sh")).toMatch(/--check\) check=1/);
    expect(check).toContain("& ([scriptblock]::Create((irm https://hersir.dev/install.ps1))) -Check");
    expect(read("scripts/get.ps1")).toMatch(/param\(\s*\[switch\]\$Check/);
  });

  it("config.yml turns blank issues off and sends questions to Q&A, security problems to the policy and readers to the docs", () => {
    const config = parseYaml(read(`${TEMPLATES}/config.yml`));
    expect(Object.keys(config).sort()).toEqual(["blank_issues_enabled", "contact_links"]);
    expect(config.blank_issues_enabled).toBe(false);
    for (const link of config.contact_links) {
      expect(Object.keys(link).sort()).toEqual(["about", "name", "url"]);
      expect(isText(link.name) && isText(link.about)).toBe(true);
      expect(link.url).toMatch(/^https:\/\//);
    }
    expect(config.contact_links.map((l) => l.url)).toEqual([`${REPO}/discussions/categories/q-a`, `${REPO}/security/policy`, "https://hersir.dev/docs"]);
  });
});

describe("the README's first screen (the research's section 3.3)", () => {
  const readme = read("README.md");
  const lines = readme.split("\n");
  const section = (title) => readme.split(`\n## ${title}\n`)[1].split("\n## ")[0];

  it("under the logo: the headline, who it's for, and the links with the Website first", () => {
    // GA-256: the paragraph under the headline leads with the Chief
    expect(lines.slice(8, 15)).toEqual([
      '<div align="center">',
      "",
      "**Your coding agents need a chief, not a babysitter.**",
      "",
      "Talk to one Chief instead of juggling a dozen agent chats. Tell it what you need, for any client or codebase: it turns that into cards on your board, starts the agents, answers their questions and puts only tested work in your review.",
      "Free and open source, on Claude Code, Codex CLI or Gemini CLI. Made for freelancers and software companies with several clients and projects.",
      "",
    ]);
    expect(lines[15]).toBe(`[Website](https://hersir.dev) · [Install](#install) · [Docs](#documentation) · [Releases](${REPO}/releases) · [Licence](LICENSE)`);
  });

  it("What it is, Made for work with several clients and The team come before Features, and Install after", () => {
    const headings = lines.filter((l) => l.startsWith("## ")).slice(0, 5);
    expect(headings).toEqual(["## What it is", "## Made for work with several clients", "## The team", "## Features", "## Install"]);
    expect(section("What it is")).toMatch(/^\nHersir is an Agentic Development Application \(ADA\)\. You write the cards;/);
    expect(section("Made for work with several clients").match(/^- \*\*[^*]+\*\*/gm)).toEqual([
      "- **Clients, projects and cards in one app.**", "- **Memory kept apart per client.**", "- **Several accounts per CLI.**",
    ]);
    const team = section("The team");
    expect(team).toMatch(/A new install starts with five agents\./);
    for (const agent of ["**Chief:**", "**Backend and Frontend Agents**", "**QA Agent**", "**DevOps Agent**", "**Design Agent**", "**R&D Agent**"]) expect(team).toContain(agent);
  });

  it("Features keeps its eight lines, each with its Read more, and the third names Codex CLI and Gemini CLI", () => {
    const features = section("Features").split("\n").filter((l) => l.startsWith("- **"));
    expect(features).toHaveLength(8);
    for (const f of features) expect(f).toMatch(/\[Read more\]\(docs\/features\/[a-z-]+\.md\)$/);
    expect(features[2]).toMatch(/^- \*\*Bring your own CLI\.\*\* .*like Claude Code, Codex CLI or Gemini CLI,/);
  });

  it("its last line says Hersir is independent of Anthropic, OpenAI and Google, and names their trademarks", () => {
    expect(readme.trimEnd().split("\n").at(-1)).toBe(
      "Hersir is an independent open-source project by YoTech, not affiliated with or endorsed by Anthropic, OpenAI or Google. " +
        "It runs Claude Code, Codex CLI and Gemini CLI with your own accounts, and never sells AI usage. " +
        "Claude Code is a trademark of Anthropic, Codex of OpenAI, and Gemini of Google LLC.",
    );
  });
});

describe("release.yml", () => {
  it("leaves the pull requests labelled release out of GitHub's list in a release's notes", () => {
    expect(parseYaml(read(".github/release.yml"))).toEqual({ changelog: { exclude: { labels: ["release"] } } });
  });
});

// GitHub's heading anchors: lower case, without punctuation, a hyphen for each space.
const slug = (heading) => heading.trim().toLowerCase().replace(/[^\p{L}\p{N}\s_-]/gu, "").replace(/\s/g, "-");
// a Markdown file without its code blocks, which keeps its line numbers
const withoutCode = (md) => md.replace(/^ *```[\s\S]*?^ *```/gm, (block) => block.replace(/[^\n]/g, ""));
const anchors = (path) => withoutCode(read(path)).split("\n").filter((l) => /^#{1,6} /.test(l)).map((l) => slug(l.replace(/^#+ /, "")));

// Every link in a file that points into this repository: a relative one, or one to its files on GitHub's main.
function repoLinks(path) {
  const links = [];
  for (const [, target] of withoutCode(read(path)).matchAll(/\]\(([^)\s]+)\)/g)) {
    const blob = target.match(/^https:\/\/github\.com\/Yotech-AI\/hersir\/blob\/main\/(.+)$/);
    if (blob) links.push({ target, file: blob[1].split("#")[0], anchor: blob[1].split("#")[1] });
    else if (!/^[a-z]+:/.test(target)) {
      const [file, anchor] = target.split("#");
      links.push({ target, file: file ? normalize(join(dirname(path), file)) : path, anchor });
    }
  }
  return links;
}

describe("the community files", () => {
  const FILES = ["CONTRIBUTING.md", "SECURITY.md", ".github/pull_request_template.md", ".github/release.yml"];

  it("CONTRIBUTING.md, SECURITY.md, the pull request template and release.yml exist and are in git", () => {
    for (const file of FILES) expect(existsSync(join(ROOT, file))).toBe(true);
    expect(git("ls-files", "--error-unmatch", ...FILES).status).toBe(0);
  });

  it.each(["README.md", "CONTRIBUTING.md", "SECURITY.md", ".github/pull_request_template.md", "docs/RELEASING.md", ...Object.values(FORMS).map((f) => `${TEMPLATES}/${f}`)])(
    "every link in %s to this repository reaches a file and its heading",
    (path) => {
      const broken = repoLinks(path).filter(({ file, anchor }) => !existsSync(join(ROOT, file)) || (anchor && !anchors(file).includes(anchor)));
      expect(broken.map((l) => l.target)).toEqual([]);
    },
  );

  it("SECURITY.md: the latest release only, reports through GitHub's private vulnerability reporting only, a first reply within 7 days", () => {
    const security = read("SECURITY.md");
    expect(security).toMatch(/Only the latest release gets security fixes/);
    expect(security).toMatch(/private vulnerability reporting/);
    expect(security).toContain(`${REPO}/security/advisories/new`);
    expect(security).toMatch(/the only way to report a security problem\. Never report one in a public issue/);
    expect(security).toMatch(/a first reply within 7 days/);
    expect(security).not.toMatch(/@[a-z0-9-]+\.[a-z]{2,}/i);
  });

  it("CONTRIBUTING.md: set up and test, never against your own Hersir, the code's rules, pull requests into main, Apache-2.0 without a CLA", () => {
    const contributing = read("CONTRIBUTING.md");
    const headings = anchors("CONTRIBUTING.md");
    expect(headings).toEqual(expect.arrayContaining(["set-up", "never-test-against-your-own-hersir", "pull-requests", "licence"]));
    expect(contributing).toMatch(/source scripts\/env\.sh[\s\S]*cargo test --workspace && npm test/);
    expect(contributing).toMatch(/scripts\/ui-test\.sh/);
    expect(contributing).toMatch(/HERSIR_DATA_DIR/); // GA-249: the variable's hersir name
    expect(contributing).toMatch(/Open them into `main`/);
    expect(contributing).toMatch(/Apache License 2\.0[\s\S]*no CLA/);
    for (const script of contributing.match(/scripts\/[a-z-]+\.sh/g)) expect(existsSync(join(ROOT, script))).toBe(true);
  });

  it("the pull request template asks what changed, how it was tested and on which systems", () => {
    expect(anchors(".github/pull_request_template.md")).toEqual(["what-changed", "how-it-was-tested", "systems-tried"]);
    expect(read(".github/pull_request_template.md")).toMatch(/- \[ \] Linux\n- \[ \] macOS \(Apple silicon\)\n- \[ \] Windows 11/);
  });
});

describe("docs/RELEASING.md", () => {
  const releasing = read("docs/RELEASING.md");
  const commands = (cmd) => releasing.split("\n").map((l) => l.trim()).filter((l) => l.startsWith(cmd));

  it("steps 1 and 2 open their pull requests with the label release.yml leaves out", () => {
    const label = parseYaml(read(".github/release.yml")).changelog.exclude.labels[0];
    const prs = commands("gh pr create");
    expect(prs).toHaveLength(2);
    expect(prs[0]).toMatch(/--base main --title "Bump version to X\.Y\.Z" .* --label release$/);
    expect(prs[1]).toMatch(/--base production --head main --title "Release vX\.Y\.Z" .* --label release$/);
    expect(label).toBe("release");
  });

  it("step 3 makes the release with the notes file above GitHub's list, posted in Announcements, never a draft or a pre-release", () => {
    expect(commands("gh release create")).toEqual([
      'gh release create vX.Y.Z --target production --title "vX.Y.Z" --notes-file <notes.md> --generate-notes --discussion-category "Announcements"',
    ]);
    const step3 = releasing.split("\n3. **Create the release**")[1].split("\n4. **")[0];
    expect(step3).toMatch(/only works once Jeffrey has turned Discussions on/);
    expect(step3).toMatch(/Don't add `--draft` or `--prerelease`/);
  });

  it("Release notes: the format, a sample with at most six bullets of what's new, and who writes them", () => {
    const notes = withoutCode(releasing).split("\n## Release notes\n")[1].split("\n## ")[0];
    expect(notes).toMatch(/\*\*A share line:\*\*[\s\S]*\*\*What's new:\*\* at most six bullets[\s\S]*\*\*Update:\*\*[\s\S]*\*\*All changes:\*\*/);
    const sample = releasing.match(/```markdown\n([\s\S]*?)```/)[1];
    expect(sample).toMatch(/^Hersir \d+\.\d+\.\d+ .+\n\n## What's new\n/);
    const bullets = sample.split("## What's new")[1].split("\n**Update:**")[0].match(/^- \*\*/gm);
    expect(bullets.length).toBeGreaterThan(0);
    expect(bullets.length).toBeLessThanOrEqual(6);
    expect(sample).toMatch(/\*\*Update:\*\* .+\n\n## All changes\n$/);
    expect(notes).toMatch(/\*\*Who writes them:\*\*[\s\S]*The Chief drafts[\s\S]*starts the DevOps Agent[\s\S]*The DevOps Agent[\s\S]*needs_decision/);
  });
});

// The docs that say Hersir (GA-234): the README, CLAUDE.md, NOTICE, the community files and the issue forms, every
// Markdown file in docs/, and design's READMEs and notes. Not the records of their day, which keep the name they had:
// the past release notes, the dated plans, and one morning's notes and open items.
const DATED = /^docs\/superpowers\/plans\/|^docs\/(MORNING-2026-10-07|OPEN-ITEMS|v0\.1-notes|v0\.3-notes)\.md$/;
const NAMED_DOCS = git("ls-files", "README.md", "CLAUDE.md", "NOTICE", "CONTRIBUTING.md", "SECURITY.md", ".github/ISSUE_TEMPLATE", ".github/pull_request_template.md", "docs/*.md", "design/*.md")
  .stdout.split("\n").filter((p) => p && !DATED.test(p));
// The one line for people who knew the old name, word for word.
const OLD_NAME = "Hersir was called GizAi until version 0.14.";
// The website's address before hersir.dev (GA-237), api.gizai.app too. ai.gizai.app is the app's identifier, not the
// address; Gizai.app was the macOS app. docs/research and docs/superpowers are dated records, left as written.
const OLD_ADDRESS = /(?<!\bai\.)\bgizai\.app\b/;
const RECORDS = /^docs\/(research|superpowers)\//;
// The one line that still gives the old address, word for word: the old install lines keep working.
const OLDER_LINES = "- Older install lines with gizai.app keep working.";
// The old name, spelled any way the docs had it: GizAi, Gizai, GizAI or Giz AI (Figma's file Giz-AI and giz.ai aren't it).
const OLD = /Giz ?[Aa][Ii]/;
// The old name the docs keep, in backticks or a code block, as the code has it (or Figma, for the app's variables, and
// Homebrew, whose class for gizai.rb is Gizai): the data folders on macOS and Windows, the install folder on Windows
// and the backups folder an older Hersir made, which is still found (GA-246; Hersir makes Hersir backups now).
const KEPT = [
  { name: /Application Support\/Gizai/, code: ["src-tauri/src/lib.rs", 'join("Library/Application Support/Gizai")'] },
  { name: /APPDATA%\\Gizai/, code: ["src-tauri/src/lib.rs", '.join("Gizai")'] },
  { name: /Programs\\Gizai/, code: ["install.ps1", "Join-Path $env:LOCALAPPDATA 'Programs\\Gizai'"] },
  { name: /Gizai backups/, code: ["crates/gizai-core/src/backups.rs", 'pub const OLD_FOLDER_NAME: &str = "Gizai backups";'] },
  { name: /GizAi (colour|size)/ },
  { name: /class Gizai < Formula/ },
];
const withoutKept = (text) => KEPT.reduce((rest, k) => rest.replace(new RegExp(k.name.source, "g"), ""), text);
// The code in a Markdown file, line by line: each line of its code blocks, and its `code` spans.
function codeParts(md) {
  const parts = [];
  let block = false;
  md.split("\n").forEach((text, i) => {
    if (/^ *```/.test(text)) block = !block;
    else if (block) parts.push({ line: i + 1, text });
    else for (const [span] of text.matchAll(/`[^`]*`/g)) parts.push({ line: i + 1, text: span });
  });
  return parts;
}
// The app's own texts with Hersir the docs quote, each in the app's code outside its tests (when the app changes one,
// the doc that quotes it shows up here).
const QUOTES = ["Open Hersir", "Quit Hersir completely", "From Hersir's catalog", "a tool from outside Hersir", "because Hersir quit", "Hersir can't find what", "Hersir pushed", "Hersir kept", "no Hersir data", "This Hersir runs from", "From Hersir card"];
// The names the docs give as the code has them (the card's item 4), and the forms they'd take if a doc renamed one by
// mistake. GA-248 made the programs, the command, the app id, the install folders and the variables of the scripts and
// the update Hersir's (hersir, hersir-mcp, hersir-launch, dev.hersir.app, lib/hersir, Programs\Hersir, hersir-src and
// HERSIR_PREFIX, HERSIR_REPO, HERSIR_BRANCH, HERSIR_RELEASES_URL, HERSIR_REF, HERSIR_MCP_BIN); GA-249 gave the data
// folders, the variables Hersir reads and sets, the keychain's service and the team template file their hersir names. An
// install from before 0.17.0 keeps its gizai names, which the docs still give, and the gizai ones still work. The rest
// stays gizai: GIZAI_RESULT, the MCP server gizai, gizai: links, .gizai-tmp, the files in the data folder and the backup
// format.
const CODE_NAMES = ["hersir --version", "hersir-mcp", "hersir-launch", "dev.hersir.app", "HERSIR_PREFIX", "hersir-installed.txt",
  "gizai --version", "gizai-mcp", "gizai-launch", "~/.local/share/gizai", "~/.local/share/hersir", "GIZAI_DATA_DIR", "HERSIR_DATA_DIR", "GIZAI_PREFIX", "GIZAI_RESULT", "ai.gizai.app", "mcp__gizai", ".gizai-tmp", "gizai-installed.txt", ".gizai-team.json", ".hersir-team.json", "gizai-backups.json"];
const RENAMED = /\bhersir(-backups|\.db|\.lock)\b|\bHERSIR_RESULT\b|ai\.hersir|mcp__hersir|\.hersir-tmp|\bhersir:[a-z]/;

describe("one name: Hersir in the docs, and Gizai only in the names the code keeps (GA-234)", () => {
  it("the docs it covers include the README, NOTICE, the community files, every doc in docs/ and design's READMEs, but not the records of their day", () => {
    for (const doc of ["README.md", "CLAUDE.md", "NOTICE", "CONTRIBUTING.md", "SECURITY.md", ".github/pull_request_template.md", ".github/ISSUE_TEMPLATE/bug.yml", ".github/ISSUE_TEMPLATE/config.yml",
      "docs/install.md", "docs/settings.md", "docs/memory.md", "docs/agent-tools.md", "docs/RELEASING.md", "docs/PLATFORMS.md", "docs/PAID-FEATURES.md", "docs/backup-format.md",
      "docs/team-template-format.md", "docs/HANDOFF-2026-10-07.md", "docs/features/board.md", "docs/research/packaging.md", "docs/superpowers/specs/2026-10-06-gizai-chat-design.md",
      "design/README.md", "design/SOURCES.md", "design/brand/README.md", "design/website/feature-page.md"])
      expect(NAMED_DOCS).toContain(doc);
    for (const doc of ["docs/v0.1-notes.md", "docs/v0.3-notes.md", "docs/superpowers/plans/2026-10-06-gizai-chat.md", "docs/MORNING-2026-10-07.md", "docs/OPEN-ITEMS.md"]) {
      expect(existsSync(join(ROOT, doc)), doc).toBe(true);
      expect(NAMED_DOCS).not.toContain(doc);
    }
  });

  it("no doc says GizAi or Gizai outside backticks and code blocks, but for the line for people who knew the old name", () => {
    const left = [];
    for (const doc of NAMED_DOCS) {
      withoutCode(read(doc)).split("\n").forEach((line, i) => {
        if (OLD.test(line.replace(/`[^`]*`/g, "").split(OLD_NAME).join(""))) left.push(`${doc}:${i + 1}: ${line.trim().slice(0, 120)}`);
      });
    }
    expect(left).toEqual([]);
  });

  it("in backticks and code blocks, the old name is only in the names the code keeps: the data, install and backups folders", () => {
    const left = [];
    for (const doc of NAMED_DOCS) for (const { line, text } of codeParts(read(doc))) if (OLD.test(withoutKept(text))) left.push(`${doc}:${line}: ${text.trim().slice(0, 120)}`);
    expect(left).toEqual([]);
  });

  it("each name kept with Gizai is still the code's, and still in a doc", () => {
    for (const { name, code } of KEPT) {
      expect(NAMED_DOCS.some((doc) => name.test(read(doc))), String(name)).toBe(true);
      if (code) expect(read(code[0]), String(name)).toContain(code[1]);
    }
  });

  it('README.md\'s "What it is" and the top of docs/install.md say "Hersir was called GizAi until version 0.14.", and no other doc does', () => {
    const whatItIs = read("README.md").split("\n## What it is\n")[1].split("\n## ")[0];
    // GA-248: the paragraph goes on with the programs' names from 0.17.0, on the next line (GA-256's chief-docs test
    // reads the old name's line as it was)
    expect(whatItIs.split("\n\n").map((p) => p.trim()).some((p) => p.startsWith(`${OLD_NAME}\n`) || p === OLD_NAME)).toBe(true);
    expect(read("docs/install.md").split("\n").slice(0, 3)).toEqual(["# Install Hersir", "", OLD_NAME]);
    expect(NAMED_DOCS.filter((doc) => read(doc).includes(OLD_NAME))).toEqual(["README.md", "docs/install.md"]);
  });

  it("each text with Hersir the docs quote from the app is the app's own", () => {
    for (const quote of QUOTES) {
      expect(NAMED_DOCS.some((doc) => read(doc).includes(quote)), quote).toBe(true);
      expect(git("grep", "-lF", quote, "--", "src", "src-tauri/src", "crates", ":!*.test.*", ":!**/tests/**").stdout, quote).not.toBe("");
    }
  });

  it("links go to github.com/Yotech-AI/hersir and hersir.dev, none to Yotech-AI/gizai, hersir.ai or gizai.app, and the install lines stay word for word", () => {
    const wrong = [];
    for (const doc of NAMED_DOCS) {
      read(doc).split("\n").forEach((line, i) => {
        const oldAddress = !RECORDS.test(doc) && OLD_ADDRESS.test(line) && line !== OLDER_LINES;
        if (oldAddress || /Yotech-AI\/gizai(?![\w-])|\b[Hh]ersir\.ai\b/.test(line)) wrong.push(`${doc}:${i + 1}: ${line.trim().slice(0, 120)}`);
      });
    }
    expect(wrong).toEqual([]);
    expect(NAMED_DOCS.filter((doc) => read(doc).includes(`${REPO}/`)).length).toBeGreaterThan(10);
    for (const doc of ["README.md", "docs/install.md"]) {
      expect(read(doc), doc).toContain("curl -fsSL https://hersir.dev/install.sh | sh");
      expect(read(doc), doc).toContain("irm https://hersir.dev/install.ps1 | iex");
    }
    expect(read("README.md")).toContain("[Website](https://hersir.dev)");
    expect(read("docs/PAID-FEATURES.md")).toContain("api.hersir.dev");
  });

  it("in the whole repository, the old address is only in the install doc's bullet and in the test that checks it", () => {
    const hits = git("grep", "-F", "gizai.app", "--", ".", ":!docs/research", ":!docs/superpowers").stdout.split(/\r?\n/);
    expect(hits.filter((hit) => OLD_ADDRESS.test(hit) && !hit.startsWith("scripts/community-files.test.mjs:"))).toEqual([`docs/install.md:${OLDER_LINES}`]);
  });

  it("the install doc says older lines keep working, right after the bullet on where the line goes", () => {
    const lines = read("docs/install.md").split("\n");
    const bullet = lines.indexOf(OLDER_LINES);
    expect(bullet).toBeGreaterThan(0);
    expect(lines[bullet - 1]).toMatch(/^- hersir\.dev sends the line on to /);
  });

  // GA-248: the app id is dev.hersir.app (ai.gizai.app before 0.17.0)
  it("the app's identifier is dev.hersir.app where the app and the installers use it", () => {
    expect(JSON.parse(read("src-tauri/tauri.conf.json")).identifier).toBe("dev.hersir.app");
    expect(read("src-tauri/src/lib.rs")).toContain('pub const APP_ID: &str = "dev.hersir.app";');
    expect(read("install.ps1")).toContain("'dev.hersir.app')");
    expect(read(".github/workflows/install.yml")).toContain('[ "$id" = dev.hersir.app ]');
    expect(read(".github/workflows/install.yml")).toContain("if ($id -ne 'dev.hersir.app')");
  });

  it("commands, paths, variables and file names are the code's: hersir's programs, install and HERSIR_ variables (GA-248), gizai's kept names, never a hersir in their place", () => {
    const docs = NAMED_DOCS.map(read).join("\n");
    for (const name of CODE_NAMES) {
      expect(docs, name).toContain(name);
      expect(git("grep", "-lF", name, "--", "src", "src-tauri", "crates", "install.sh", "install.ps1", "scripts", ":!*.test.*", ":!**/tests/**").stdout, name).not.toBe("");
    }
    const renamed = [];
    for (const doc of NAMED_DOCS) {
      read(doc).split("\n").forEach((line, i) => {
        if (RENAMED.test(line)) renamed.push(`${doc}:${i + 1}: ${line.trim().slice(0, 120)}`);
      });
    }
    expect(renamed).toEqual([]);
  });

  it.each(NAMED_DOCS.filter((doc) => doc.endsWith(".md") && !doc.startsWith("docs/research/")))("every relative link and #anchor in %s resolves", (doc) => {
    const broken = repoLinks(doc).filter(({ file, anchor }) => !existsSync(join(ROOT, file)) || (anchor && !anchors(file).includes(anchor)));
    expect(broken.map((l) => l.target)).toEqual([]);
  });

  // GA-233: the app's name for the OS is Hersir; the names an installed Gizai is found by stay, so it updates into it.
  // GA-248: the identifier, the program and the desktop entry are Hersir's too; the data folders and the keychain stay.
  it("the app is called Hersir (productName, the window title, the launcher), and so are the identifier, the program and the desktop entry, while the data folders stay", () => {
    const conf = JSON.parse(read("src-tauri/tauri.conf.json"));
    expect(conf.productName).toBe("Hersir");
    expect(conf.app.windows[0].title).toBe("Hersir");
    expect(conf.identifier).toBe("dev.hersir.app");
    expect(conf.mainBinaryName).toBe("hersir");
    expect(read("install.sh")).toContain(`grep -q '"productName": "Hersir"'`);
    expect(read("install.ps1")).toContain(`-Pattern '"productName": "Hersir"'`);
    expect(read("install.sh")).toMatch(/^Name=Hersir$/m);
    expect(read("install.sh")).toContain('cat > "$APPS/hersir.desktop" <<DESKTOP');
    expect(read("install.sh")).toMatch(/^Icon=hersir$/m);
    expect(read("install.sh")).toMatch(/^StartupWMClass=hersir$/m);
    expect(read("src-tauri/src/lib.rs")).toContain('join("Library/Application Support/Gizai")');
    expect(read("src-tauri/src/lib.rs")).toMatch(/\.join\("Gizai"\)/);
    // GA-249: the data folders and the keychain service are Hersir's for a new install; the gizai ones are still found
    expect(read("src-tauri/src/lib.rs")).toContain('join("Library/Application Support/Hersir")');
    expect(read("src-tauri/src/lib.rs")).toMatch(/\.join\("Hersir"\)/);
    expect(read("crates/gizai-agents/src/secrets.rs")).toContain('pub const SERVICE: &str = "hersir";');
    expect(read("crates/gizai-agents/src/secrets.rs")).toContain('pub const OLD_SERVICE: &str = "gizai";');
  });
});

// What install.sh, install.ps1 and the one-line scripts print: the header install.sh and install.ps1 print for --help
// and -Help (the lines their own help reads), and every line of code, which holds each say, step, Say and Step, the
// help get.sh and get.ps1 print and the notes they print later. Comments outside the header are never printed.
const INSTALLERS = ["install.sh", "install.ps1", "scripts/get.sh", "scripts/get.ps1"];
function printed(path) {
  const text = read(path);
  const [from, to] = path === "install.sh" ? text.match(/sed -n '(\d+),(\d+)p' "\$0"/).slice(1).map(Number)
    : path === "install.ps1" ? [1, Number(text.match(/-TotalCount (\d+)/)[1])] : [0, 0];
  return text.split("\n").map((line, i) => ({ n: i + 1, line })).filter(({ n, line }) => (n >= from && n <= to) || !/^\s*#/.test(line));
}
// The Gizai an installer keeps (the card's item 8): the data and install folders, the old install's Gizai.app and
// Gizai.lnk, and the code's own names for its functions and types, which print nothing.
const INSTALLER_KEPT = /Application Support\/Gizai|APPDATA%\\Gizai|APPDATA 'Gizai'|Programs\\Gizai|Gizai\.(app|lnk)|Invoke-Gizai|\$NewGizai|GizaiInstall/g;

describe("the installers' messages say Hersir (GA-234)", () => {
  it("what they print includes their help and their messages", () => {
    const lines = (path) => printed(path).map((p) => p.line.trim());
    expect(lines("install.sh")).toContain("# Hersir installer for Linux and macOS. Builds Hersir from source and installs it for you alone (no sudo):");
    expect(lines("install.sh")).toContain("#   ./install.sh --uninstall     remove Hersir, also an install from before 0.17.0 (add --purge to delete your data too)");
    expect(lines("install.sh")).toContain('[ "$ok" = 1 ] && say "Everything Hersir needs to build is here."');
    expect(lines("install.ps1")).toContain("# Hersir installer for Windows. Builds Hersir from source and installs it for you alone (no administrator rights):");
    expect(lines("install.ps1")).toContain("if ($ok) { Say 'Everything Hersir needs to build is here.' }");
    expect(lines("scripts/get.sh")).toContain("Hersir's one-line install for Linux and macOS:");
    expect(lines("scripts/get.ps1")).toContain('Say "Hersir\'s one-line install for Windows, in PowerShell:"');
    expect(lines("install.sh").some((l) => l.startsWith("# The tray icon loads libayatana-appindicator"))).toBe(false);
  });

  it.each(INSTALLERS)("nothing %s prints says Gizai, but for the data and install folders and the old install's Gizai.app and Gizai.lnk", (path) => {
    const left = printed(path).filter(({ line }) => OLD.test(line.replace(INSTALLER_KEPT, ""))).map(({ n, line }) => `${path}:${n}: ${line.trim().slice(0, 120)}`);
    expect(left).toEqual([]);
  });

  it("a message names Gizai.app or Gizai.lnk only for the old install's files", () => {
    const old = INSTALLERS.flatMap((path) => printed(path).filter(({ line }) => /Gizai\.(app|lnk)/.test(line) && /\b(say|Say)\b/.test(line)).map(({ line }) => line.trim()));
    expect(old).toEqual(['rm -rf "$LIB/Gizai.app" || say "Note: could not remove the old $LIB/Gizai.app (Hersir.app replaces it). Remove it yourself."']);
  });

  it('install.ps1 quotes the tray in its own words: "(Quit Hersir completely, in its tray menu)"', () => {
    const quit = read("src-tauri/src/lib.rs").match(/MenuItem::with_id\(app, "quit", "([^"]+)"/)[1];
    expect(quit).toBe("Quit Hersir completely");
    const quotes = read("install.ps1").split("\n").filter((l) => l.includes("in its tray menu"));
    expect(quotes).toHaveLength(2);
    for (const l of quotes) expect(l).toContain(`(${quit}, in its tray menu)`);
  });

  it("Cargo.toml's descriptions and browser.html's title say Hersir", () => {
    const tomls = git("ls-files", "Cargo.toml", "*/Cargo.toml").stdout.split("\n").filter(Boolean);
    const descriptions = tomls.flatMap((t) => [...read(t).matchAll(/^description = "(.*)"$/gm)].map((m) => m[1]));
    expect(descriptions).toHaveLength(4);
    for (const d of descriptions) expect(d).toMatch(/^Hersir[: ]/);
    expect(descriptions.filter((d) => OLD.test(d))).toEqual([]);
    expect(read("browser.html")).toContain("<title>Hersir (browser mode)</title>");
    expect(read("browser.html")).not.toMatch(OLD);
  });
});
