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

test("a changed outcome is not misreported as a performance improvement", () => {
	const comparison = compareRuntimeReports(
		policy,
		{
			cases: [
				{ id: "example/instruction", computeUnits: 1_000, succeeded: true },
			],
		},
		{
			cases: [
				{ id: "example/instruction", computeUnits: 500, succeeded: false },
			],
		},
	).comparisons[0];
	assert.equal(comparison?.deltaCu, 500);
	assert.equal(comparison?.status, "behavior-changed");
	assert.equal(comparison?.baseSucceeded, true);
	assert.equal(comparison?.headSucceeded, false);
});

test("an unexpected head outcome fails the runtime policy", () => {
	const expectedRejectionPolicy = {
		...policy,
		runtimeExpectedOutcomes: { "example/instruction": false },
	};
	const unexpectedSuccess = compareRuntimeReports(
		expectedRejectionPolicy,
		{
			cases: [
				{ id: "example/instruction", computeUnits: 1_000, succeeded: false },
			],
		},
		{
			cases: [
				{ id: "example/instruction", computeUnits: 500, succeeded: true },
			],
		},
	);
	assert.equal(unexpectedSuccess.hardErrors.length, 1);
	assert.match(unexpectedSuccess.hardErrors[0] ?? "", /expected.*reject/);

	const missingOutcome = compareRuntimeReports(
		expectedRejectionPolicy,
		{ cases: [{ id: "example/instruction", computeUnits: 1_000 }] },
		{ cases: [{ id: "example/instruction", computeUnits: 500 }] },
	);
	assert.equal(missingOutcome.hardErrors.length, 1);
	assert.match(missingOutcome.hardErrors[0] ?? "", /missing.*outcome/);
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
});
