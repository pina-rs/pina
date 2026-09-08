#!/usr/bin/env node

import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { pathToFileURL } from "node:url";

const OK_STATUS = "ok";

interface Threshold {
	deltaCu: number;
	deltaPercent: number;
}

export interface ComputeUnitPolicy {
	excludedPrograms?: string[];
	trackedPrograms?: string[];
	runtimeCases?: string[];
	warn: Threshold;
	fail: Threshold;
	approvedTotals?: Record<string, number>;
	runtimeApprovedTotals?: Record<string, number>;
}

interface ProfileManifestResult {
	status?: string;
	detail?: string;
}

interface ProfileManifest {
	results?: Record<string, ProfileManifestResult>;
}

interface StaticProfile {
	total_cu: number;
	binary_size: number;
	text_size: number;
	total_syscalls: number;
}

interface RuntimeCase {
	id: string;
	computeUnits: number;
}

interface RuntimeReport {
	provenance?: unknown;
	cases?: RuntimeCase[];
	missingCases?: string[];
	testFailures?: string[];
	unavailablePrograms?: string[];
}

type ComparisonStatus =
	| "fail"
	| "warn"
	| "improved"
	| "unchanged"
	| "small-regression"
	| "approved-regression";

interface StaticComparison {
	program: string;
	status: ComparisonStatus;
	baseTotalCu: number;
	headTotalCu: number;
	deltaCu: number;
	deltaPercent: number;
	headMinusBaseCu: number;
	baseBinarySize: number;
	headBinarySize: number;
	deltaBinarySize: number;
	baseTextSize: number;
	headTextSize: number;
	deltaTextSize: number;
	baseTotalSyscalls: number;
	headTotalSyscalls: number;
	deltaTotalSyscalls: number;
}

interface StaticBaseline {
	program: string;
	totalCu: number;
	binarySize: number;
}

export interface RuntimeComparison {
	id: string;
	status: ComparisonStatus;
	baseCu: number;
	headCu: number;
	deltaCu: number;
	deltaPercent: number;
	headMinusBaseCu: number;
}

interface RuntimeComparisonResult {
	comparisons: RuntimeComparison[];
	newBaselines: string[];
	hardErrors: string[];
}

interface Arguments {
	policyFile: string;
	baseDir: string;
	headDir: string;
	markdownOutput: string;
	jsonOutput: string;
	baseRuntime?: string;
	headRuntime?: string;
	runtimeOnly?: boolean;
	staticOnly?: boolean;
}

function requireValue(values: string[], index: number, option: string): string {
	const value = values[index + 1];
	if (value === undefined || value.startsWith("--")) {
		throw new Error(`${option} requires a value`);
	}
	return value;
}

function parseArguments(values: string[]): Arguments {
	const parsed: Partial<Arguments> = {
		runtimeOnly: false,
		staticOnly: false,
	};
	for (let index = 0; index < values.length;) {
		const option = values[index];
		if (option === undefined) {
			break;
		}
		if (option === "--runtime-only" || option === "--static-only") {
			parsed[option === "--runtime-only" ? "runtimeOnly" : "staticOnly"] = true;
			index += 1;
			continue;
		}
		const value = requireValue(values, index, option);
		switch (option) {
			case "--policy-file":
				parsed.policyFile = value;
				break;
			case "--base-dir":
				parsed.baseDir = value;
				break;
			case "--head-dir":
				parsed.headDir = value;
				break;
			case "--markdown-output":
				parsed.markdownOutput = value;
				break;
			case "--json-output":
				parsed.jsonOutput = value;
				break;
			case "--base-runtime":
				parsed.baseRuntime = value;
				break;
			case "--head-runtime":
				parsed.headRuntime = value;
				break;
			default:
				throw new Error(`unknown option: ${option}`);
		}
		index += 2;
	}

	if (parsed.runtimeOnly && parsed.staticOnly) {
		throw new Error("--runtime-only and --static-only are mutually exclusive");
	}

	for (
		const key of [
			"policyFile",
			"baseDir",
			"headDir",
			"markdownOutput",
			"jsonOutput",
		] as const
	) {
		if (parsed[key] === undefined) {
			throw new Error(`missing required option for ${key}`);
		}
	}

	if (
		(parsed.baseRuntime === undefined) !== (parsed.headRuntime === undefined)
	) {
		throw new Error(
			"--base-runtime and --head-runtime must be provided together",
		);
	}

	return parsed as Arguments;
}

