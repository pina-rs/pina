#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";

const WARMUP_RUNS = 1;
const SAMPLE_ROUNDS = 8;
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
		// The renderer only rewrites a tree it recognises as its own, and the
		// generated header changes between revisions. CI restores `target/` from
		// cache, so a tree written by an older revision would abort the run.
		clean: "target/benchmark/clients/cli-rust",
	},
] as const;

interface HyperfineResult {
	command: string;
	mean: number;
	stddev: number | null;
	median: number;
	min: number;
	max: number;
	user: number;
	system: number;
	times: number[];
	exit_codes: number[];
}

interface Invocation {
	name: string;
	command: string;
}

interface InvocationPair {
	base: Invocation | undefined;
	head: Invocation;
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
	const result = spawnSync(binary, args, { encoding: "utf8" });

	if (result.error !== undefined) {
		throw result.error;
	}

	if (result.status !== 0) {
		console.error(
			`probe failed: ${binary} ${args.join(" ")}\n${result.stdout ?? ""}\n${
				result.stderr ?? ""
			}`,
		);
	}

	return result.status === 0;
}

/** Read finite, nonnegative timing values from hyperfine's external JSON. */
function timing(value: unknown): number {
	if (typeof value !== "number" || !Number.isFinite(value) || value < 0) {
		throw new Error("hyperfine returned an invalid timing");
	}
	return value;
}

/** Reject failed commands and incomplete single-sample rounds before comparison. */
function parseResults(value: unknown): HyperfineResult[] {
	if (
		value === null || typeof value !== "object" || !("results" in value) ||
		!Array.isArray(value.results)
	) {
		throw new Error("hyperfine omitted its results");
	}
	const rows: unknown[] = value.results;
	return rows.map((row) => {
		if (row === null || typeof row !== "object") {
			throw new Error("hyperfine returned an invalid result");
		}
		const fields = row as Record<string, unknown>;
		if (
			typeof fields.command !== "string" || !Array.isArray(fields.times) ||
			fields.times.length !== 1 || !Array.isArray(fields.exit_codes) ||
			fields.exit_codes.length !== 1 || fields.exit_codes[0] !== 0
		) {
			throw new Error("hyperfine returned failed or incomplete samples");
		}
		const samples: unknown[] = fields.times;
		return {
			command: fields.command,
			mean: timing(fields.mean),
			stddev: fields.stddev === null ? null : timing(fields.stddev),
			median: timing(fields.median),
			min: timing(fields.min),
			max: timing(fields.max),
			user: timing(fields.user),
			system: timing(fields.system),
			times: samples.map(timing),
			exit_codes: [0],
		};
	});
}

/** Sum the same samples in the same order while retaining their original order separately. */
function mean(values: number[]): number {
	return values.toSorted((a, b) => a - b).reduce(
		(sum, value) => sum + value,
		0,
	) /
		values.length;
}

/** Combine all successful rounds; report means without dropping any raw timing. */
function aggregate(name: string, samples: HyperfineResult[]): HyperfineResult {
	if (samples.length !== SAMPLE_ROUNDS) {
		throw new Error(`incomplete rounds for ${name}`);
	}
	const times = samples.flatMap((sample) => sample.times);
	const average = mean(times);
	const sorted = times.toSorted((a, b) => a - b);
	const middle = sorted.length / 2;
	const lower = sorted[middle - 1];
	const upper = sorted[middle];
	if (lower === undefined || upper === undefined) {
		throw new Error(`incomplete median samples for ${name}`);
	}
	return {
		command: name,
		mean: average,
		stddev: Math.sqrt(
			times.reduce((sum, value) => sum + (value - average) ** 2, 0) /
				(times.length - 1),
		),
		median: (lower + upper) / 2,
		min: Math.min(...times),
		max: Math.max(...times),
		user: mean(samples.map((sample) => sample.user)),
		system: mean(samples.map((sample) => sample.system)),
		times,
		exit_codes: samples.flatMap((sample) => sample.exit_codes),
	};
}

function formatCount(count: number, singular: string): string {
	return `${count} ${singular}${count === 1 ? "" : "s"}`;
}

