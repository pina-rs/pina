#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { mkdirSync, realpathSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

const SAMPLE_RUNS = 7;
const BENCHMARK_PATTERN =
	/\[BENCH\] (.+): total=(\d+(?:\.\d+)?)(ns|µs|ms|s), avg=\d+ns \((\d+) iterations\)/u;

interface Arguments {
	baseWorkspace: string;
	headWorkspace: string;
	outputDirectory: string;
	targetDirectory: string;
}

interface Comparison {
	id: string;
	baseNanoseconds?: number;
	headNanoseconds?: number;
	baseSamples?: number[];
	headSamples?: number[];
	performancePercent?: number;
}

type PerformanceStatus =
	| "improved"
	| "new-baseline"
	| "regressed"
	| "removed"
	| "within-noise";

function parseArguments(values: string[]): Arguments {
	if (values.length !== 4) {
		throw new Error(
			"Usage: benchmark-core.ts <base-workspace> <head-workspace> <output-directory> <target-directory>",
		);
	}

	return {
		baseWorkspace: realpathSync(values[0] ?? "."),
		headWorkspace: realpathSync(values[1] ?? "."),
		outputDirectory: resolve(values[2] ?? "."),
		targetDirectory: resolve(values[3] ?? "."),
	};
}

function buildBenchmark(workspace: string, target: string): string {
	const result = spawnSync(
		"cargo",
		[
			"test",
			"--release",
			"--locked",
			"-p",
			"pina",
			"--test",
			"benchmarks",
			"--no-run",
			"--message-format=json",
		],
		{
			cwd: workspace,
			env: { ...process.env, CARGO_TARGET_DIR: target },
			encoding: "utf8",
			stdio: ["ignore", "pipe", "inherit"],
			maxBuffer: 32 * 1024 * 1024,
		},
	);

	if (result.error !== undefined) {
		throw result.error;
	}

	if (result.status !== 0) {
		throw new Error(`failed to build core benchmark in ${workspace}`);
	}

	for (const line of (result.stdout ?? "").split(/\r?\n/u)) {
		if (line.length === 0) {
			continue;
		}

		const message = JSON.parse(line) as {
			reason?: string;
			executable?: string;
			target?: { name?: string };
		};

		if (
			message.reason === "compiler-artifact" &&
			message.target?.name === "benchmarks" && message.executable !== undefined
		) {
			return message.executable;
		}
	}

	throw new Error(
		`Cargo did not report a benchmark executable for ${workspace}`,
	);
}

function median(values: number[]): number {
	const sorted = values.toSorted((left, right) => left - right);
	return sorted[Math.floor(sorted.length / 2)] ?? 0;
}

function measure(executable: string, samples: Map<string, number[]>): void {
	const result = spawnSync(
		executable,
		["--nocapture", "--test-threads=1"],
		{ encoding: "utf8" },
	);

	if (result.error !== undefined) {
		throw result.error;
	}

	if (result.status !== 0) {
		process.stderr.write(result.stderr ?? "");
		throw new Error(
			`core benchmark failed with status ${result.status ?? 1}`,
		);
	}

	const output = `${result.stdout ?? ""}\n${result.stderr ?? ""}`;

	for (const line of output.split(/\r?\n/u)) {
		const match = BENCHMARK_PATTERN.exec(line);

		if (match === null) {
			if (line.includes("[BENCH]")) {
				throw new Error(`invalid benchmark output: ${line}`);
			}

			continue;
		}

		const id = match[1] ?? "unknown";
		const scale = { ns: 1, µs: 1_000, ms: 1_000_000, s: 1_000_000_000 };
		const unit = match[3];
		if (unit !== "ns" && unit !== "µs" && unit !== "ms" && unit !== "s") {
			throw new Error(`unknown benchmark duration unit: ${unit}`);
		}

		const iterations = Number(match[4]);
		const nanoseconds = Number(match[2]) * scale[unit] / iterations;
		if (
			!Number.isFinite(nanoseconds) || !Number.isSafeInteger(iterations) ||
			iterations <= 0
		) {
			throw new Error(`invalid benchmark duration: ${line}`);
		}

		samples.set(id, [...(samples.get(id) ?? []), nanoseconds]);
	}

	if (samples.size === 0) {
		throw new Error(
			`no [BENCH] measurements captured from ${executable}`,
		);
	}
}

function compare(
	base: Map<string, number[]>,
	head: Map<string, number[]>,
): Comparison[] {
	const ids = new Set([...base.keys(), ...head.keys()]);

	return [...ids].toSorted().map((id) => {
		const baseSamples = base.get(id);
		const headSamples = head.get(id);
		for (const samples of [baseSamples, headSamples]) {
			if (samples !== undefined && samples.length !== SAMPLE_RUNS) {
				throw new Error(
					`expected ${SAMPLE_RUNS} samples for ${id}, got ${samples.length}`,
				);
			}
		}

		const baseNanoseconds = baseSamples === undefined
			? undefined
			: median(baseSamples);
		const headNanoseconds = headSamples === undefined
			? undefined
			: median(headSamples);
		const performancePercent = baseNanoseconds === undefined ||
				headNanoseconds === undefined || baseNanoseconds === 0
			? undefined
			: ((baseNanoseconds - headNanoseconds) / baseNanoseconds) * 100;

		return {
			id,
			baseNanoseconds,
			headNanoseconds,
			baseSamples,
			headSamples,
			performancePercent,
		};
	});
}