function loadJson<T>(path: string): T {
	const value: unknown = JSON.parse(readFileSync(path, "utf8"));
	return value as T;
}

function loadOptionalManifest(directory: string): ProfileManifest {
	try {
		return loadJson<ProfileManifest>(join(directory, "manifest.json"));
	} catch (error: unknown) {
		if ((error as NodeJS.ErrnoException).code === "ENOENT") {
			return {};
		}
		throw error;
	}
}

function loadOptionalProfile(
	directory: string,
	program: string,
): StaticProfile | undefined {
	try {
		return loadJson<StaticProfile>(join(directory, `${program}.json`));
	} catch (error: unknown) {
		if ((error as NodeJS.ErrnoException).code === "ENOENT") {
			return undefined;
		}
		throw error;
	}
}

export function performancePercent(
	baseValue: number,
	headValue: number,
): number {
	if (baseValue === 0) {
		return headValue === 0 ? 0 : -100;
	}
	return ((baseValue - headValue) / baseValue) * 100;
}

function classifyStatic(
	program: string,
	baseTotalCu: number,
	headTotalCu: number,
	performanceScoreCu: number,
	performanceScorePercent: number,
	policy: ComputeUnitPolicy,
): ComparisonStatus {
	if (performanceScoreCu > 0) {
		return "improved";
	}
	if (performanceScoreCu === 0) {
		return "unchanged";
	}

	const regressionCu = -performanceScoreCu;
	const regressionPercent = -performanceScorePercent;
	let status: ComparisonStatus;
	if (
		regressionCu >= policy.fail.deltaCu &&
		regressionPercent >= policy.fail.deltaPercent
	) {
		status = "fail";
	} else if (
		regressionCu >= policy.warn.deltaCu &&
		regressionPercent >= policy.warn.deltaPercent
	) {
		status = "warn";
	} else {
		status = "small-regression";
	}

	const approvedTotal = policy.approvedTotals?.[program];
	if (
		(status === "fail" || status === "warn") &&
		approvedTotal !== undefined &&
		baseTotalCu < approvedTotal &&
		headTotalCu <= approvedTotal
	) {
		return "approved-regression";
	}
	return status;
}

function classifyRuntime(
	caseId: string,
	baseCu: number,
	headCu: number,
	performanceScoreCu: number,
	policy: ComputeUnitPolicy,
): ComparisonStatus {
	if (performanceScoreCu > 0) {
		return "improved";
	}
	if (performanceScoreCu === 0) {
		return "unchanged";
	}

	const approvedTotal = policy.runtimeApprovedTotals?.[caseId];
	if (
		approvedTotal !== undefined && baseCu < approvedTotal &&
		headCu <= approvedTotal
	) {
		return "approved-regression";
	}
	return "fail";
}

export function compareRuntimeReports(
	policy: ComputeUnitPolicy,
	baseReport: RuntimeReport,
	headReport: RuntimeReport,
): RuntimeComparisonResult {
	const comparisons: RuntimeComparison[] = [];
	const newBaselines: string[] = [];
	const hardErrors: string[] = [];
	const baseCases = new Map(
		(baseReport.cases ?? []).map((item) => [item.id, item]),
	);
	const headCases = new Map(
		(headReport.cases ?? []).map((item) => [item.id, item]),
	);
	const trackedCases = new Set([
		...baseCases.keys(),
		...headCases.keys(),
		...(policy.runtimeCases ?? []),
	]);

	for (const caseId of trackedCases) {
		const base = baseCases.get(caseId);
		const head = headCases.get(caseId);
		if (base === undefined && head === undefined) {
			hardErrors.push(`\`${caseId}\` is missing from both runtime reports`);
			continue;
		}
		if (head === undefined) {
			hardErrors.push(`\`${caseId}\` is missing from the head runtime report`);
			continue;
		}
		if (base === undefined) {
			newBaselines.push(
				`\`${caseId}\` established a new baseline at ${
					formatInt(head.computeUnits)
				} CU`,
			);
			continue;
		}

		const deltaCu = base.computeUnits - head.computeUnits;
		comparisons.push({
			id: caseId,
			status: classifyRuntime(
				caseId,
				base.computeUnits,
				head.computeUnits,
				deltaCu,
				policy,
			),
			baseCu: base.computeUnits,
			headCu: head.computeUnits,
			deltaCu,
			deltaPercent: performancePercent(base.computeUnits, head.computeUnits),
			headMinusBaseCu: head.computeUnits - base.computeUnits,
		});
	}

	for (const missing of headReport.missingCases ?? []) {
		hardErrors.push(
			`\`${missing}\` has no successful or expected-error Surfpool coverage`,
		);
	}

	for (const failure of headReport.testFailures ?? []) {
		hardErrors.push(`head benchmark test failed: ${failure}`);
	}

	for (const program of headReport.unavailablePrograms ?? []) {
		hardErrors.push(`head benchmark ELF is unavailable for \`${program}\``);
	}
	return { comparisons, newBaselines, hardErrors };
}