function run(arguments_: Arguments): void {
	rmSync(arguments_.jsonOutput, { force: true });
	rmSync(arguments_.markdownOutput, { force: true });
	const pairs: InvocationPair[] = [];
	const baseSupported = new Map<string, boolean>();
	const cleanPaths = COMMANDS.filter((command) => "clean" in command).map(
		(command) => resolve(command.clean),
	);

	// The renderer refuses to rewrite a generated tree written by a different
	// revision, and base and head alternate on the same output directory. A
	// clean slate before every probe and every timed run keeps each side
	// measuring a cold generation.
	const cleanOutputs = () => {
		for (const path of cleanPaths) {
			rmSync(path, { recursive: true, force: true });
		}
	};
	const prepareArgument = cleanPaths.length === 0
		? []
		: ["--prepare", `rm -rf ${cleanPaths.map(shellQuote).join(" ")}`];

	for (const command of COMMANDS) {
		cleanOutputs();

		const supportsBase = commandSucceeds(arguments_.baseBin, command.args);
		baseSupported.set(command.id, supportsBase);

		cleanOutputs();

		if (!commandSucceeds(arguments_.headBin, command.args)) {
			throw new Error(
				`head CLI command failed before benchmarking: pina ${
					command.args.join(" ")
				}`,
			);
		}

		pairs.push({
			base: supportsBase
				? {
					name: `base/${command.id}`,
					command: [arguments_.baseBin, ...command.args].map(shellQuote).join(
						" ",
					),
				}
				: undefined,
			head: {
				name: `head/${command.id}`,
				command: [arguments_.headBin, ...command.args].map(shellQuote).join(
					" ",
				),
			},
		});
	}

	mkdirSync(dirname(arguments_.jsonOutput), { recursive: true });
	const measurements = new Map<string, HyperfineResult[]>();
	for (const pair of pairs) {
		if (pair.base !== undefined) measurements.set(pair.base.name, []);
		measurements.set(pair.head.name, []);
	}
	const executionOrder: string[] = [];
	const rounds: unknown[] = [];
	const roundOutput = `${arguments_.jsonOutput}.round.json`;
	for (let round = 0; round < SAMPLE_ROUNDS; round++) {
		const baseFirst = round % 2 === 0;
		executionOrder.push(baseFirst ? "base-first" : "head-first");
		const commands = pairs.flatMap((pair) => {
			const ordered = pair.base === undefined
				? [pair.head]
				: baseFirst
				? [pair.base, pair.head]
				: [pair.head, pair.base];
			return ordered.flatMap((invocation) => [
				"--command-name",
				invocation.name,
				invocation.command,
			]);
		});
		const hyperfineArguments = [
			"--warmup",
			String(WARMUP_RUNS),
			"--runs",
			"1",
			"--style",
			"basic",
			"--shell=none",
			...prepareArgument,
			"--export-json",
			roundOutput,
			...commands,
		];
		const hyperfineBinary = process.env.PINA_HYPERFINE_BIN;
		const hyperfine = hyperfineBinary === undefined
			? spawnSync("cargo", ["hyperfine", ...hyperfineArguments], {
				stdio: "inherit",
			})
			: spawnSync(hyperfineBinary, hyperfineArguments, { stdio: "inherit" });
		if (hyperfine.error !== undefined) throw hyperfine.error;
		if (hyperfine.status !== 0) {
			throw new Error(`hyperfine failed with status ${hyperfine.status ?? 1}`);
		}
		const report: unknown = JSON.parse(readFileSync(roundOutput, "utf8"));
		const seen = new Set<string>();
		for (const result of parseResults(report)) {
			const samples = measurements.get(result.command);
			if (samples === undefined || seen.has(result.command)) {
				throw new Error(`unexpected or duplicate command: ${result.command}`);
			}
			seen.add(result.command);
			samples.push(result);
		}
		if (seen.size !== measurements.size) {
			throw new Error("hyperfine omitted commands from a round");
		}
		rounds.push(report);
		rmSync(roundOutput);
	}
	const results = new Map(
		[...measurements].map((
			[name, samples],
		) => [name, aggregate(name, samples)]),
	);
	writeFileSync(
		arguments_.jsonOutput,
		JSON.stringify(
			{
				sampleRuns: SAMPLE_ROUNDS,
				executionOrder,
				rounds,
				results: [...results.values()],
			},
			null,
			2,
		) + "\n",
	);
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
		`Mean of ${SAMPLE_ROUNDS} interleaved rounds with hyperfine --shell=none --warmup ${WARMUP_RUNS} --runs 1 per round. Each side runs first equally often. Raw samples and execution order are retained in JSON. Positive change means the pull request is faster.`,
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
