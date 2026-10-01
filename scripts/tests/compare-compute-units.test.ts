import assert from "node:assert/strict";
import {
	mkdirSync,
	mkdtempSync,
	readFileSync,
	unlinkSync,
	writeFileSync,
} from "node:fs";
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
	runtimeCuApprovals: {},
};

const baseRevision = "a".repeat(40);

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
		runtimeCuApprovals: {
			"example/instruction": {
				baseRevision,
				base: 1_000,
				head: 1_100,
				reason: "Reviewed redesign",
			},
		},
	};
	const comparison = compareRuntimeReports(
		approvedPolicy,
		{ cases: [{ id: "example/instruction", computeUnits: 1_000 }] },
		{ cases: [{ id: "example/instruction", computeUnits: 1_050 }] },
		baseRevision,
	).comparisons[0];
	assert.equal(comparison?.deltaCu, -50);
	assert.equal(comparison?.status, "approved-regression");
});

test("runtime approvals expire after merging and cannot undo later savings", () => {
	const approvedPolicy: ComputeUnitPolicy = {
		...policy,
		runtimeCuApprovals: {
			"example/instruction": {
				baseRevision,
				base: 1_000,
				head: 1_100,
				reason: "Reviewed redesign",
			},
		},
	};

	for (
		const [base, head, revision] of [
			[1_000, 1_101, baseRevision], // Above the reviewed head.
			[900, 1_050, baseRevision], // Savings changed the measured base.
			[1_000, 1_050, "b".repeat(40)], // A later PR has the same numbers.
			[1_000, 1_050, undefined], // A caller must provide its PR base.
		] as const
	) {
		const result = compareRuntimeReports(
			approvedPolicy,
			{ cases: [{ id: "example/instruction", computeUnits: base }] },
			{ cases: [{ id: "example/instruction", computeUnits: head }] },
			revision,
		);
		assert.equal(result.comparisons[0]?.status, "fail");
	}
});

test("an approval cannot use a moving branch name instead of a commit ID", () => {
	const result = compareRuntimeReports(
		{
			...policy,
			runtimeCuApprovals: {
				"example/instruction": {
					baseRevision: "main",
					base: 1_000,
					head: 1_100,
					reason: "Invalid scope",
				},
			},
		},
		{ cases: [{ id: "example/instruction", computeUnits: 1_000 }] },
		{ cases: [{ id: "example/instruction", computeUnits: 1_050 }] },
		"main",
	);
	assert.equal(result.comparisons[0]?.status, "fail");
});

/** Exercise the actual report writer and CI exit status with synthetic ELFs. */
function staticReport(
	baseCu: number,
	headCu: number,
	baseSize: number,
	headSize: number,
	approvals: Partial<ComputeUnitPolicy> = {},
	revision: string | undefined = baseRevision,
) {
	const root = mkdtempSync(join(tmpdir(), "pina-program-policy-"));
	const base = join(root, "base");
	const head = join(root, "head");
	const manifest = { results: { example: { status: "ok" } } };
	mkdirSync(base);
	mkdirSync(head);
	writeFileSync(join(base, "manifest.json"), JSON.stringify(manifest));
	writeFileSync(join(head, "manifest.json"), JSON.stringify(manifest));
	writeFileSync(
		join(root, "policy.json"),
		JSON.stringify({ ...policy, ...approvals }),
	);

	for (
		const [directory, cu, size] of [[base, baseCu, baseSize], [
			head,
			headCu,
			headSize,
		]] as const
	) {
		writeFileSync(
			join(directory, "example.json"),
			JSON.stringify({
				total_cu: cu,
				binary_size: size,
				text_size: 5,
				total_syscalls: 1,
			}),
		);
	}

	const exitCode = run({
		policyFile: join(root, "policy.json"),
		baseRevision: revision,
		baseDir: base,
		headDir: head,
		staticOnly: true,
		markdownOutput: join(root, "comparison.md"),
		jsonOutput: join(root, "comparison.json"),
	});
	const report = JSON.parse(
		readFileSync(join(root, "comparison.json"), "utf8"),
	) as {
		summary: { failures: number; improvements: number };
		programs: Array<{ status: string; cuStatus: string; sizeStatus: string }>;
	};

	return {
		root,
		exitCode,
		report,
		markdown: readFileSync(join(root, "comparison.md"), "utf8"),
	};
}

test("every byte of binary growth fails even when compute units improve", () => {
	for (const increase of [1, 90_000]) {
		const result = staticReport(1_334, 1_333, 12_984, 12_984 + increase);
		assert.equal(result.exitCode, 2);
		assert.equal(result.report.programs[0]?.status, "fail");
		assert.equal(result.report.programs[0]?.cuStatus, "improved");
		assert.equal(result.report.programs[0]?.sizeStatus, "fail");
		assert.equal(result.report.summary.failures, 1);
		assert.equal(result.report.summary.improvements, 0);
		assert.match(result.markdown, /1 blocking regression/u);
		assert.match(result.markdown, /CU status.*Size status/u);
	}
});