function formatInt(value: number): string {
	return new Intl.NumberFormat("en-US", { maximumFractionDigits: 0 }).format(
		value,
	);
}

function formatSignedInt(value: number): string {
	return `${value >= 0 ? "+" : ""}${formatInt(value)}`;
}

function formatPercent(value: number): string {
	return `${value >= 0 ? "+" : ""}${value.toFixed(1)}%`;
}

function statusLabel(status: ComparisonStatus): string {
	return {
		fail: "❌ fail",
		warn: "⚠️ warn",
		improved: "✅ improved",
		unchanged: "➖ unchanged",
		"small-regression": "⚠️ small regression",
		"approved-regression": "⚠️ approved regression",
	}[status];
}

interface BenchmarkSummaryCounts {
	advisoryRegressions: number;
	blockingRegressions: number;
	errors: number;
	improvements: number;
	newBaselines: number;
	unchanged: number;
}

function formatCount(
	count: number,
	singular: string,
	plural = `${singular}s`,
): string {
	return `${formatInt(count)} ${count === 1 ? singular : plural}`;
}

function benchmarkSummary(
	counts: BenchmarkSummaryCounts,
	extra: string[] = [],
): string {
	const health: string[] = [];

	if (counts.errors > 0) {
		health.push(`❌ ${formatCount(counts.errors, "measurement error")}`);
	}

	if (counts.blockingRegressions > 0) {
		health.push(
			`❌ ${formatCount(counts.blockingRegressions, "blocking regression")}`,
		);
	}

	if (counts.advisoryRegressions > 0) {
		health.push(
			`⚠️ ${formatCount(counts.advisoryRegressions, "advisory regression")}`,
		);
	}

	if (health.length === 0) {
		health.push("✅ No regressions or measurement errors");
	}

	return [
		...health,
		`🚀 ${formatCount(counts.improvements, "improvement")}`,
		`➖ ${formatCount(counts.unchanged, "unchanged result")}`,
		...(counts.newBaselines > 0
			? [`🆕 ${formatCount(counts.newBaselines, "new baseline")}`]
			: []),
		...extra,
	].join(" · ");
}

