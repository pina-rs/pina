import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import {
	compareRuntimeReports,
	type ComputeUnitPolicy,
	performancePercent,
	run,
} from "../compare-compute-units.ts";

const policy: ComputeUnitPolicy = {
	trackedPrograms: [],
	runtimeCases: ["example/instruction"],
	warn: { deltaCu: 250, deltaPercent: 5 },
	fail: { deltaCu: 500, deltaPercent: 10 },
	runtimeApprovedTotals: {},
};

test("runtime savings are positive and regressions are negative", () => {
	const savings = compareRuntimeReports(
		policy,
		{ cases: [{ id: "example/instruction", computeUnits: 1_000 }] },
		{ cases: [{ id: "example/instruction", computeUnits: 900 }] },
	).comparisons[0];
	assert.equal(savings?.deltaCu, 100);
	assert.equal(savings?.deltaPercent, 10);
	assert.equal(savings?.status, "improved");

	const regression = compareRuntimeReports(
		policy,
		{ cases: [{ id: "example/instruction", computeUnits: 1_000 }] },
		{ cases: [{ id: "example/instruction", computeUnits: 1_001 }] },
	).comparisons[0];
	assert.equal(regression?.deltaCu, -1);
	assert.equal(regression?.deltaPercent, -0.1);
	assert.equal(regression?.status, "fail");
});

test("an approved runtime regression retains its negative score", () => {
	const approvedPolicy = {
		...policy,
		runtimeApprovedTotals: { "example/instruction": 1_100 },
	};
	const comparison = compareRuntimeReports(
		approvedPolicy,
		{ cases: [{ id: "example/instruction", computeUnits: 1_000 }] },
		{ cases: [{ id: "example/instruction", computeUnits: 1_050 }] },
	).comparisons[0];
	assert.equal(comparison?.deltaCu, -50);
	assert.equal(comparison?.status, "approved-regression");
});

test("missing head cases fail while missing base cases establish a baseline", () => {
	const missingHead = compareRuntimeReports(
		policy,
		{ cases: [{ id: "example/instruction", computeUnits: 1_000 }] },
		{ cases: [] },
	);
	assert.equal(missingHead.hardErrors.length, 1);
	assert.equal(missingHead.newBaselines.length, 0);

	const missingBase = compareRuntimeReports(
		policy,
		{ cases: [] },
		{ cases: [{ id: "example/instruction", computeUnits: 1_000 }] },
	);
	assert.equal(missingBase.hardErrors.length, 0);
	assert.equal(missingBase.newBaselines.length, 1);
});

test("base-only runtime cases are removed unless policy still requires them", () => {
	const optionalCasePolicy = { ...policy, runtimeCases: [] };
	const removed = compareRuntimeReports(
		optionalCasePolicy,
		{ cases: [{ id: "example/removed", computeUnits: 1_000 }] },
		{ cases: [] },
	);
	assert.deepEqual(removed.removedCases, ["example/removed"]);
	assert.equal(removed.hardErrors.length, 0);

	const required = compareRuntimeReports(
		{ ...policy, runtimeCases: ["example/removed"] },
		{ cases: [{ id: "example/removed", computeUnits: 1_000 }] },
		{ cases: [] },
	);
	assert.equal(required.removedCases.length, 0);
	assert.equal(required.hardErrors.length, 1);
});