test("missing static inventories fail instead of silently reporting no regressions", () => {
	for (const side of ["base", "head"] as const) {
		const { root } = staticReport(1_000, 1_000, 10_000, 10_000);
		unlinkSync(join(root, side, "manifest.json"));
		assert.equal(
			run({
				policyFile: join(root, "policy.json"),
				baseDir: join(root, "base"),
				headDir: join(root, "head"),
				staticOnly: true,
				markdownOutput: join(root, "comparison.md"),
				jsonOutput: join(root, "comparison.json"),
			}),
			1,
		);
		assert.match(
			readFileSync(join(root, "comparison.md"), "utf8"),
			/static profile inventory is missing/u,
		);
	}
});

test("runtime-only comparisons do not apply the static size gate", () => {
	const { root } = staticReport(1_000, 1_000, 10_000, 10_001);
	assert.equal(
		run({
			policyFile: join(root, "policy.json"),
			baseDir: join(root, "base"),
			headDir: join(root, "head"),
			runtimeOnly: true,
			markdownOutput: join(root, "comparison.md"),
			jsonOutput: join(root, "comparison.json"),
		}),
		0,
	);
});

test("static-only comparisons do not read runtime input files", () => {
	const { root } = staticReport(1_000, 1_000, 10_000, 10_000);
	assert.equal(
		run({
			policyFile: join(root, "policy.json"),
			baseDir: join(root, "base"),
			headDir: join(root, "head"),
			baseRuntime: join(root, "does-not-exist.json"),
			headRuntime: join(root, "does-not-exist.json"),
			baseExactRuntime: join(root, "does-not-exist.json"),
			headExactRuntime: join(root, "does-not-exist.json"),
			staticOnly: true,
			markdownOutput: join(root, "comparison.md"),
			jsonOutput: join(root, "comparison.json"),
		}),
		0,
	);
});

test("binary savings count as an improvement when compute units are unchanged", () => {
	const result = staticReport(1_000, 1_000, 10_000, 9_999);
	assert.equal(result.exitCode, 0);
	assert.equal(result.report.programs[0]?.status, "improved");
	assert.equal(result.report.programs[0]?.cuStatus, "unchanged");
	assert.equal(result.report.programs[0]?.sizeStatus, "improved");
});