function compareStaticReports(
	policy: ComputeUnitPolicy,
	baseDir: string,
	headDir: string,
): {
	comparisons: StaticComparison[];
	newBaselines: StaticBaseline[];
	removedPrograms: string[];
	hardErrors: string[];
} {
	const comparisons: StaticComparison[] = [];
	const newBaselines: StaticBaseline[] = [];
	const removedPrograms: string[] = [];
	const hardErrors: string[] = [];
	const baseManifest = loadOptionalManifest(baseDir);
	const headManifest = loadOptionalManifest(headDir);
	const programs = new Set([
		...(policy.trackedPrograms ?? []),
		...Object.keys(baseManifest.results ?? {}),
		...Object.keys(headManifest.results ?? {}),
	]);

	for (const program of [...programs].toSorted()) {
		const base = loadOptionalProfile(baseDir, program);
		const head = loadOptionalProfile(headDir, program);
		const baseTracked = baseManifest.results?.[program] !== undefined;
		const headTracked = headManifest.results?.[program] !== undefined;
		const baseDetail = baseManifest.results?.[program]?.detail ??
			`profile unavailable in ${baseDir}`;
		const headDetail = headManifest.results?.[program]?.detail ??
			`profile unavailable in ${headDir}`;

		if (!baseTracked && !headTracked) {
			hardErrors.push(
				`\`${program}\` is configured but absent from both profile inventories`,
			);
			continue;
		}

		if (!headTracked) {
			removedPrograms.push(program);
			continue;
		}

		if (head === undefined) {
			hardErrors.push(
				`\`${program}\` did not produce a head profile (${headDetail})`,
			);
			continue;
		}

		if (!baseTracked) {
			newBaselines.push({
				program,
				totalCu: head.total_cu,
				binarySize: head.binary_size,
			});
			continue;
		}

		if (base === undefined) {
			hardErrors.push(
				`\`${program}\` did not produce a base profile (${baseDetail})`,
			);
			continue;
		}

		const deltaCu = base.total_cu - head.total_cu;
		const deltaPercent = performancePercent(base.total_cu, head.total_cu);
		comparisons.push({
			program,
			status: classifyStatic(
				program,
				base.total_cu,
				head.total_cu,
				deltaCu,
				deltaPercent,
				policy,
			),
			baseTotalCu: base.total_cu,
			headTotalCu: head.total_cu,
			deltaCu,
			deltaPercent,
			headMinusBaseCu: head.total_cu - base.total_cu,
			baseBinarySize: base.binary_size,
			headBinarySize: head.binary_size,
			deltaBinarySize: head.binary_size - base.binary_size,
			baseTextSize: base.text_size,
			headTextSize: head.text_size,
			deltaTextSize: head.text_size - base.text_size,
			baseTotalSyscalls: base.total_syscalls,
			headTotalSyscalls: head.total_syscalls,
			deltaTotalSyscalls: head.total_syscalls - base.total_syscalls,
		});
	}
	return { comparisons, newBaselines, removedPrograms, hardErrors };
}

