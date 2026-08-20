import assert from "node:assert/strict";
import { existsSync, mkdtempSync, readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const testDirectory = dirname(fileURLToPath(import.meta.url));
const repositoryRoot = resolve(testDirectory, "../../..");
const require = createRequire(import.meta.url);
const installer = require(
	join(repositoryRoot, "packages/pina__skill/bin/pina-skill.cjs"),
);

test("default destination respects CODEX_HOME", () => {
	assert.equal(
		installer.defaultDestination({ CODEX_HOME: "/tmp/codex-home" }),
		join("/tmp/codex-home", "skills", "pina"),
	);
});

test("installer copies the runtime skill and refuses replacement", () => {
	const sandbox = mkdtempSync(join(tmpdir(), "pina-skill-"));
	const destination = join(sandbox, "pina");
	assert.equal(installer.installSkill(destination), destination);
	assert.ok(existsSync(join(destination, "SKILL.md")));
	assert.ok(existsSync(join(destination, "agents/openai.yaml")));
	assert.ok(existsSync(join(destination, "references/program-authoring.md")));
	assert.match(
		readFileSync(join(destination, "SKILL.md"), "utf8"),
		/^---\nname: pina\n/,
	);
	assert.throws(
		() => installer.installSkill(destination),
		/Refusing to replace existing skill directory/,
	);
});

test("help describes every supported operation", () => {
	const help = installer.helpText();
	assert.match(help, /--install \[DIR\]/);
	assert.match(help, /--print-path/);
	assert.match(help, /never overwrites/);
});

test("installer rejects ambiguous destination arguments", () => {
	const originalConsoleError = console.error;
	console.error = () => {};
	try {
		assert.equal(installer.main(["--install", "--unexpected"]), 2);
		assert.equal(installer.main(["--install", "/tmp/pina", "extra"]), 2);
	} finally {
		console.error = originalConsoleError;
	}
});