function classify(item: Comparison): PerformanceStatus {
	if (item.baseNanoseconds === undefined) {
		return "new-baseline";
	}

	if (item.headNanoseconds === undefined) {
		return "removed";
	}

	const percent = item.performancePercent ?? 0;

	if (percent >= 5) {
		return "improved";
	}

	if (percent <= -5) {
		return "regressed";
	}

	return "within-noise";
}

function statusLabel(status: PerformanceStatus): string {
	return {
		improved: "🚀 improved",
		"new-baseline": "🆕 new baseline",
		regressed: "⚠️ regressed",
		removed: "removed",
		"within-noise": "➖ within noise",
	}[status];
}

function formatCount(count: number, singular: string): string {
	return `${count} ${singular}${count === 1 ? "" : "s"}`;
}

function formatNanoseconds(value: number | undefined): string {
	return value === undefined
		? "n/a"
		: new Intl.NumberFormat("en-US").format(value);
}

function run(arguments_: Arguments): void {
	mkdirSync(arguments_.outputDirectory, { recursive: true });
	const baseExecutable = buildBenchmark(
		arguments_.baseWorkspace,
		join(arguments_.targetDirectory, "base"),
	);
	const headExecutable = buildBenchmark(
		arguments_.headWorkspace,
		join(arguments_.targetDirectory, "head"),
	);
	const baseMeasurements = new Map<string, number[]>();
	const headMeasurements = new Map<string, number[]>();
	const executions = [
		{ executable: baseExecutable, samples: baseMeasurements },
		{ executable: headExecutable, samples: headMeasurements },
	];
	for (let index = 0; index < SAMPLE_RUNS; index += 1) {
		for (const run of index % 2 === 0 ? executions : executions.toReversed()) {
			measure(run.executable, run.samples);
		}
	}

	const comparisons = compare(baseMeasurements, headMeasurements);
	const statuses = comparisons.map(classify);
	const regressions = statuses.filter((item) => item === "regressed").length;
	const improvements = statuses.filter((item) => item === "improved").length;
	const withinNoise = statuses.filter((item) => item === "within-noise").length;
	const newBaselines =
		statuses.filter((item) => item === "new-baseline").length;
	const removed = statuses.filter((item) => item === "removed").length;
	const summary = [
		regressions === 0
			? "✅ No advisory regressions"
			: `⚠️ ${formatCount(regressions, "advisory regression")}`,
		`🚀 ${formatCount(improvements, "improvement")}`,
		`➖ ${withinNoise} within noise`,
		...(newBaselines > 0
			? [`🆕 ${formatCount(newBaselines, "new baseline")}`]
			: []),
		...(removed > 0 ? [`🗂️ ${formatCount(removed, "removed benchmark")}`] : []),
	].join(" · ");
	const lines = [
		"## Core host performance",
		"",
		summary,
		"",
		"<details>",
		"<summary>View core benchmark details</summary>",
		"",
		`Median of ${SAMPLE_RUNS} interleaved runs, calculated from total duration without truncating to integer nanoseconds. Raw samples are retained in the JSON artifact. Positive change means the pull request is faster.`,
		"",
		"| Operation | Base | Head | Performance change | Status |",
		"| --------- | ---: | ---: | -----------------: | ------ |",
		...comparisons.map((item, index) => {
			const percent = item.performancePercent;
			const formattedPercent = percent === undefined
				? "n/a"
				: `${percent >= 0 ? "+" : ""}${percent.toFixed(1)}%`;
			const comparisonStatus = statuses[index] ?? "within-noise";

			return `| \`${item.id}\` | ${
				formatNanoseconds(item.baseNanoseconds)
			} ns | ${
				formatNanoseconds(item.headNanoseconds)
			} ns | ${formattedPercent} | ${statusLabel(comparisonStatus)} |`;
		}),
		"",
		"Core timings are advisory. Compute-unit measurements remain the on-chain regression gate.",
		"",
		"</details>",
		"",
	];
	writeFileSync(
		join(arguments_.outputDirectory, "core.md"),
		lines.join("\n"),
		"utf8",
	);
	writeFileSync(
		join(arguments_.outputDirectory, "core.json"),
		`${JSON.stringify({ sampleRuns: SAMPLE_RUNS, comparisons }, null, 2)}\n`,
		"utf8",
	);
}

try {
	run(parseArguments(process.argv.slice(2)));
} catch (error: unknown) {
	process.stderr.write(
		`Error: ${error instanceof Error ? error.message : String(error)}\n`,
	);
	process.exitCode = 1;
}
