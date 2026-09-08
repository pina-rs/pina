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
	{ id: "render bundled docs", args: ["docs", "pina-overview"] },
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

function status(percent: number): string {
	if (percent >= 3) {
		return "✅ improved";
	}

	if (percent <= -3) {
		return "⚠️ regressed";
	}

	return "➖ within noise";
}

function run(arguments_: Arguments): void {
	const commands: string[] = [];

	for (const command of COMMANDS) {
		for (
			const [revision, binary] of [
				["base", arguments_.baseBin],
				["head", arguments_.headBin],
			] as const
		) {
			const name = `${revision}/${command.id}`;
			const invocation = [binary, ...command.args].map(shellQuote).join(" ");
			commands.push("--command-name", name, invocation);
		}
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
	const lines = [
		"## CLI performance",
		"",
		`Measured with hyperfine --shell=none --warmup ${WARMUP_RUNS} --runs ${BENCHMARK_RUNS}. Positive change means the pull request is faster.`,
		"",
		"| Command | Base | Head | Performance change | Status |",
		"| ------- | ---: | ---: | -----------------: | ------ |",
	];

	for (const command of COMMANDS) {
		const base = results.get(`base/${command.id}`);
		const head = results.get(`head/${command.id}`);

		if (base === undefined || head === undefined) {
			throw new Error(`hyperfine omitted ${command.id}`);
		}

		const percent = performancePercent(base.mean, head.mean);
		lines.push(
			`| \`pina ${command.args.join(" ")}\` | ${
				(base.mean * 1_000).toFixed(2)
			} ms | ${(head.mean * 1_000).toFixed(2)} ms | ${percent >= 0 ? "+" : ""}${
				percent.toFixed(1)
			}% | ${status(percent)} |`,
		);
	}

	lines.push(
		"",
		"CLI timing changes are advisory because hosted-runner timing is noisy.",
		"",
	);
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
