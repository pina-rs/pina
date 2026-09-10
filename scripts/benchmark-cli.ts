#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";

const WARMUP_RUNS = 1;
const BENCHMARK_RUNS = 6;
const COMMANDS = [
	{ id: "version", args: ["--version"] },
	{ id: "root help", args: ["--help"] },
	{ id: "profile help", args: ["profile", "--help"] },
	{ id: "IDL help", args: ["idl", "--help"] },
	{ id: "client generation help", args: ["generate", "--help"] },
	{
		id: "cli-rust generation help",
		args: ["generate", "--client", "cli-rust", "--help"],
	},
	{ id: "render bundled docs", args: ["docs", "pina-overview"] },
	{
		id: "cli-rust project generation",
		args: [
			"generate",
			"--client",
			"cli-rust",
			"--project",
			"examples/counter_program",
			"--output",
			"target/benchmark/clients/cli-rust",
		],
	},
] as const;

interface HyperfineResult {
	command: string;
	mean: number;
	stddev: number;
}

interface HyperfineReport {
	results: HyperfineResult[];
}

interface Arguments {
	baseBin: string;
	headBin: string;
	markdownOutput: string;
	jsonOutput: string;
}

type PerformanceStatus =
	| "improved"
	| "new-baseline"
	| "regressed"
	| "within-noise";

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
			case "--base-bin":
				parsed.baseBin = resolve(value);
				break;
			case "--head-bin":
				parsed.headBin = resolve(value);
				break;
			case "--markdown-output":
				parsed.markdownOutput = resolve(value);
				break;
			case "--json-output":
				parsed.jsonOutput = resolve(value);
				break;
			default:
				throw new Error(`unknown option: ${option}`);
		}
	}

	for (
		const key of [
			"baseBin",
			"headBin",
			"markdownOutput",
			"jsonOutput",
		] as const
	) {
		if (parsed[key] === undefined) {
			throw new Error(`missing required option for ${key}`);
		}
	}

	return parsed as Arguments;
}

function shellQuote(value: string): string {
	const escaped = value.replaceAll("'", "'\"'\"'");

	return `'${escaped}'`;
}

function performancePercent(baseSeconds: number, headSeconds: number): number {
	return ((baseSeconds - headSeconds) / baseSeconds) * 100;
}

function classify(percent: number): PerformanceStatus {
	if (percent >= 3) {
		return "improved";
	}

	if (percent <= -3) {
		return "regressed";
	}

	return "within-noise";
}

function statusLabel(status: PerformanceStatus): string {
	return {
		improved: "🚀 improved",
		"new-baseline": "🆕 new baseline",
		regressed: "⚠️ regressed",
		"within-noise": "➖ within noise",
	}[status];
}

function commandSucceeds(binary: string, args: readonly string[]): boolean {
	const result = spawnSync(binary, args, { stdio: "ignore" });

	if (result.error !== undefined) {
		throw result.error;
	}

	return result.status === 0;
}

function formatCount(count: number, singular: string): string {
	return `${count} ${singular}${count === 1 ? "" : "s"}`;
}

