#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { mkdirSync, realpathSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

const SAMPLE_RUNS = 7;
const BENCHMARK_PATTERN = /\[BENCH\] (.+): .*avg=(\d+)ns/u;

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

function measure(executable: string): Map<string, number> {
	const samples = new Map<string, number[]>();

	for (let index = 0; index < SAMPLE_RUNS; index += 1) {
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
				continue;
			}

			const id = match[1] ?? "unknown";
			const nanoseconds = Number.parseInt(match[2] ?? "0", 10);
			samples.set(id, [...(samples.get(id) ?? []), nanoseconds]);
		}
	}

	if (samples.size === 0) {
		throw new Error(
			`no [BENCH] measurements captured from ${executable}`,
		);
	}

	return new Map(
		[...samples].map(([id, values]) => [id, median(values)]),
	);
}

function compare(
	base: Map<string, number>,
	head: Map<string, number>,
): Comparison[] {
	const ids = new Set([...base.keys(), ...head.keys()]);

	return [...ids].toSorted().map((id) => {
		const baseNanoseconds = base.get(id);
		const headNanoseconds = head.get(id);
		const performancePercent = baseNanoseconds === undefined ||
				headNanoseconds === undefined || baseNanoseconds === 0
			? undefined
			: ((baseNanoseconds - headNanoseconds) / baseNanoseconds) * 100;

		return { id, baseNanoseconds, headNanoseconds, performancePercent };
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
		arguments_.targetDirectory,
	);
	const baseMeasurements = measure(baseExecutable);
	const headExecutable = buildBenchmark(
		arguments_.headWorkspace,
		arguments_.targetDirectory,
	);
	const comparisons = compare(baseMeasurements, measure(headExecutable));
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
		`Median of ${SAMPLE_RUNS} runs. Positive change means the pull request is faster.`,
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
