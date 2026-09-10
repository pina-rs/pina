#!/usr/bin/env node

/** Function-level profile diffs between the base and head static profiles. */

import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const OK_STATUS = "ok";

/** Exit status `pina profile compare` uses when the total regression reaches the threshold. */
const COMPARE_THRESHOLD_STATUS = 2;

/** Same program-name shape `profile-tracked-examples.ts` enforces when building. */
const VALID_PROGRAM_NAME = /^[A-Za-z0-9_-]+$/u;

interface ManifestResult {
	status?: string;
	detail?: string;
}

interface ProfileManifest {
	sourceRevision?: string;
	trackedPrograms?: string[];
	results?: Record<string, ManifestResult>;
}

interface ProfileTotals {
	total_cu: number;
	total_instructions: number;
	total_syscalls: number;
	binary_size: number;
	text_size: number;
}

export interface FunctionDelta {
	name: string;
	change: "added" | "removed" | "changed" | "unchanged";
	baseline_cu: number | null;
	current_cu: number | null;
	delta_cu: number;
	delta_percent: number;
	baseline_instructions: number | null;
	current_instructions: number | null;
	delta_instructions: number;
}

/** The `pina profile compare --json` comparison document. */
export interface ComparisonDocument {
	schema_version: number;
	baseline_program_name: string;
	current_program_name: string;
	totals: {
		baseline: ProfileTotals;
		current: ProfileTotals;
		delta_cu: number;
		delta_percent: number;
		delta_instructions: number;
		delta_syscalls: number;
		delta_binary_size: number;
		delta_text_size: number;
	};
	status: "unchanged" | "improved" | "regression" | "threshold-regression";
	threshold: { delta_cu: number; delta_percent: number };
	exceeds_threshold: boolean;
	functions: FunctionDelta[];
}

export type FunctionDiffClassification =
	| "compared"
	| "new-baseline"
	| "skipped"
	| "unavailable";

export interface FunctionDiffResult {
	program: string;
	classification: FunctionDiffClassification;
	comparison?: ComparisonDocument;
	changedFunctions?: number;
	increasedFunctions?: number;
	headTotalCu?: number;
	detail?: string;
}

export interface FunctionDiffReport {
	schemaVersion: number;
	summary: {
		trackedPrograms: number;
		compared: number;
		withChanges: number;
		unchanged: number;
		newBaselines: number;
		skipped: number;
		unavailable: number;
	};
	programs: FunctionDiffResult[];
}

export interface CompareOutcome {
	status: number;
	stdout: string;
	stderr: string;
}

/** Runs `pina profile compare <baseline> <artifact> --json`; injectable for tests. */
export type CompareRunner = (
	baselinePath: string,
	artifactPath: string,
) => CompareOutcome;

function spawnCompare(
	baselinePath: string,
	artifactPath: string,
): CompareOutcome {
	const result = spawnSync(
		"cargo",
		[
			"run",
			"--quiet",
			"--locked",
			"-p",
			"pina_cli",
			"--",
			"profile",
			"compare",
			baselinePath,
			artifactPath,
			"--json",
		],
		{
			encoding: "utf8",
			stdio: ["ignore", "pipe", "pipe"],
			maxBuffer: 64 * 1024 * 1024,
		},
	);
	if (result.error !== undefined) {
		return {
			status: result.status ?? 1,
			stdout: result.stdout ?? "",
			stderr: String(result.error),
		};
	}
	return {
		status: result.status ?? 1,
		stdout: result.stdout ?? "",
		stderr: result.stderr ?? "",
	};
}

interface ManifestState {
	tracked: boolean;
	ok: boolean;
	detail: string;
}

function manifestState(
	manifest: ProfileManifest,
	program: string,
): ManifestState {
	const result = manifest.results?.[program];
	return {
		tracked: result !== undefined,
		ok: result?.status === OK_STATUS,
		detail: result?.detail ?? "no manifest entry",
	};
}

export interface FunctionDiffPlan {
	program: string;
	classification: "compare" | "new-baseline" | "skipped";
	detail?: string;
}

