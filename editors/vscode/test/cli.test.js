// Unit tests for the command wiring and the shipped assets. Run with `npm test`.
const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const cli = require("../out/cli.js");

const root = path.join(__dirname, "..");
const pkg = JSON.parse(fs.readFileSync(path.join(root, "package.json"), "utf8"));
const read = (rel) => JSON.parse(fs.readFileSync(path.join(root, rel), "utf8"));

test("every registered command is contributed in package.json, and vice versa", () => {
  const contributed = pkg.contributes.commands.map((c) => c.command).sort();
  assert.deepStrictEqual([...cli.COMMAND_IDS].sort(), contributed);
});

test("commands run the planned CLI names", () => {
  const args = Object.fromEntries(cli.RUN_COMMANDS.map((c) => [c.id, c.args.join(" ")]));
  assert.strictEqual(args["veld.validate"], "check");
  assert.strictEqual(args["veld.generate"], "generate");
  assert.strictEqual(args["veld.generateDryRun"], "generate --dry-run");
  assert.strictEqual(args["veld.generateCheck"], "generate --check");
  assert.strictEqual(args["veld.setup"], "setup");
  assert.strictEqual(args["veld.doctor"], "doctor");
  assert.strictEqual(cli.WATCH.args.join(" "), "generate --watch");
  assert.deepStrictEqual(cli.exportArgs("openapi", "out.json"), ["export", "openapi", "-o", "out.json"]);
});

test("the veld.path setting is honoured, blank means PATH", () => {
  assert.strictEqual(cli.executable(undefined), "veld");
  assert.strictEqual(cli.executable("  "), "veld");
  assert.strictEqual(cli.executable("/opt/veld"), "/opt/veld");
});

test("the check summary line is parsed", () => {
  assert.deepStrictEqual(cli.parseSummary("✗ 1 error, 2 warnings"), { errors: 1, warnings: 2 });
  assert.strictEqual(cli.parseSummary("nothing"), undefined);
});

test("the problem matcher matches a real diagnostic", () => {
  const [first, second] = pkg.contributes.problemMatchers[0].pattern.map((p) => new RegExp(p.regexp));
  const head = first.exec("error[E0205]: unknown type `Strng`");
  assert.ok(head && head[1] === "error" && head[2] === "E0205");
  const loc = second.exec(" --> x/a.veld:2:6");
  assert.ok(loc && loc[1] === "x/a.veld" && loc[2] === "2" && loc[3] === "6");
});

test("snippets cover every v1 purpose in v2 syntax", () => {
  const prefixes = new Set(Object.values(read("snippets/veld.json")).map((s) => s.prefix));
  for (const needed of [
    "model", "enum", "error", "union", "service", "get", "post", "put", "patch", "delete",
    "crud", "sse", "ws", "field?", "field[]", "fieldmap", "fielddef", "use", "usepkg", "const",
    "@unique", "@index", "@relation", "@example", "@deprecated", "@auth",
  ]) {
    assert.ok(prefixes.has(needed), `missing snippet prefix ${needed}`);
  }
  for (const s of Object.values(read("snippets/veld.json"))) {
    for (const line of s.body) {
      assert.ok(!/^\s*(module|action)\s/.test(line), `v1 keyword in snippet line: ${line}`);
    }
  }
});

test("veld.toml snippets cover single service, workspace, targets, plugin and registry", () => {
  const prefixes = new Set(Object.values(read("snippets/veld-toml.json")).map((s) => s.prefix));
  for (const needed of ["veld-single", "veld-workspace", "veld-node", "veld-java", "veld-typescript", "veld-react", "veld-plugin", "veld-dependency"]) {
    assert.ok(prefixes.has(needed), `missing ${needed}`);
  }
});

test("the veld.toml schema is contributed and exists", () => {
  const entry = pkg.contributes.tomlValidation[0];
  assert.strictEqual(entry.fileMatch, "veld.toml");
  const schema = read(entry.url);
  assert.strictEqual(schema.title, "veld.toml");
  assert.deepStrictEqual(schema.properties.targets.items.properties.role.enum, ["client", "server", "spec"]);
});

test("snippet files are contributed", () => {
  const paths = pkg.contributes.snippets.map((s) => s.path);
  for (const p of paths) {
    assert.ok(fs.existsSync(path.join(root, p)), p);
  }
});
