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
	trackedPrograms: string[];
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
}

function requireValue(values: string[], index: number, option: string): string {
	const value = values[index + 1];
	if (value === undefined || value.startsWith("--")) {
		throw new Error(`${option} requires a value`);
	}
	return value;
}

function parseArguments(values: string[]): Arguments {
	const parsed: Partial<Arguments> = {};
	for (let index = 0; index < values.length; index += 2) {
		const option = values[index];
		if (option === undefined) {
			break;
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
	const trackedCases = new Set(policy.runtimeCases ?? []);

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

	const unexpected = [...headCases.keys()].filter((caseId) =>
		!trackedCases.has(caseId)
	).sort();
	if (unexpected.length > 0) {
		hardErrors.push(
			`head runtime report contains untracked cases: ${
				unexpected
					.map((caseId) => `\`${caseId}\``)
					.join(", ")
			}`,
		);
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

function compareStaticReports(
	policy: ComputeUnitPolicy,
	baseDir: string,
	headDir: string,
): {
	comparisons: StaticComparison[];
	skipped: string[];
	hardErrors: string[];
} {
	const comparisons: StaticComparison[] = [];
	const skipped: string[] = [];
	const hardErrors: string[] = [];
	const baseManifest = loadOptionalManifest(baseDir);
	const headManifest = loadOptionalManifest(headDir);

	for (const program of policy.trackedPrograms) {
		const base = loadOptionalProfile(baseDir, program);
		const head = loadOptionalProfile(headDir, program);
		const baseDetail = baseManifest.results?.[program]?.detail ??
			`profile unavailable in ${baseDir}`;
		const headDetail = headManifest.results?.[program]?.detail ??
			`profile unavailable in ${headDir}`;

		if (base === undefined && head === undefined) {
			hardErrors.push(
				`\`${program}\` failed because base and head profiles were unavailable (${baseDetail}; ${headDetail})`,
			);
			continue;
		}
		if (head === undefined) {
			hardErrors.push(
				`\`${program}\` produced a base profile but not a head profile (${headDetail})`,
			);
			continue;
		}
		if (base === undefined) {
			skipped.push(
				`\`${program}\` established a new head-only baseline because the base profile was unavailable (${baseDetail})`,
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
	return { comparisons, skipped, hardErrors };
}

function renderMarkdown(
	policy: ComputeUnitPolicy,
	staticComparisons: StaticComparison[],
	skipped: string[],
	staticErrors: string[],
	runtime: RuntimeComparisonResult,
): string {
	const lines = [
		"## Compute-unit regression report",
		"",
		"A positive performance change means the head uses fewer compute units. A negative change means it uses more.",
		"",
		"### Exact Mollusk instruction CU",
		"",
		"Each case runs twice against the exact copied ELF and must produce the same count. Any unapproved increase fails CI.",
		"",
	];

	if (runtime.comparisons.length > 0) {
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
	} else {
		lines.push(
			"No exact runtime cases produced comparable base/head measurements.",
		);
	}
	if (runtime.newBaselines.length > 0) {
		lines.push(
			"",
			"New runtime baselines:",
			...runtime.newBaselines.map((item) => `- ${item}`),
		);
	}
	if (runtime.hardErrors.length > 0) {
		lines.push(
			"",
			"Runtime measurement errors:",
			...runtime.hardErrors.map((item) => `- ${item}`),
		);
	}

	lines.push(
		"",
		"### Static SBF estimates",
		"",
		`Tracked programs: ${
			policy.trackedPrograms.map((program) => `\`${program}\``).join(", ")
		}`,
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
		"Summary:",
		`- compared programs: ${staticComparisons.length}`,
		`- failures: ${
			staticComparisons.filter((item) => item.status === "fail").length
		}`,
		`- warnings: ${
			staticComparisons.filter((item) => item.status === "warn").length
		}`,
		`- improvements: ${
			staticComparisons.filter((item) => item.status === "improved").length
		}`,
		`- skipped programs: ${skipped.length}`,
		`- availability errors: ${staticErrors.length}`,
		"",
	);
	if (staticComparisons.length > 0) {
		lines.push(
			"| Program | Base CU | Head CU | Performance change | Change % | Status |",
			"| ------- | ------: | ------: | -----------------: | -------: | ------ |",
		);
		for (const item of staticComparisons) {
			lines.push(
				`| \`${item.program}\` | ${formatInt(item.baseTotalCu)} | ${
					formatInt(item.headTotalCu)
				} | ${formatSignedInt(item.deltaCu)} | ${
					formatPercent(item.deltaPercent)
				} | ${statusLabel(item.status)} |`,
			);
		}
	} else {
		lines.push("No tracked programs produced comparable base/head profiles.");
	}
	if (skipped.length > 0) {
		lines.push(
			"",
			"New static baselines:",
			...skipped.map((item) => `- ${item}`),
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
		"The JSON artifact includes exact-runtime provenance plus text-section, syscall, and binary-size deltas for each statically profiled program.",
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
		staticResult.skipped,
		staticResult.hardErrors,
		runtime,
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
						skippedPrograms: staticResult.skipped.length,
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
					newStaticBaselines: staticResult.skipped,
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