/** Decide how each tracked program can be diffed, in deterministic name order. */
export function planDiffs(
	baseManifest: ProfileManifest,
	headManifest: ProfileManifest,
): FunctionDiffPlan[] {
	const names = new Set([
		...(baseManifest.trackedPrograms ?? []),
		...(headManifest.trackedPrograms ?? []),
		...Object.keys(baseManifest.results ?? {}),
		...Object.keys(headManifest.results ?? {}),
	]);
	for (const program of names) {
		if (!VALID_PROGRAM_NAME.test(program)) {
			throw new Error(
				`invalid program name in profile manifest: ${JSON.stringify(program)}`,
			);
		}
	}
	const plans: FunctionDiffPlan[] = [];
	for (const program of [...names].toSorted()) {
		const base = manifestState(baseManifest, program);
		const head = manifestState(headManifest, program);
		if (base.ok && head.ok) {
			plans.push({ program, classification: "compare" });
		} else if (head.ok && !base.tracked) {
			plans.push({ program, classification: "new-baseline" });
		} else if (!head.tracked) {
			plans.push({
				program,
				classification: "skipped",
				detail: "program is absent from the head profile inventory",
			});
		} else if (!head.ok) {
			plans.push({
				program,
				classification: "skipped",
				detail: `head profile unavailable: ${head.detail}`,
			});
		} else {
			plans.push({
				program,
				classification: "skipped",
				detail: `base profile unavailable: ${base.detail}`,
			});
		}
	}
	return plans;
}

function parseComparisonDocument(
	stdout: string,
): ComparisonDocument | undefined {
	let parsed: unknown;
	try {
		parsed = JSON.parse(stdout);
	} catch {
		return undefined;
	}
	if (
		typeof parsed !== "object" || parsed === null ||
		!Array.isArray((parsed as ComparisonDocument).functions) ||
		typeof (parsed as ComparisonDocument).status !== "string"
	) {
		return undefined;
	}
	return parsed as ComparisonDocument;
}