function run(arguments_: Arguments): void {
	const commands: string[] = [];
	const baseSupported = new Map<string, boolean>();

	for (const command of COMMANDS) {
		const supportsBase = commandSucceeds(arguments_.baseBin, command.args);
		baseSupported.set(command.id, supportsBase);

		if (!commandSucceeds(arguments_.headBin, command.args)) {
			throw new Error(
				`head CLI command failed before benchmarking: pina ${
					command.args.join(" ")
				}`,
			);
		}

		if (supportsBase) {
			const baseInvocation = [arguments_.baseBin, ...command.args]
				.map(shellQuote)
				.join(" ");
			commands.push(
				"--command-name",
				`base/${command.id}`,
				baseInvocation,
			);
		}

		const headInvocation = [arguments_.headBin, ...command.args]
			.map(shellQuote)
			.join(" ");
		commands.push("--command-name", `head/${command.id}`, headInvocation);
	}

	mkdirSync(dirname(arguments_.jsonOutput), { recursive: true });
	const hyperfineArguments = [
		"--warmup",
		String(WARMUP_RUNS),
		"--runs",
		String(BENCHMARK_RUNS),
		"--style",
		"basic",
		"--shell=none",
		"--export-json",
		arguments_.jsonOutput,
		...commands,
	];
	const hyperfineBinary = process.env.PINA_HYPERFINE_BIN;
	const hyperfine = hyperfineBinary === undefined
		? spawnSync("cargo", ["hyperfine", ...hyperfineArguments], {
			stdio: "inherit",
		})
		: spawnSync(hyperfineBinary, hyperfineArguments, { stdio: "inherit" });

	if (hyperfine.error !== undefined) {
		throw hyperfine.error;
	}

	if (hyperfine.status !== 0) {
		throw new Error(`hyperfine failed with status ${hyperfine.status ?? 1}`);
	}

	const report = JSON.parse(
		readFileSync(arguments_.jsonOutput, "utf8"),
	) as HyperfineReport;
	const results = new Map(report.results.map((item) => [item.command, item]));
	const rows: string[] = [];
	const statuses: PerformanceStatus[] = [];

	for (const command of COMMANDS) {
		const base = results.get(`base/${command.id}`);
		const head = results.get(`head/${command.id}`);

		if (head === undefined) {
			throw new Error(`hyperfine omitted ${command.id}`);
		}

		if (baseSupported.get(command.id) === false) {
			statuses.push("new-baseline");
			rows.push(
				`| \`pina ${command.args.join(" ")}\` | n/a | ${
					(head.mean * 1_000).toFixed(2)
				} ms | n/a | ${statusLabel("new-baseline")} |`,
			);
			continue;
		}

		if (base === undefined) {
			throw new Error(`hyperfine omitted base/${command.id}`);
		}

		const percent = performancePercent(base.mean, head.mean);
		const comparisonStatus = classify(percent);
		statuses.push(comparisonStatus);
		rows.push(
			`| \`pina ${command.args.join(" ")}\` | ${
				(base.mean * 1_000).toFixed(2)
			} ms | ${(head.mean * 1_000).toFixed(2)} ms | ${percent >= 0 ? "+" : ""}${
				percent.toFixed(1)
			}% | ${statusLabel(comparisonStatus)} |`,
		);
	}

	const regressions = statuses.filter((item) => item === "regressed").length;
	const improvements = statuses.filter((item) => item === "improved").length;
	const withinNoise = statuses.filter((item) => item === "within-noise").length;
	const newBaselines =
		statuses.filter((item) => item === "new-baseline").length;
	const summary = [
		regressions === 0
			? "✅ No advisory regressions"
			: `⚠️ ${formatCount(regressions, "advisory regression")}`,
		`🚀 ${formatCount(improvements, "improvement")}`,
		`➖ ${withinNoise} within noise`,
		...(newBaselines > 0
			? [`🆕 ${formatCount(newBaselines, "new baseline")}`]
			: []),
	].join(" · ");
	const lines = [
		"## CLI performance",
		"",
		summary,
		"",
		"<details>",
		"<summary>View CLI benchmark details</summary>",
		"",
		`Measured with hyperfine --shell=none --warmup ${WARMUP_RUNS} --runs ${BENCHMARK_RUNS}. Positive change means the pull request is faster.`,
		"",
		"| Command | Base | Head | Performance change | Status |",
		"| ------- | ---: | ---: | -----------------: | ------ |",
		...rows,
		"",
		"CLI timing changes are advisory because hosted-runner timing is noisy.",
		"",
		"</details>",
		"",
	];
	mkdirSync(dirname(arguments_.markdownOutput), { recursive: true });
	writeFileSync(arguments_.markdownOutput, lines.join("\n"), "utf8");
}

try {
	run(parseArguments(process.argv.slice(2)));
} catch (error: unknown) {
	process.stderr.write(
		`Error: ${error instanceof Error ? error.message : String(error)}\n`,
	);
	process.exitCode = 1;
}
