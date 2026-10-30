import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import {
	existsSync,
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

test("core reports revision changes without execution-position bias", (context) => {
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
  const program = [
    "#!/usr/bin/env node",
    'const { appendFileSync, existsSync, readFileSync } = require("node:fs");',
    "const orderFile = " + JSON.stringify(join(target, "..", "sample-order")) + ";",
    'const position = existsSync(orderFile) ? readFileSync(orderFile, "utf8").trim().split("\\\\n").length % 2 : 0;',
    "appendFileSync(orderFile, " + JSON.stringify(label + "\\n") + ");",
    "console.log(" + JSON.stringify("test benchmark ... [BENCH] " + label + ": total=100µs, avg=100ns (1000 iterations)") + ' + (process.env.PINA_TEST_BENCHMARK_SUFFIX ?? ""));',
    "console.log(" + JSON.stringify("[BENCH] shared: total=" + (label === "base-only" ? "1.5µs" : "1.6µs") + ", avg=1ns (1000 iterations)") + ");",
    'console.log("[BENCH] position-sensitive: total=" + (position === 0 ? "100µs, avg=100ns" : "90µs, avg=90ns") + " (1000 iterations)");',
    "",
  ].join("\\n");
  writeFileSync(executable, program, { mode: 0o755 });
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
	assert.match(
		report,
		/\| `shared` \| 1\.5 ns \| 1\.6 ns \| -6\.7% \| ⚠️ regressed \|/u,
	);
	assert.match(
		report,
		/\| `position-sensitive` \| 95 ns \| 95 ns \| \+0\.0% \| ➖ within noise \|/u,
	);
	const samples: unknown = JSON.parse(
		readFileSync(join(output, "core.json"), "utf8"),
	);
	assert.deepEqual(samples, {
		sampleRuns: 8,
		executionOrder: [
			"base-first",
			"head-first",
			"base-first",
			"head-first",
			"base-first",
			"head-first",
			"base-first",
			"head-first",
		],
		comparisons: [
			{
				id: "base-only",
				baseNanoseconds: 100,
				baseSamples: Array(8).fill(100),
			},
			{
				id: "head-only",
				headNanoseconds: 100,
				headSamples: Array(8).fill(100),
			},
			{
				id: "position-sensitive",
				baseNanoseconds: 95,
				headNanoseconds: 95,
				baseSamples: [100, 90, 100, 90, 100, 90, 100, 90],
				headSamples: [90, 100, 90, 100, 90, 100, 90, 100],
				performancePercent: 0,
			},
			{
				id: "shared",
				baseNanoseconds: 1.5,
				headNanoseconds: 1.6,
				baseSamples: Array(8).fill(1.5),
				headSamples: Array(8).fill(1.6),
				performancePercent: (1.5 - 1.6) / 1.5 * 100,
			},
		],
	});
	assert.deepEqual(
		readFileSync(join(root, "target", "sample-order"), "utf8").trim().split(
			"\n",
		),
		[
			"base-only",
			"head-only",
			"head-only",
			"base-only",
			"base-only",
			"head-only",
			"head-only",
			"base-only",
			"base-only",
			"head-only",
			"head-only",
			"base-only",
			"base-only",
			"head-only",
			"head-only",
			"base-only",
		],
	);
	for (const [index, suffix] of [" trailing text", " (malformed)"].entries()) {
		const invalidOutput = join(root, `invalid-report-${index}`);
		assert.throws(
			() =>
				execFileSync(process.execPath, [
					fileURLToPath(new URL("../benchmark-core.ts", import.meta.url)),
					base,
					head,
					invalidOutput,
					join(root, `invalid-target-${index}`),
				], {
					env: {
						...process.env,
						PATH: `${bin}${delimiter}${process.env.PATH ?? ""}`,
						PINA_TEST_BENCHMARK_SUFFIX: suffix,
					},
					encoding: "utf8",
					stdio: "pipe",
				}),
			/invalid benchmark output/u,
		);
		assert.equal(existsSync(join(invalidOutput, "core.json")), false);
	}
});