const ansiEscapePattern = /\u001B\[[0-9;?]*[A-Za-z]/g;

function summarizeStderr(stderr: string): string {
	const lines = stderr.split("\n").map((line) =>
		// Strip ANSI styling so details render cleanly in the markdown report.
		line.replaceAll(ansiEscapePattern, "").trim()
	).filter((line) => line.length > 0);
	const tail = lines.slice(-10).join(" | ");
	return tail.length > 0 ? tail.slice(-1000) : "no stderr output";
}

function countFunctions(document: ComparisonDocument): {
	changedFunctions: number;
	increasedFunctions: number;
} {
	let changedFunctions = 0;
	let increasedFunctions = 0;
	for (const function_ of document.functions) {
		if (function_.change === "unchanged") {
			continue;
		}
		changedFunctions += 1;
		if (function_.delta_cu > 0) {
			increasedFunctions += 1;
		}
	}
	return { changedFunctions, increasedFunctions };
}

function readHeadTotalCu(headDir: string, program: string): number | undefined {
	try {
		const parsed: unknown = JSON.parse(
			readFileSync(join(headDir, `${program}.json`), "utf8"),
		);
		const totalCu = (parsed as { total_cu?: unknown }).total_cu;
		return typeof totalCu === "number" ? totalCu : undefined;
	} catch {
		return undefined;
	}
}

export function collectFunctionDiffs(
	baseManifest: ProfileManifest,
	headManifest: ProfileManifest,
	baseDir: string,
	headDir: string,
	runCompare: CompareRunner,
): FunctionDiffReport {
	const programs: FunctionDiffResult[] = [];
	for (const plan of planDiffs(baseManifest, headManifest)) {
		if (plan.classification === "skipped") {
			programs.push({
				program: plan.program,
				classification: "skipped",
				detail: plan.detail,
			});
			continue;
		}
		if (plan.classification === "new-baseline") {
			programs.push({
				program: plan.program,
				classification: "new-baseline",
				headTotalCu: readHeadTotalCu(headDir, plan.program),
			});
			continue;
		}
		const outcome = runCompare(
			join(baseDir, `${plan.program}.json`),
			join(headDir, `${plan.program}.so`),
		);
		const document = parseComparisonDocument(outcome.stdout);
		if (document === undefined) {
			programs.push({
				program: plan.program,
				classification: "unavailable",
				detail: `pina profile compare exited with status ${outcome.status}: ${
					summarizeStderr(outcome.stderr)
				}`,
			});
			continue;
		}
		if (
			document.baseline_program_name !== document.current_program_name
		) {
			// A name mismatch means the pairing is wrong (different programs);
			// rendering it as a diff would be all-noise additions and removals.
			programs.push({
				program: plan.program,
				classification: "unavailable",
				detail: `baseline program ${
					JSON.stringify(
						document.baseline_program_name,
					)
				} differs from current program ${
					JSON.stringify(
						document.current_program_name,
					)
				}`,
			});
			continue;
		}
		programs.push({
			program: plan.program,
			classification: "compared",
			comparison: document,
			...countFunctions(document),
		});
	}

	const compared = programs.filter(
		(program) => program.classification === "compared",
	);
	return {
		schemaVersion: 1,
		summary: {
			trackedPrograms: programs.length,
			compared: compared.length,
			withChanges: compared.filter(
				(program) => (program.changedFunctions ?? 0) > 0,
			).length,
			unchanged: compared.filter(
				(program) => (program.changedFunctions ?? 0) === 0,
			).length,
			newBaselines: programs.filter(
				(program) => program.classification === "new-baseline",
			).length,
			skipped:
				programs.filter((program) => program.classification === "skipped")
					.length,
			unavailable: programs.filter(
				(program) => program.classification === "unavailable",
			).length,
		},
		programs,
	};
}

function formatInt(value: number): string {
	return new Intl.NumberFormat("en-US", { maximumFractionDigits: 0 }).format(
		value,
	);
}

function formatSignedInt(value: number): string {
	return value < 0 ? `-${formatInt(-value)}` : `+${formatInt(value)}`;
}

function formatSignedPercent(value: number): string {
	return `${value < 0 ? "-" : "+"}${Math.abs(value).toFixed(1)}%`;
}

function escapeCell(value: string): string {
	return value.replaceAll("|", "\\|");
}

function directionArrow(delta: number): string {
	return delta > 0 ? "↑" : delta < 0 ? "↓" : "·";
}

function functionStatusLabel(delta: FunctionDelta): string {
	if (delta.change === "added") {
		return "🆕 added";
	}
	if (delta.change === "removed") {
		return "removed";
	}
	return delta.delta_cu > 0 ? "⚠️ increased" : "decreased";
}

function functionChangeDetail(delta: FunctionDelta): string {
	if (delta.change === "added") {
		return "new";
	}
	if (delta.change === "removed") {
		return "removed";
	}
	return formatSignedPercent(delta.delta_percent);
}

function functionRow(delta: FunctionDelta): string {
	const name = `\`${escapeCell(delta.name)}\``;
	const baseCu = delta.baseline_cu === null
		? "—"
		: formatInt(delta.baseline_cu);
	const headCu = delta.current_cu === null ? "—" : formatInt(delta.current_cu);
	const change = `${directionArrow(delta.delta_cu)} ${
		formatSignedInt(delta.delta_cu)
	} (${functionChangeDetail(delta)})`;
	return `| ${name} | ${baseCu} | ${headCu} | ${change} | ${
		functionStatusLabel(delta)
	} |`;
}

function programSection(result: FunctionDiffResult): string[] {
	const document = result.comparison;
	if (document === undefined) {
		return [];
	}
	const totals = document.totals;
	const totalDelta = totals.delta_cu;
	const thresholdNote = document.exceeds_threshold
		? " — ⚠️ exceeds the local compare threshold"
		: "";
	const lines = [
		`### \`${escapeCell(result.program)}\``,
		"",
		`Program total: ${formatInt(totals.baseline.total_cu)} → ${
			formatInt(totals.current.total_cu)
		} CU (${directionArrow(totalDelta)} ${formatSignedInt(totalDelta)}, ${
			formatSignedPercent(totals.delta_percent)
		})${thresholdNote}.`,
		"",
		"| Function | Base CU | Head CU | CU change | Status |",
		"| -------- | ------: | ------: | --------: | ------ |",
		...document.functions
			.filter((function_) => function_.change !== "unchanged")
			.map(functionRow),
		"",
	];
	return lines;
}

function summaryLine(report: FunctionDiffReport): string {
	const { summary } = report;
	const parts = [
		`Compared ${summary.compared} of ${summary.trackedPrograms} tracked programs at function level (static SBF estimates)`,
		`🔢 ${summary.withChanges} with function changes`,
		`➖ ${summary.unchanged} unchanged`,
	];
	if (summary.newBaselines > 0) {
		parts.push(`🆕 ${summary.newBaselines} new baseline(s)`);
	}
	if (summary.skipped > 0) {
		parts.push(`⏭️ ${summary.skipped} skipped`);
	}
	if (summary.unavailable > 0) {
		parts.push(`⚠️ ${summary.unavailable} diff unavailable`);
	}
	return parts.join(" · ");
}

export function renderFunctionDiffMarkdown(report: FunctionDiffReport): string {
	const compared = report.programs.filter(
		(program) => program.classification === "compared",
	);
	const withChanges = compared.filter(
		(program) => (program.changedFunctions ?? 0) > 0,
	);
	const unchanged = compared.filter(
		(program) => (program.changedFunctions ?? 0) === 0,
	);
	const newBaselines = report.programs.filter(
		(program) => program.classification === "new-baseline",
	);
	const skipped = report.programs.filter(
		(program) => program.classification === "skipped",
	);
	const unavailable = report.programs.filter(
		(program) => program.classification === "unavailable",
	);

	const lines: string[] = [
		"## Program function-level profile diffs",
		"",
		summaryLine(report),
		"",
	];

	for (const program of withChanges) {
		lines.push(...programSection(program));
	}

	if (newBaselines.length > 0) {
		lines.push(
			`New baselines (no base profile): ${
				newBaselines.map((program) =>
					program.headTotalCu === undefined
						? `\`${escapeCell(program.program)}\``
						: `\`${escapeCell(program.program)}\` (${
							formatInt(program.headTotalCu)
						} CU)`
				).join(", ")
			}.`,
			"",
		);
	}
	if (skipped.length > 0) {
		lines.push(
			"Skipped programs:",
			...skipped.map((program) =>
				`- \`${escapeCell(program.program)}\` — ${
					program.detail ?? "unavailable"
				}`
			),
			"",
		);
	}
	if (unavailable.length > 0) {
		lines.push(
			"Function diffs unavailable:",
			...unavailable.map((program) =>
				`- \`${escapeCell(program.program)}\` — ${
					program.detail ?? "unavailable"
				}`
			),
			"",
		);
	}
	if (unchanged.length > 0) {
		lines.push(
			`Unchanged: ${
				unchanged.map((program) => `\`${escapeCell(program.program)}\``).join(
					", ",
				)
			}.`,
			"",
		);
	}
	if (report.programs.length === 0) {
		lines.push("No tracked programs produced profiles to diff.", "");
	}

	lines.push(
		"This section is diagnostic: enforcement remains the instruction compute-unit policy gate.",
		"",
	);
	return lines.join("\n");
}

export interface FunctionDiffRunOptions {
	baseDir: string;
	headDir: string;
	outputDir: string;
	runCompare?: CompareRunner;
}

function loadManifest(directory: string): ProfileManifest {
	let parsed: unknown;
	try {
		parsed = JSON.parse(
			readFileSync(join(directory, "manifest.json"), "utf8"),
		) as unknown;
	} catch (error: unknown) {
		throw new Error(
			`failed to read profile manifest in ${directory}: ${
				error instanceof Error ? error.message : String(error)
			}`,
		);
	}
	if (typeof parsed !== "object" || parsed === null) {
		throw new Error(`profile manifest in ${directory} is not a JSON object`);
	}
	return parsed as ProfileManifest;
}

export function run(options: FunctionDiffRunOptions): number {
	const baseDir = resolve(options.baseDir);
	const headDir = resolve(options.headDir);
	const outputDir = resolve(options.outputDir);
	const baseManifest = loadManifest(baseDir);
	const headManifest = loadManifest(headDir);
	mkdirSync(outputDir, { recursive: true });

	const report = collectFunctionDiffs(
		baseManifest,
		headManifest,
		baseDir,
		headDir,
		options.runCompare ?? spawnCompare,
	);

	writeFileSync(
		join(outputDir, "function-diff.md"),
		renderFunctionDiffMarkdown(report),
		"utf8",
	);
	writeFileSync(
		join(outputDir, "function-diff.json"),
		`${JSON.stringify(report, null, 2)}\n`,
		"utf8",
	);
	process.stdout.write(
		`Wrote ${join(outputDir, "function-diff.md")} and function-diff.json\n`,
	);
	return 0;
}

function main(): number {
	const [baseDir, headDir, outputDir] = process.argv.slice(2);
	if (
		baseDir === undefined || headDir === undefined || outputDir === undefined ||
		process.argv.length !== 5
	) {
		process.stderr.write(
			"Usage: profile-example-diffs.ts <base-reports-dir> <head-reports-dir> <output-dir>\n",
		);
		return 1;
	}
	return run({ baseDir, headDir, outputDir });
}

if (
	process.argv[1] !== undefined &&
	import.meta.url === pathToFileURL(process.argv[1]).href
) {
	try {
		process.exitCode = main();
	} catch (error: unknown) {
		process.stderr.write(
			`Error: ${error instanceof Error ? error.message : String(error)}\n`,
		);
		process.exitCode = 1;
	}
}
