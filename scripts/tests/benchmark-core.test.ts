import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import {
	mkdirSync,
	mkdtempSync,
	readFileSync,
	rmSync,
	writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { delimiter, join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

test("core reports benchmarks added and removed between revisions", (context) => {
	const root = mkdtempSync(join(tmpdir(), "pina-core-benchmark-"));
	context.after(() => rmSync(root, { recursive: true, force: true }));
	const base = join(root, "base");
	const head = join(root, "head");
	const bin = join(root, "bin");
	for (const directory of [base, head, bin]) {
		mkdirSync(directory);
	}
	writeFileSync(join(base, "benchmark-label"), "base-only");
	writeFileSync(join(head, "benchmark-label"), "head-only");

	// A fresh Cargo artifact may survive a workspace switch in a shared target.
	writeFileSync(
		join(bin, "cargo"),
		`#!/usr/bin/env node
const { existsSync, mkdirSync, readFileSync, writeFileSync } = require("node:fs");
const { join } = require("node:path");
const target = process.env.CARGO_TARGET_DIR;
if (target === undefined) throw new Error("Missing Cargo target");
mkdirSync(target, { recursive: true });
const executable = join(target, "benchmarks");
if (!existsSync(executable)) {
  const label = readFileSync(join(process.cwd(), "benchmark-label"), "utf8");
  writeFileSync(executable,
    "#!/usr/bin/env node\\nconsole.log(" + JSON.stringify("[BENCH] " + label + ": avg=100ns") + ");\\n",
    { mode: 0o755 });
}
console.log(JSON.stringify({ reason: "compiler-artifact", target: { name: "benchmarks" }, executable }));
`,
		{ mode: 0o755 },
	);

	const output = join(root, "report");
	execFileSync(process.execPath, [
		fileURLToPath(new URL("../benchmark-core.ts", import.meta.url)),
		base,
		head,
		output,
		join(root, "target"),
	], {
		env: {
			...process.env,
			PATH: `${bin}${delimiter}${process.env.PATH ?? ""}`,
		},
		encoding: "utf8",
	});

	const report = readFileSync(join(output, "core.md"), "utf8");
	assert.match(
		report,
		/\| `base-only` \| 100 ns \| n\/a ns \| n\/a \| removed \|/u,
	);
	assert.match(
		report,
		/\| `head-only` \| n\/a ns \| 100 ns \| n\/a \| 🆕 new baseline \|/u,
	);
});
