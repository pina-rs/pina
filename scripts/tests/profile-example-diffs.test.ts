import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import {
	collectFunctionDiffs,
	type CompareOutcome,
	type CompareRunner,
	type ComparisonDocument,
	type FunctionDelta,
	type FunctionDiffReport,
	planDiffs,
	renderFunctionDiffMarkdown,
	run,
} from "../profile-example-diffs.ts";

function totalsSnapshot(totalCu: number, totalInstructions: number) {
	return {
		total_cu: totalCu,
		total_instructions: totalInstructions,
		total_syscalls: 0,
		binary_size: 1000,
		text_size: totalInstructions * 8,
	};
}

function delta(
	name: string,
	change: FunctionDelta["change"],
	baselineCu: number | null,
	currentCu: number | null,
): FunctionDelta {
	const deltaCu = (currentCu ?? 0) - (baselineCu ?? 0);
	const base = baselineCu ?? 0;
	return {
		name,
		change,
		baseline_cu: baselineCu,
		current_cu: currentCu,
		delta_cu: deltaCu,
		delta_percent: base === 0
			? (deltaCu === 0 ? 0 : 100)
			: (deltaCu / base) * 100,
		baseline_instructions: baselineCu === null ? null : baselineCu,
		current_instructions: currentCu === null ? null : currentCu,
		delta_instructions: deltaCu,
	};
}

function document(
	functions: FunctionDelta[],
	options: { status?: ComparisonDocument["status"]; exceeds?: boolean } = {},
): ComparisonDocument {
	const baselineCu = functions.reduce(
		(sum, item) => sum + (item.baseline_cu ?? 0),
		0,
	);
	const currentCu = functions.reduce(
		(sum, item) => sum + (item.current_cu ?? 0),
		0,
	);
	const deltaCu = currentCu - baselineCu;
	return {
		schema_version: 1,
		baseline_program_name: "demo",
		current_program_name: "demo",
		totals: {
			baseline: totalsSnapshot(baselineCu, baselineCu),
			current: totalsSnapshot(currentCu, currentCu),
			delta_cu: deltaCu,
			delta_percent: baselineCu === 0 ? 0 : (deltaCu / baselineCu) * 100,
			delta_instructions: deltaCu,
			delta_syscalls: 0,
			delta_binary_size: 0,
			delta_text_size: deltaCu * 8,
		},
		status: options.status ?? (deltaCu === 0 ? "unchanged" : "regression"),
		threshold: { delta_cu: 500, delta_percent: 10 },
		exceeds_threshold: options.exceeds ?? false,
		functions,
	};
}

function comparedResult(
	program: string,
	functions: FunctionDelta[],
	options: { status?: ComparisonDocument["status"]; exceeds?: boolean } = {},
): FunctionDiffReport {
	return {
		schemaVersion: 1,
		summary: {
			trackedPrograms: 1,
			compared: 1,
			withChanges: functions.some((item) => item.change !== "unchanged")
				? 1
				: 0,
			unchanged: functions.every((item) => item.change === "unchanged") ? 1 : 0,
			newBaselines: 0,
			skipped: 0,
			unavailable: 0,
		},
		programs: [
			{
				program,
				classification: "compared",
				comparison: document(functions, options),
				changedFunctions:
					functions.filter((item) => item.change !== "unchanged").length,
				increasedFunctions:
					functions.filter((item) => item.delta_cu > 0).length,
			},
		],
	};
}

test("plans cover every manifest program in name order", () => {
	const base = {
		trackedPrograms: ["alpha", "beta", "gamma", "zeta"],
		results: {
			alpha: { status: "ok" },
			beta: { status: "unavailable", detail: "SBF build failed" },
			gamma: { status: "ok" },
			zeta: { status: "ok" },
		},
	};
	const head = {
		trackedPrograms: ["alpha", "beta", "gamma", "delta"],
		results: {
			alpha: { status: "ok" },
			beta: { status: "ok" },
			delta: { status: "ok" },
			gamma: { status: "ok" },
		},
	};
	assert.deepEqual(planDiffs(base, head), [
		{ program: "alpha", classification: "compare" },
		{
			program: "beta",
			classification: "skipped",
			detail: "base profile unavailable: SBF build failed",
		},
		{ program: "delta", classification: "new-baseline" },
		{ program: "gamma", classification: "compare" },
		{
			program: "zeta",
			classification: "skipped",
			detail: "program is absent from the head profile inventory",
		},
	]);
});