test("static reports use the same performance-score direction", () => {
	const root = mkdtempSync(join(tmpdir(), "pina-cu-comparison-"));
	const base = join(root, "base");
	const head = join(root, "head");
	mkdirSync(base);
	mkdirSync(head);
	const staticPolicy: ComputeUnitPolicy = {
		trackedPrograms: ["faster", "slower"],
		warn: { deltaCu: 250, deltaPercent: 5 },
		fail: { deltaCu: 500, deltaPercent: 10 },
	};
	const profile = (totalCu: number) => ({
		total_cu: totalCu,
		binary_size: 10,
		text_size: 5,
		total_syscalls: 1,
	});
	writeFileSync(join(root, "policy.json"), JSON.stringify(staticPolicy));
	const manifest = {
		results: {
			faster: { status: "ok" },
			slower: { status: "ok" },
		},
	};
	writeFileSync(join(base, "manifest.json"), JSON.stringify(manifest));
	writeFileSync(join(head, "manifest.json"), JSON.stringify(manifest));
	writeFileSync(join(base, "faster.json"), JSON.stringify(profile(1_000)));
	writeFileSync(join(head, "faster.json"), JSON.stringify(profile(900)));
	writeFileSync(join(base, "slower.json"), JSON.stringify(profile(1_000)));
	writeFileSync(join(head, "slower.json"), JSON.stringify(profile(1_100)));

	const status = run({
		policyFile: join(root, "policy.json"),
		baseDir: base,
		headDir: head,
		markdownOutput: join(root, "comparison.md"),
		jsonOutput: join(root, "comparison.json"),
	});
	assert.equal(status, 0);
	const report = JSON.parse(
		readFileSync(join(root, "comparison.json"), "utf8"),
	) as {
		programs: Array<{ program: string; deltaCu: number; status: string }>;
	};
	assert.deepEqual(
		report.programs.map(({ program, deltaCu, status: comparisonStatus }) => ({
			program,
			deltaCu,
			status: comparisonStatus,
		})),
		[
			{ program: "faster", deltaCu: 100, status: "improved" },
			{ program: "slower", deltaCu: -100, status: "small-regression" },
		],
	);
	assert.equal(performancePercent(1_000, 900), 10);
	const markdown = readFileSync(join(root, "comparison.md"), "utf8");
	assert.match(markdown, /⚠️ 1 advisory regression/u);
	assert.match(markdown, /🚀 1 improvement/u);
	assert.match(markdown, /<details>/u);
	assert.match(
		markdown,
		/<summary>View program benchmark details<\/summary>/u,
	);
	assert.ok(
		markdown.indexOf("<details>") < markdown.indexOf("| Program |"),
		"program table should follow the details opener",
	);
	assert.ok(
		markdown.indexOf("| Program |") < markdown.lastIndexOf("</details>"),
		"program table should precede the details closer",
	);
});

test("runtime report keeps its summary visible and table collapsed", () => {
	const root = mkdtempSync(join(tmpdir(), "pina-runtime-summary-"));
	const base = join(root, "base");
	const head = join(root, "head");
	const baseRuntime = join(root, "runtime-base.json");
	const headRuntime = join(root, "runtime-head.json");
	mkdirSync(base);
	mkdirSync(head);
	writeFileSync(join(base, "manifest.json"), JSON.stringify({ results: {} }));
	writeFileSync(join(head, "manifest.json"), JSON.stringify({ results: {} }));
	writeFileSync(join(root, "policy.json"), JSON.stringify(policy));
	writeFileSync(
		baseRuntime,
		JSON.stringify({
			cases: [{ id: "example/instruction", computeUnits: 1_000 }],
		}),
	);
	writeFileSync(
		headRuntime,
		JSON.stringify({
			cases: [{ id: "example/instruction", computeUnits: 900 }],
		}),
	);

	const status = run({
		policyFile: join(root, "policy.json"),
		baseDir: base,
		headDir: head,
		baseRuntime,
		headRuntime,
		runtimeOnly: true,
		markdownOutput: join(root, "comparison.md"),
		jsonOutput: join(root, "comparison.json"),
	});
	assert.equal(status, 0);
	const markdown = readFileSync(join(root, "comparison.md"), "utf8");
	assert.match(markdown, /✅ No regressions or measurement errors/u);
	assert.match(markdown, /🚀 1 improvement/u);
	assert.match(
		markdown,
		/<summary>View instruction benchmark details<\/summary>/u,
	);
	assert.ok(
		markdown.indexOf("<details>") < markdown.indexOf("| Instruction case |"),
		"instruction table should follow the details opener",
	);
	assert.ok(
		markdown.indexOf("| Instruction case |") < markdown.indexOf("</details>"),
		"instruction table should precede the details closer",
	);
});

test("a new program reports its current compute units and build size", () => {
	const root = mkdtempSync(join(tmpdir(), "pina-cu-baseline-"));
	const base = join(root, "base");
	const head = join(root, "head");
	mkdirSync(base);
	mkdirSync(head);
	writeFileSync(
		join(root, "policy.json"),
		JSON.stringify({
			warn: { deltaCu: 250, deltaPercent: 5 },
			fail: { deltaCu: 500, deltaPercent: 10 },
		}),
	);
	writeFileSync(join(base, "manifest.json"), JSON.stringify({ results: {} }));
	writeFileSync(
		join(head, "manifest.json"),
		JSON.stringify({
			results: { new_example: { status: "ok" } },
		}),
	);
	writeFileSync(
		join(head, "new_example.json"),
		JSON.stringify({
			total_cu: 1_234,
			binary_size: 56_789,
			text_size: 5,
			total_syscalls: 1,
		}),
	);

	const status = run({
		policyFile: join(root, "policy.json"),
		baseDir: base,
		headDir: head,
		markdownOutput: join(root, "comparison.md"),
		jsonOutput: join(root, "comparison.json"),
	});
	assert.equal(status, 0);
	const markdown = readFileSync(join(root, "comparison.md"), "utf8");
	assert.match(markdown, /new_example/u);
	assert.match(markdown, /56,789 B/u);
});