function renderMarkdown(
	policy: ComputeUnitPolicy,
	staticComparisons: StaticComparison[],
	newStaticBaselines: StaticBaseline[],
	removedPrograms: string[],
	staticErrors: string[],
	runtime: RuntimeComparisonResult,
	runtimeOnly: boolean,
	staticOnly: boolean,
): string {
	const runtimeBlockingRegressions = runtime.comparisons.filter(
		(item) => item.status === "fail",
	).length;
	const runtimeAdvisoryRegressions = runtime.comparisons.filter(
		(item) => item.status === "approved-regression",
	).length;
	const runtimeImprovements = runtime.comparisons.filter(
		(item) => item.status === "improved",
	).length;
	const runtimeUnchanged = runtime.comparisons.filter(
		(item) => item.status === "unchanged",
	).length;
	const runtimeSummary = benchmarkSummary({
		advisoryRegressions: runtimeAdvisoryRegressions,
		blockingRegressions: runtimeBlockingRegressions,
		errors: runtime.hardErrors.length,
		improvements: runtimeImprovements,
		newBaselines: runtime.newBaselines.length,
		unchanged: runtimeUnchanged,
	});
	const staticBlockingRegressions = staticComparisons.filter(
		(item) => item.status === "fail",
	).length;
	const staticAdvisoryRegressions =
		staticComparisons.filter((item) =>
			["warn", "small-regression", "approved-regression"].includes(item.status)
		).length;
	const staticImprovements = staticComparisons.filter(
		(item) => item.status === "improved",
	).length;
	const staticUnchanged = staticComparisons.filter(
		(item) => item.status === "unchanged",
	).length;
	const smallerPrograms = staticComparisons.filter(
		(item) => item.deltaBinarySize < 0,
	).length;
	const largerPrograms = staticComparisons.filter(
		(item) => item.deltaBinarySize > 0,
	).length;
	const staticSummary = benchmarkSummary(
		{
			advisoryRegressions: staticAdvisoryRegressions,
			blockingRegressions: staticBlockingRegressions,
			errors: staticErrors.length,
			improvements: staticImprovements,
			newBaselines: newStaticBaselines.length,
			unchanged: staticUnchanged,
		},
		[
			`📦 ${formatCount(smallerPrograms, "smaller program")}`,
			`📦 ${formatCount(largerPrograms, "larger program")}`,
			...(removedPrograms.length > 0
				? [`🗂️ ${formatCount(removedPrograms.length, "removed program")}`]
				: []),
		],
	);
	const lines = staticOnly
		? [
			"## Program compute units and build sizes",
			"",
			staticSummary,
			"",
			"<details>",
			"<summary>View program benchmark details</summary>",
			"",
		]
		: [
			runtimeOnly
				? "## Instruction compute units"
				: "## Compute-unit regression report",
			"",
			...(runtimeOnly ? [] : ["### Instruction compute units", ""]),
			runtimeSummary,
			"",
			"<details>",
			"<summary>View instruction benchmark details</summary>",
			"",
			"A positive performance change means the head uses fewer compute units. A negative change means it uses more.",
			"",
			"Every instruction exercised by the example Surfpool suites is simulated against the exact base and head ELFs. The maximum observed CU per instruction is compared, and any unapproved increase fails CI.",
			"",
		];

	if (!staticOnly && runtime.comparisons.length > 0) {
		lines.push(
			"| Instruction case | Base CU | Head CU | Performance change | Change % | Status |",
			"| ---------------- | ------: | ------: | -----------------: | -------: | ------ |",
		);
		for (const item of runtime.comparisons) {
			lines.push(
				`| \`${item.id}\` | ${formatInt(item.baseCu)} | ${
					formatInt(item.headCu)
				} | ${formatSignedInt(item.deltaCu)} | ${
					formatPercent(item.deltaPercent)
				} | ${statusLabel(item.status)} |`,
			);
		}
	} else if (!staticOnly) {
		lines.push(
			"No instruction cases produced comparable base/head measurements.",
		);
	}
	if (!staticOnly && runtime.newBaselines.length > 0) {
		lines.push(
			"",
			"New runtime baselines:",
			...runtime.newBaselines.map((item) => `- ${item}`),
		);
	}
	if (!staticOnly && runtime.hardErrors.length > 0) {
		lines.push(
			"",
			"Runtime measurement errors:",
			...runtime.hardErrors.map((item) => `- ${item}`),
		);
	}
	if (runtimeOnly) {
		lines.push("", "</details>", "");
		return lines.join("\n");
	}

	lines.push(
		...(staticOnly ? [] : [
			"",
			"</details>",
			"",
			"### Static SBF estimates",
			"",
			staticSummary,
			"",
			"<details>",
			"<summary>View program benchmark details</summary>",
			"",
		]),
		"Static SBF estimates and ELF build sizes.",
		"",
		"Policy:",
		`- warn when \`total_cu\` increases by at least +${policy.warn.deltaCu} CU and +${
			policy.warn.deltaPercent.toFixed(1)
		}%`,
		`- fail when \`total_cu\` increases by at least +${policy.fail.deltaCu} CU and +${
			policy.fail.deltaPercent.toFixed(1)
		}%`,
		"- explicit absolute totals approve reviewed redesigns once without weakening future relative checks",
		"- savings are positive; increases are negative and visibly marked as regressions",
		"- smaller increases pass the threshold gate but are not labeled as improvements",
		"- values come from `pina profile` static SBF estimates, not runtime validator traces",
		"",
	);
	if (staticComparisons.length > 0) {
		lines.push(
			"| Program | Base CU | Head CU | CU change | Change % | Base size | Head size | Size change | Status |",
			"| ------- | ------: | ------: | --------: | -------: | --------: | --------: | ----------: | ------ |",
		);
		for (const item of staticComparisons) {
			lines.push(
				`| \`${item.program}\` | ${formatInt(item.baseTotalCu)} | ${
					formatInt(item.headTotalCu)
				} | ${formatSignedInt(item.deltaCu)} | ${
					formatPercent(item.deltaPercent)
				} | ${formatInt(item.baseBinarySize)} B | ${
					formatInt(item.headBinarySize)
				} B | ${formatSignedInt(item.deltaBinarySize)} B | ${
					statusLabel(item.status)
				} |`,
			);
		}
	} else {
		lines.push("No programs produced comparable base/head profiles.");
	}
	if (newStaticBaselines.length > 0) {
		lines.push(
			"",
			"New program baselines:",
			"",
			"| Program | Current CU | Current build size |",
			"| ------- | ---------: | -----------------: |",
			...newStaticBaselines.map((item) =>
				`| \`${item.program}\` | ${formatInt(item.totalCu)} | ${
					formatInt(item.binarySize)
				} B |`
			),
		);
	}
	if (removedPrograms.length > 0) {
		lines.push(
			"",
			`Removed programs: ${
				removedPrograms.map((program) => `\`${program}\``).join(", ")
			}`,
		);
	}
	if (staticErrors.length > 0) {
		lines.push(
			"",
			"Static profile errors:",
			...staticErrors.map((item) => `- ${item}`),
		);
	}
	lines.push(
		"",
		"Program CU and build sizes come from `pina profile`. The JSON artifact also includes instruction-runtime provenance, text-section sizes, and syscall deltas.",
		"",
		"</details>",
		"",
	);
	return lines.join("\n");
}

export function run(arguments_: Arguments): number {
	const policy = loadJson<ComputeUnitPolicy>(arguments_.policyFile);
	const staticResult = compareStaticReports(
		policy,
		arguments_.baseDir,
		arguments_.headDir,
	);
	let baseRuntime: RuntimeReport = {};
	let headRuntime: RuntimeReport = {};
	let runtime: RuntimeComparisonResult = {
		comparisons: [],
		newBaselines: [],
		hardErrors: [],
	};
	if (
		arguments_.baseRuntime !== undefined && arguments_.headRuntime !== undefined
	) {
		baseRuntime = loadJson<RuntimeReport>(arguments_.baseRuntime);
		headRuntime = loadJson<RuntimeReport>(arguments_.headRuntime);
		runtime = compareRuntimeReports(policy, baseRuntime, headRuntime);
	}

	const markdown = renderMarkdown(
		policy,
		staticResult.comparisons,
		staticResult.newBaselines,
		staticResult.removedPrograms,
		staticResult.hardErrors,
		runtime,
		arguments_.runtimeOnly ?? false,
		arguments_.staticOnly ?? false,
	);
	mkdirSync(dirname(arguments_.markdownOutput), { recursive: true });
	mkdirSync(dirname(arguments_.jsonOutput), { recursive: true });
	writeFileSync(arguments_.markdownOutput, markdown, "utf8");
	writeFileSync(
		arguments_.jsonOutput,
		`${
			JSON.stringify(
				{
					policy,
					summary: {
						runtimeComparedCases: runtime.comparisons.length,
						runtimeFailures:
							runtime.comparisons.filter((item) => item.status === "fail")
								.length,
						runtimeNewBaselines: runtime.newBaselines.length,
						runtimeErrors: runtime.hardErrors.length,
						comparedPrograms: staticResult.comparisons.length,
						failures:
							staticResult.comparisons.filter((item) => item.status === "fail")
								.length,
						warnings:
							staticResult.comparisons.filter((item) => item.status === "warn")
								.length,
						improvements: staticResult.comparisons.filter((item) =>
							item.status === "improved"
						)
							.length,
						newProgramBaselines: staticResult.newBaselines.length,
						removedPrograms: staticResult.removedPrograms.length,
						availabilityErrors: staticResult.hardErrors.length,
					},
					programs: staticResult.comparisons,
					runtime: {
						baseProvenance: baseRuntime.provenance,
						headProvenance: headRuntime.provenance,
						cases: runtime.comparisons,
						newBaselines: runtime.newBaselines,
						errors: runtime.hardErrors,
					},
					newStaticBaselines: staticResult.newBaselines,
					removedPrograms: staticResult.removedPrograms,
					staticErrors: staticResult.hardErrors,
				},
				null,
				2,
			)
		}\n`,
		"utf8",
	);
	process.stdout.write(`${markdown}\n`);

	if (staticResult.hardErrors.length > 0 || runtime.hardErrors.length > 0) {
		return 1;
	}
	const failed = [...staticResult.comparisons, ...runtime.comparisons].some(
		(item) => item.status === "fail",
	);
	return failed ? 2 : 0;
}

function main(): number {
	try {
		return run(parseArguments(process.argv.slice(2)));
	} catch (error: unknown) {
		process.stderr.write(
			`Error: ${error instanceof Error ? error.message : String(error)}\n`,
		);
		return 1;
	}
}

if (
	process.argv[1] !== undefined &&
	import.meta.url === pathToFileURL(process.argv[1]).href
) {
	process.exitCode = main();
}
