import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import {
	existsSync,
	mkdtempSync,
	readFileSync,
	rmSync,
	writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

test("CLI comparisons balance execution positions and retain successful samples", (context) => {
	const root = mkdtempSync(join(tmpdir(), "pina-cli-benchmark-"));
	context.after(() => rmSync(root, { recursive: true, force: true }));
	const binary = join(root, "pina");
	writeFileSync(binary, "#!/usr/bin/env node\nprocess.exit(0);\n", {
		mode: 0o755,
	});
	const hyperfine = join(root, "hyperfine");
	writeFileSync(
		hyperfine,
		`#!/usr/bin/env node
const { writeFileSync } = require("node:fs");
const args = process.argv.slice(2);
const output = args[args.indexOf("--export-json") + 1];
const runs = Number(args[args.indexOf("--runs") + 1]);
const results = [];
for (let index = 0; index < args.length; index++) {
  if (args[index] !== "--command-name") continue;
  const command = args[index + 1];
  // Identical work takes 10% longer in each pair's second position.
  const seconds = results.length % 2 === 0 ? 0.1 : 0.11;
  results.push({
    command, mean: seconds, stddev: runs > 1 ? 0 : null,
    median: seconds, min: seconds, max: seconds,
    user: seconds / 2, system: seconds / 2,
    times: Array(runs).fill(seconds), exit_codes: Array(runs).fill(0),
  });
  index += 2;
}
if (process.env.PINA_FAKE_SAMPLE === "failed") results[0].exit_codes = [9];
if (process.env.PINA_FAKE_SAMPLE === "missing") results.pop();
if (process.env.PINA_FAKE_SAMPLE === "invalid") results[0].times = [-1];
writeFileSync(output, JSON.stringify({ results }));
`,
		{ mode: 0o755 },
	);
	const markdown = join(root, "report.md");
	const json = join(root, "report.json");
	const args = [
		fileURLToPath(new URL("../benchmark-cli.ts", import.meta.url)),
		"--base-bin",
		binary,
		"--head-bin",
		binary,
		"--markdown-output",
		markdown,
		"--json-output",
		json,
	];
	execFileSync(process.execPath, args, {
		cwd: root,
		env: { ...process.env, PINA_HYPERFINE_BIN: hyperfine },
		encoding: "utf8",
	});
	assert.match(
		readFileSync(markdown, "utf8"),
		/\| `pina --version` \| 105\.00 ms \| 105\.00 ms \| \+0\.0%/u,
	);
	const report: unknown = JSON.parse(readFileSync(json, "utf8"));
	assert.ok(report !== null && typeof report === "object");
	assert.ok("sampleRuns" in report);
	assert.equal(report.sampleRuns, 8);
	assert.ok("executionOrder" in report);
	assert.deepEqual(report.executionOrder, [
		"base-first",
		"head-first",
		"base-first",
		"head-first",
		"base-first",
		"head-first",
		"base-first",
		"head-first",
	]);
	assert.ok("results" in report && Array.isArray(report.results));
	assert.equal(report.results.length, 16);
	const results: unknown[] = report.results;
	for (const result of results) {
		assert.ok(result !== null && typeof result === "object");
		assert.ok("mean" in result && typeof result.mean === "number");
		assert.ok(Math.abs(result.mean - 0.105) < 1e-12);
		assert.ok("times" in result && Array.isArray(result.times));
		assert.equal(result.times.length, 8);
		assert.ok("exit_codes" in result);
		assert.deepEqual(result.exit_codes, Array(8).fill(0));
	}
	assert.match(readFileSync(markdown, "utf8"), /No advisory regressions/u);
	for (const sample of ["failed", "missing", "invalid"]) {
		writeFileSync(markdown, "stale successful report");
		writeFileSync(json, "stale successful data");
		const failure = spawnSync(process.execPath, args, {
			cwd: root,
			env: {
				...process.env,
				PINA_HYPERFINE_BIN: hyperfine,
				PINA_FAKE_SAMPLE: sample,
			},
			encoding: "utf8",
		});
		assert.equal(failure.status, 1, failure.stderr);
		assert.match(failure.stderr, /hyperfine returned|hyperfine omitted/u);
		assert.equal(existsSync(markdown), false);
		assert.equal(existsSync(json), false);
	}
});