test("CU and size approvals are independent and expire with the reviewed base", () => {
	const approval = {
		baseRevision,
		base: 1_000,
		head: 1_500,
		reason: "Reviewed trade-off",
	};
	const approvals = {
		staticCuApprovals: { example: approval },
		binarySizeApprovals: {
			example: { ...approval, base: 10_000, head: 10_100 },
		},
	};
	const approved = staticReport(1_000, 1_500, 10_000, 10_100, approvals);
	assert.equal(approved.exitCode, 0);
	assert.equal(approved.report.programs[0]?.cuStatus, "approved-regression");
	assert.equal(approved.report.programs[0]?.sizeStatus, "approved-regression");

	for (
		const [baseCu, headCu, baseSize, headSize, revision] of [
			[1_000, 1_500, 10_000, 10_100, "b".repeat(40)],
			[900, 1_500, 10_000, 10_100, baseRevision],
			[1_000, 1_500, 9_999, 10_100, baseRevision],
			[1_000, 1_501, 10_000, 10_100, baseRevision],
			[1_000, 1_500, 10_000, 10_101, baseRevision],
		] as const
	) {
		assert.equal(
			staticReport(baseCu, headCu, baseSize, headSize, approvals, revision)
				.exitCode,
			2,
		);
	}

	assert.equal(
		staticReport(1_000, 1_500, 10_000, 10_100, {
			staticCuApprovals: approvals.staticCuApprovals,
		}).exitCode,
		2,
	);
	assert.equal(
		staticReport(1_000, 1_500, 10_000, 10_100, {
			binarySizeApprovals: approvals.binarySizeApprovals,
		}).exitCode,
		2,
	);
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

test("runtime comparison combines discovered and focused exact cases", () => {
	const root = mkdtempSync(join(tmpdir(), "pina-runtime-sources-"));
	const base = join(root, "base");
	const head = join(root, "head");
	const baseRuntime = join(root, "runtime-base.json");
	const headRuntime = join(root, "runtime-head.json");
	const baseExactRuntime = join(root, "exact-runtime-base.json");
	const headExactRuntime = join(root, "exact-runtime-head.json");
	mkdirSync(base);
	mkdirSync(head);
	writeFileSync(join(base, "manifest.json"), JSON.stringify({ results: {} }));
	writeFileSync(join(head, "manifest.json"), JSON.stringify({ results: {} }));
	writeFileSync(
		join(root, "policy.json"),
		JSON.stringify({
			...policy,
			runtimeCases: ["exact/instruction"],
			runtimeExpectedOutcomes: { "exact/instruction": true },
		}),
	);
	writeFileSync(
		baseRuntime,
		JSON.stringify({
			cases: [{ id: "discovered/instruction", computeUnits: 1_000 }],
		}),
	);
	writeFileSync(
		headRuntime,
		JSON.stringify({
			cases: [{ id: "discovered/instruction", computeUnits: 900 }],
		}),
	);
	writeFileSync(
		baseExactRuntime,
		JSON.stringify({
			cases: [
				{ id: "exact/instruction", computeUnits: 100, succeeded: true },
				{ id: "discovered/instruction", computeUnits: 500, succeeded: true },
			],
		}),
	);
	writeFileSync(
		headExactRuntime,
		JSON.stringify({
			cases: [
				{ id: "exact/instruction", computeUnits: 100, succeeded: true },
				{ id: "discovered/instruction", computeUnits: 400, succeeded: true },
			],
		}),
	);

	const status = run({
		policyFile: join(root, "policy.json"),
		baseDir: base,
		headDir: head,
		baseRuntime,
		headRuntime,
		baseExactRuntime,
		headExactRuntime,
		runtimeOnly: true,
		markdownOutput: join(root, "comparison.md"),
		jsonOutput: join(root, "comparison.json"),
	});
	assert.equal(status, 0);

	const report = JSON.parse(
		readFileSync(join(root, "comparison.json"), "utf8"),
	) as { runtime: { cases: Array<{ id: string }> } };
	assert.deepEqual(
		report.runtime.cases.map((item) => item.id).toSorted(),
		["discovered/instruction", "exact/instruction"],
	);
});

test("discovered comparison does not require focused exact cases", () => {
	const root = mkdtempSync(join(tmpdir(), "pina-runtime-discovered-"));
	const base = join(root, "base");
	const head = join(root, "head");
	const baseRuntime = join(root, "runtime-base.json");
	const headRuntime = join(root, "runtime-head.json");
	mkdirSync(base);
	mkdirSync(head);
	writeFileSync(join(base, "manifest.json"), JSON.stringify({ results: {} }));
	writeFileSync(join(head, "manifest.json"), JSON.stringify({ results: {} }));
	writeFileSync(
		join(root, "policy.json"),
		JSON.stringify({ ...policy, runtimeCases: ["focused/instruction"] }),
	);
	writeFileSync(
		baseRuntime,
		JSON.stringify({
			cases: [{ id: "discovered/instruction", computeUnits: 1_000 }],
		}),
	);
	writeFileSync(
		headRuntime,
		JSON.stringify({
			cases: [{ id: "discovered/instruction", computeUnits: 1_000 }],
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

test("tolerated teardown exits surface as notes without failing the comparison", () => {
	const tolerated = compareRuntimeReports(
		policy,
		{ cases: [{ id: "example/instruction", computeUnits: 1_000 }] },
		{
			cases: [{ id: "example/instruction", computeUnits: 1_000 }],
			incompleteTeardowns: ["Surfpool batch 4 exited with 101"],
		},
	);
	assert.deepEqual(tolerated.hardErrors, []);
	assert.deepEqual(tolerated.notes, [
		"head benchmark tolerated a teardown exit: Surfpool batch 4 exited with 101",
	]);

	const failed = compareRuntimeReports(
		policy,
		{ cases: [{ id: "example/instruction", computeUnits: 1_000 }] },
		{
			cases: [{ id: "example/instruction", computeUnits: 1_000 }],
			testFailures: ["Surfpool batch 4 exited with 101"],
		},
	);
	assert.deepEqual(failed.hardErrors, [
		"head benchmark test failed: Surfpool batch 4 exited with 101",
	]);
	assert.deepEqual(failed.notes, []);

	const root = mkdtempSync(join(tmpdir(), "pina-runtime-teardown-"));
	const base = join(root, "base");
	const head = join(root, "head");
	const baseRuntime = join(root, "runtime-base.json");
	const headRuntime = join(root, "runtime-head.json");
	mkdirSync(base);
	mkdirSync(head);
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
			cases: [{ id: "example/instruction", computeUnits: 1_000 }],
			incompleteTeardowns: ["Surfpool batch 4 exited with 101"],
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
	assert.match(markdown, /🧹 1 tolerated teardown exit/u);
	assert.match(markdown, /Runtime measurement notes:/u);
	assert.match(
		markdown,
		/- head benchmark tolerated a teardown exit: Surfpool batch 4 exited with 101/u,
	);

	const report = JSON.parse(
		readFileSync(join(root, "comparison.json"), "utf8"),
	) as { summary: { runtimeNotes: number }; runtime: { notes: string[] } };
	assert.equal(report.summary.runtimeNotes, 1);
	assert.equal(report.runtime.notes.length, 1);
});