test("unchanged-only programs render a summary line without tables", () => {
	const report = comparedResult("demo", [
		delta("entry", "unchanged", 20, 20),
		delta("helper", "unchanged", 10, 10),
	]);
	const markdown = renderFunctionDiffMarkdown(report);
	assert.match(markdown, /^## Program function-level profile diffs$/mu);
	assert.match(markdown, /➖ 1 unchanged/u);
	assert.doesNotMatch(markdown, /### /u);
	assert.doesNotMatch(markdown, /\| Function \|/u);
	assert.match(markdown, /Unchanged: `demo`\./u);
	assert.match(
		markdown,
		/enforcement remains the instruction compute-unit policy gate/u,
	);
});

test("mixed added, removed, and changed functions render a sorted table", () => {
	// Mirrors the CLI ordering: |delta| descending with a name tie-break.
	const report = comparedResult("demo", [
		delta("added", "added", null, 10),
		delta("grew", "changed", 20, 30),
		delta("removed", "removed", 10, null),
		delta("shrank", "changed", 20, 10),
	]);
	const markdown = renderFunctionDiffMarkdown(report);

	assert.match(markdown, /### `demo`/u);
	assert.match(
		markdown,
		/Program total: 50 → 50 CU \(· \+0, \+0\.0%\)\./u,
	);

	const table = markdown.slice(
		markdown.indexOf("| Function |"),
		markdown.indexOf("\n\n", markdown.indexOf("| Function |")),
	);
	const rows = table.split("\n").filter((line) => line.startsWith("| `"));
	assert.deepEqual(
		rows.map((row) => row.split(" |")[0]),
		["| `added`", "| `grew`", "| `removed`", "| `shrank`"],
	);
	assert.match(rows[0] ?? "", /— \| 10 \| ↑ \+10 \(new\) \| 🆕 added \|/u);
	assert.match(
		rows[1] ?? "",
		/20 \| 30 \| ↑ \+10 \(\+50\.0%\) \| ⚠️ increased \|/u,
	);
	assert.match(rows[2] ?? "", /10 \| — \| ↓ -10 \(removed\) \| removed \|/u);
	assert.match(rows[3] ?? "", /20 \| 10 \| ↓ -10 \(-50\.0%\) \| decreased \|/u);
	assert.doesNotMatch(markdown, /Unchanged: /u);
});

test("a threshold regression is flagged in the program total", () => {
	const report = comparedResult(
		"demo",
		[delta("entry", "changed", 1_000, 1_650)],
		{ status: "threshold-regression", exceeds: true },
	);
	const markdown = renderFunctionDiffMarkdown(report);
	assert.match(
		markdown,
		/`entry` \| 1,000 \| 1,650 \| ↑ \+650 \(\+65\.0%\) \| ⚠️ increased \|/u,
	);
	assert.match(
		markdown,
		/Program total: 1,000 → 1,650 CU \(↑ \+650, \+65\.0%\) — ⚠️ exceeds the local compare threshold\./u,
	);
});

test("new baselines, skipped, and unavailable programs are listed", () => {
	const report: FunctionDiffReport = {
		schemaVersion: 1,
		summary: {
			trackedPrograms: 4,
			compared: 1,
			withChanges: 1,
			unchanged: 0,
			newBaselines: 1,
			skipped: 1,
			unavailable: 1,
		},
		programs: [
			{
				program: "beta",
				classification: "new-baseline",
				headTotalCu: 1_234,
			},
			{
				program: "gamma",
				classification: "skipped",
				detail: "head profile unavailable: SBF build failed with status 1",
			},
			{
				program: "zeta",
				classification: "unavailable",
				detail:
					"pina profile compare exited with status 1: Error: baseline is not valid JSON",
			},
			{
				program: "alpha",
				classification: "compared",
				comparison: document([delta("entry", "changed", 10, 30)]),
				changedFunctions: 1,
				increasedFunctions: 1,
			},
		],
	};
	const markdown = renderFunctionDiffMarkdown(report);
	assert.match(
		markdown,
		/New baselines \(no base profile\): `beta` \(1,234 CU\)\./u,
	);
	assert.match(
		markdown,
		/- `gamma` — head profile unavailable: SBF build failed with status 1/u,
	);
	assert.match(
		markdown,
		/- `zeta` — pina profile compare exited with status 1: Error: baseline is not valid JSON/u,
	);
	assert.match(markdown, /⚠️ 1 diff unavailable/u);
	const sectionStart = markdown.indexOf("### `alpha`");
	const newBaselinesStart = markdown.indexOf("New baselines");
	assert.ok(sectionStart > 0);
	assert.ok(newBaselinesStart > sectionStart);
});

test("rendering is deterministic", () => {
	const report = comparedResult("demo", [
		delta("added", "added", null, 10),
		delta("grew", "changed", 20, 30),
	]);
	assert.equal(
		renderFunctionDiffMarkdown(report),
		renderFunctionDiffMarkdown(report),
	);
});

test("collect pairs ok programs and isolates compare failures", () => {
	const base = {
		trackedPrograms: ["alpha", "beta"],
		results: {
			alpha: { status: "ok" },
			beta: { status: "ok" },
		},
	};
	const head = {
		trackedPrograms: ["alpha", "beta"],
		results: {
			alpha: { status: "ok" },
			beta: { status: "ok" },
		},
	};
	const calls: Array<[string, string]> = [];
	const runner: CompareRunner = (baselinePath, artifactPath) => {
		calls.push([baselinePath, artifactPath]);
		if (baselinePath.includes("alpha")) {
			const outcome: CompareOutcome = {
				status: 0,
				stdout: JSON.stringify(document([delta("entry", "changed", 10, 30)])),
				stderr: "",
			};
			return outcome;
		}
		return {
			status: 1,
			stdout: "",
			stderr: "Error: \u001B[31mbaseline\u001B[0m is not valid JSON",
		};
	};

	const report = collectFunctionDiffs(base, head, "/base", "/head", runner);
	assert.deepEqual(calls, [
		["/base/alpha.json", "/head/alpha.so"],
		["/base/beta.json", "/head/beta.so"],
	]);
	assert.deepEqual(report.summary, {
		trackedPrograms: 2,
		compared: 1,
		withChanges: 1,
		unchanged: 0,
		newBaselines: 0,
		skipped: 0,
		unavailable: 1,
	});
	const beta = report.programs.find((program) => program.program === "beta");
	assert.equal(beta?.classification, "unavailable");
	assert.match(beta?.detail ?? "", /status 1.*baseline is not valid JSON/su);
	assert.doesNotMatch(beta?.detail ?? "", /\u001B\[/u);
	const alpha = report.programs.find((program) => program.program === "alpha");
	assert.equal(alpha?.comparison?.functions[0]?.delta_cu, 20);
});

test("run writes markdown and json artifacts without failing on regressions", () => {
	const root = mkdtempSync(join(tmpdir(), "pina-function-diffs-"));
	const base = join(root, "base");
	const head = join(root, "head");
	const output = join(root, "function-diffs");
	mkdirSync(base);
	mkdirSync(head);
	writeFileSync(
		join(base, "manifest.json"),
		JSON.stringify({
			trackedPrograms: ["alpha", "beta", "gamma"],
			results: {
				alpha: { status: "ok" },
				beta: { status: "ok" },
				gamma: { status: "unavailable", detail: "SBF build failed" },
			},
		}),
	);
	writeFileSync(
		join(head, "manifest.json"),
		JSON.stringify({
			trackedPrograms: ["alpha", "beta", "gamma", "delta"],
			results: {
				alpha: { status: "ok" },
				beta: { status: "ok" },
				gamma: { status: "unavailable", detail: "SBF build failed" },
				delta: { status: "ok" },
			},
		}),
	);
	writeFileSync(
		join(base, "alpha.json"),
		JSON.stringify({ program_name: "alpha", total_cu: 1_000 }),
	);
	writeFileSync(join(head, "alpha.so"), "elf");
	writeFileSync(
		join(head, "alpha.json"),
		JSON.stringify({ program_name: "alpha", total_cu: 1_650 }),
	);
	writeFileSync(
		join(head, "delta.json"),
		JSON.stringify({ program_name: "delta", total_cu: 650 }),
	);

	const runner: CompareRunner = (baselinePath) => {
		if (baselinePath.endsWith("alpha.json")) {
			return {
				status: 2,
				stdout: JSON.stringify(
					document([delta("entry", "changed", 1_000, 1_650)], {
						status: "threshold-regression",
						exceeds: true,
					}),
				),
				stderr: "",
			};
		}
		return {
			status: 1,
			stdout: "",
			stderr: "Error: profile artifact is unreadable",
		};
	};

	const status = run({
		baseDir: base,
		headDir: head,
		outputDir: output,
		runCompare: runner,
	});
	assert.equal(status, 0);

	const markdown = readFileSync(join(output, "function-diff.md"), "utf8");
	assert.match(markdown, /### `alpha`/u);
	assert.match(
		markdown,
		/New baselines \(no base profile\): `delta` \(650 CU\)\./u,
	);
	assert.match(
		markdown,
		/- `beta` — pina profile compare exited with status 1: Error: profile artifact is unreadable/u,
	);
	assert.match(
		markdown,
		/- `gamma` — head profile unavailable: SBF build failed/u,
	);
	assert.match(markdown, /This section is diagnostic/u);

	const parsed = JSON.parse(
		readFileSync(join(output, "function-diff.json"), "utf8"),
	) as FunctionDiffReport;
	assert.deepEqual(parsed.summary, {
		trackedPrograms: 4,
		compared: 1,
		withChanges: 1,
		unchanged: 0,
		newBaselines: 1,
		skipped: 1,
		unavailable: 1,
	});
	assert.equal(parsed.programs[0]?.comparison?.exceeds_threshold, true);
	assert.equal(parsed.programs[0]?.comparison?.status, "threshold-regression");
});

test("planDiffs rejects program names that could escape the report directory", () => {
	const manifest = {
		trackedPrograms: ["counter_program", "../evil"],
		results: {
			counter_program: { status: "ok" },
			"../evil": { status: "ok" },
		},
	};

	assert.throws(
		() => planDiffs(manifest, manifest),
		/invalid program name in profile manifest/,
	);
});

test("planDiffs accepts the tracked program name shape", () => {
	const manifest = {
		trackedPrograms: ["counter_program", "pina_bpf_program"],
		results: {
			counter_program: { status: "ok" },
			pina_bpf_program: { status: "ok" },
		},
	};

	assert.equal(planDiffs(manifest, manifest).length, 2);
});

test("a baseline/current program name mismatch is reported as unavailable", () => {
	const manifest = {
		trackedPrograms: ["counter_program"],
		results: { counter_program: { status: "ok" } },
	};
	const mismatched = document([delta("entry", "unchanged", 20, 20)], {});
	const overridden = { ...mismatched, baseline_program_name: "other_program" };
	const report = collectFunctionDiffs(
		manifest,
		manifest,
		"/base",
		"/head",
		() => ({ status: 0, stdout: JSON.stringify(overridden), stderr: "" }),
	);

	assert.equal(report.programs[0].classification, "unavailable");
	assert.match(
		report.programs[0].detail ?? "",
		/differs from current program/,
	);
});
