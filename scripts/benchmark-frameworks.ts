#!/usr/bin/env node

// Builds the same two programs with four Solana frameworks and records what
// each one costs on-chain: deployed size in bytes, and the compute units the
// instruction actually consumes inside a Mollusk VM.
//
// The programs live in `benchmarks/framework-comparison/programs`. Each is a
// standalone crate so the foreign framework revisions it pins cannot leak into
// the published workspace lockfile.
//
// Usage:
//
//   node scripts/benchmark-frameworks.ts <repoRoot> <outputDir> [--update-doc]
//
// `--update-doc` rewrites the generated region of
// `docs/src/framework-comparison.md`, which is the single command that keeps
// the published tables current.

import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
	existsSync,
	mkdirSync,
	readFileSync,
	rmSync,
	writeFileSync,
} from "node:fs";
import { join, relative, resolve } from "node:path";

/// The programs compared, in report order. Both are deliberately tiny: the
/// difference between frameworks is the framework's own dispatch, validation
/// and entrypoint code, not application logic.
const PROGRAMS = ["hello", "counter"];

interface Framework {
	/// Directory name under `programs/<program>/`.
	directory: string;
	/// Name shown in the published tables.
	label: string;
	/// Package name, which is also the SBF artifact stem.
	crateName: string;
	/// Instruction data per instruction, as hex.
	data: Record<string, string>;
	/// Whether `initialize` reads the PDA bump from its instruction data.
	initializeTakesBump?: boolean;
	/// Counter account layout, which the frameworks disagree about.
	counterAccount: AccountLayout;
}

/// Where the counter program keeps the bump and the count.
interface AccountLayout {
	size: number;
	discriminator: string;
	bumpOffset: number;
	countOffset: number;
}

/// Instruction discriminator Anchor derives for a handler: the first eight
/// bytes of `sha256("global:<handler_name>")`.
function anchorDiscriminator(handler: string): string {
	return createHash("sha256").update(`global:${handler}`).digest("hex").slice(
		0,
		16,
	);
}

/// Account discriminator Anchor derives for a type: the first eight bytes of
/// `sha256("account:<TypeName>")`.
function accountDiscriminator(name: string): string {
	return createHash("sha256").update(`account:${name}`).digest("hex").slice(
		0,
		16,
	);
}

/// Pina, Pinocchio and Quasar store the counter as `discriminator, bump, count`.
const BUMP_THEN_COUNT: AccountLayout = {
	size: 10,
	discriminator: "01",
	bumpOffset: 1,
	countOffset: 2,
};

/// The framework list for one program. Pina, Pinocchio and Quasar number their
/// instructions from zero; Anchor hashes the handler name instead.
function frameworksFor(program: string): Framework[] {
	return [
		{
			directory: "pina",
			label: "Pina",
			crateName: `${program}_pina`,
			// Pina and Pinocchio pass the bump in, so they skip the PDA search
			// the other two pay for during `initialize`.
			data: { hello: "00", initialize: "00", increment: "01" },
			initializeTakesBump: true,
			counterAccount: BUMP_THEN_COUNT,
		},
		{
			directory: "pinocchio",
			label: "Pinocchio (hand-written)",
			crateName: `${program}_pinocchio`,
			data: { hello: "", initialize: "00", increment: "01" },
			initializeTakesBump: true,
			counterAccount: BUMP_THEN_COUNT,
		},
		{
			directory: "quasar",
			label: "Quasar",
			crateName: `${program}_quasar`,
			data: { hello: "00", initialize: "00", increment: "01" },
			counterAccount: BUMP_THEN_COUNT,
		},
		{
			directory: "anchor_v2",
			label: "Anchor v2 (`lang-v2`, rc.1)",
			crateName: `${program}_anchor_v2`,
			// Anchor names its handlers instead of numbering them, so every
			// instruction carries an eight-byte `sha256("global:<handler>")`.
			data: {
				hello: anchorDiscriminator("hello"),
				initialize: anchorDiscriminator("initialize"),
				increment: anchorDiscriminator("increment"),
			},
			// Anchor prefixes an eight-byte discriminator and pads the payload
			// out to its alignment, so the account is 24 bytes rather than 10.
			counterAccount: {
				size: 24,
				discriminator: accountDiscriminator("CounterState"),
				bumpOffset: 16,
				countOffset: 8,
			},
		},
	];
}

/// Program ids, matching `declare_id!` in the fixture sources.
const PROGRAM_IDS: Record<string, string> = {
	hello: "DCF5KBmtQ9ryDC7mQezKLwuJHem6coVUCmKkw37M9J4A",
	counter: "GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS",
};

/// Log line each program must emit, proving it ran its own code rather than
/// returning early with a success code.
const EXPECTED_LOGS: Record<string, string> = {
	hello: "Hello, Solana!",
	counter: "Counter incremented",
};

interface MeasuredInstruction {
	name: string;
	compute_units: number;
	ok: boolean;
}

interface Measurement {
	program: string;
	framework: string;
	/// Deployed program size in bytes.
	bytes: number;
	instructions: MeasuredInstruction[];
}

function command(
	file: string,
	args: string[],
	options: { cwd?: string; env?: NodeJS.ProcessEnv } = {},
): { ok: boolean; stdout: string; stderr: string } {
	const result = spawnSync(file, args, {
		cwd: options.cwd,
		env: options.env ? { ...process.env, ...options.env } : process.env,
		encoding: "utf8",
		maxBuffer: 64 * 1024 * 1024,
	});
	return {
		ok: result.status === 0,
		stdout: result.stdout ?? "",
		stderr: result.stderr ?? "",
	};
}

/// Builds the host-side verifier once per run.
function buildVerifier(root: string, outputDir: string): string {
	const result = command(
		"cargo",
		[
			"build",
			"--release",
			"--locked",
			"--manifest-path",
			join(root, "benchmarks/framework-comparison/verifier/Cargo.toml"),
			"--target-dir",
			join(outputDir, "verifier-target"),
		],
		{ cwd: root },
	);
	if (!result.ok) {
		fail(`verifier build failed:\n${result.stderr}`);
	}
	return join(outputDir, "verifier-target/release/framework-verifier");
}

/// The production profile every framework is built with, so the comparison
/// measures framework overhead rather than build settings.
const RELEASE_PROFILE = {
	CARGO_PROFILE_RELEASE_LTO: "fat",
	CARGO_PROFILE_RELEASE_CODEGEN_UNITS: "1",
	CARGO_PROFILE_RELEASE_OPT_LEVEL: "3",
	CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS: "false",
};

function buildProgram(
	root: string,
	framework: Framework,
	program: string,
	outputDir: string,
): string {
	const outDir = join(outputDir, "programs", program, framework.directory);
	mkdirSync(outDir, { recursive: true });

	const result = command(
		"cargo",
		[
			"build-sbf",
			"--lto",
			"--manifest-path",
			join(
				root,
				`benchmarks/framework-comparison/programs/${program}/${framework.directory}/Cargo.toml`,
			),
			"--sbf-out-dir",
			outDir,
			// `cargo build-sbf` has no `--locked` of its own; the trailing
			// arguments are forwarded to its inner `cargo build`.
			"--",
			"--locked",
		],
		{ cwd: root, env: RELEASE_PROFILE },
	);
	if (!result.ok) {
		fail(`${program}/${framework.directory} build failed:\n${result.stderr}`);
	}

	const artifact = join(outDir, `${framework.crateName}.so`);
	if (!existsSync(artifact)) {
		fail(
			`${program}/${framework.directory} produced no artifact at ${artifact}`,
		);
	}
	return artifact;
}

function measure(
	verifier: string,
	artifact: string,
	program: string,
	framework: Framework,
): Measurement {
	const args = [
		"--so",
		artifact,
		"--program-id",
		PROGRAM_IDS[program],
		"--case",
		program,
	];
	if (program === "hello") {
		// The counter case is proven by the account state it leaves behind, so
		// the log assertion applies only to the single-instruction program.
		args.push("--expect-log", EXPECTED_LOGS[program]);
		args.push("--hello-data", framework.data.hello);
	} else {
		args.push("--initialize-data", framework.data.initialize);
		args.push("--increment-data", framework.data.increment);
		if (framework.initializeTakesBump) {
			args.push("--initialize-takes-bump");
		}
		const layout = framework.counterAccount;
		args.push("--account-size", String(layout.size));
		args.push("--account-discriminator", layout.discriminator);
		args.push("--bump-offset", String(layout.bumpOffset));
		args.push("--count-offset", String(layout.countOffset));
	}

	const result = command(verifier, args);
	if (!result.ok) {
		fail(
			`${program}/${framework.directory} measurement failed:\n${result.stderr}`,
		);
	}
	const parsed = JSON.parse(result.stdout.trim()) as {
		bytes: number;
		instructions: MeasuredInstruction[];
	};
	return {
		program,
		framework: framework.label,
		bytes: parsed.bytes,
		instructions: parsed.instructions,
	};
}

function fail(message: string): never {
	process.stderr.write(`benchmark-frameworks: ${message}\n`);
	process.exit(1);
}

const INSTRUCTION_NAMES: Record<string, string[]> = {
	hello: ["hello"],
	counter: ["initialize", "increment"],
};

/// Renders the generated tables. Kept in this file so the published numbers
/// and the way they were produced cannot drift apart.
function renderMarkdown(measurements: Measurement[]): string {
	const sections: string[] = [];
	for (const program of PROGRAMS) {
		const rows = measurements.filter((entry) => entry.program === program);
		const floor = rows.find((entry) => entry.framework.startsWith("Pinocchio"));
		const instructions = INSTRUCTION_NAMES[program];

		const header = [
			"| Framework | Size (bytes) | " +
			instructions.map((name) => `\`${name}\` CU`).join(" | ") +
			" | vs Pinocchio size |",
			"| --- | ---: | " + instructions.map(() => "---:").join(" | ") +
			" | ---: |",
		];
		const body = rows.map((entry) => {
			const cu = instructions.map((name) => {
				const instruction = entry.instructions.find((step) =>
					step.name === name
				);
				return instruction ? formatNumber(instruction.compute_units) : "—";
			});
			const ratio = floor ? `${formatDelta(entry.bytes, floor.bytes)}` : "—";
			return `| ${entry.framework} | ${formatNumber(entry.bytes)} | ${
				cu.join(" | ")
			} | ${ratio} |`;
		});

		const title = program === "hello" ? "Hello world" : "Counter";
		sections.push([`### ${title}`, "", ...header, ...body].join("\n"));
	}
	return sections.join("\n\n");
}

function formatNumber(value: number): string {
	return value.toLocaleString("en-US");
}

/// Signed percentage difference from the hand-written floor.
function formatDelta(value: number, floor: number): string {
	const percent = ((value - floor) / floor) * 100;
	const sign = percent >= 0 ? "+" : "−";
	return `${sign}${Math.abs(percent).toFixed(0)}%`;
}

const BEGIN_MARKER = "<!-- BEGIN GENERATED: framework-comparison -->";
const END_MARKER = "<!-- END GENERATED: framework-comparison -->";

function updateDoc(root: string, markdown: string): void {
	const path = join(root, "docs/src/framework-comparison.md");
	if (!existsSync(path)) {
		fail(`generated table target is missing: ${path}`);
	}
	const current = readFileSync(path, "utf8");
	const begin = current.indexOf(BEGIN_MARKER);
	const end = current.indexOf(END_MARKER);
	if (begin === -1 || end === -1 || end < begin) {
		fail(`${relative(root, path)} is missing its generated region markers`);
	}
	const next = current.slice(0, begin + BEGIN_MARKER.length) +
		`\n\n${markdown}\n\n` +
		current.slice(end);
	if (next !== current) {
		writeFileSync(path, next);
	}
}

function main(): void {
	const args = process.argv.slice(2).filter((argument) =>
		argument !== "--update-doc"
	);
	const updateDocFlag = process.argv.includes("--update-doc");
	const [rootArgument, outputArgument] = args;
	if (!rootArgument || !outputArgument) {
		fail(
			"usage: benchmark-frameworks.ts <repoRoot> <outputDir> [--update-doc]",
		);
	}
	const root = resolve(rootArgument);
	const outputDir = resolve(outputArgument);
	// Only the measured artifacts are cleared. The verifier's cargo target
	// directory survives, so a repeat run reuses the dependency build instead
	// of recompiling mollusk-svm and its tree from scratch every time.
	rmSync(join(outputDir, "programs"), { recursive: true, force: true });
	mkdirSync(outputDir, { recursive: true });

	const verifier = buildVerifier(root, outputDir);

	const measurements: Measurement[] = [];
	for (const program of PROGRAMS) {
		for (const framework of frameworksFor(program)) {
			process.stdout.write(`measuring ${program}/${framework.directory}\n`);
			const artifact = buildProgram(root, framework, program, outputDir);
			measurements.push(measure(verifier, artifact, program, framework));
		}
	}

	writeFileSync(
		join(outputDir, "framework-comparison.json"),
		`${JSON.stringify({ measurements }, null, "\t")}\n`,
	);

	const markdown = renderMarkdown(measurements);
	if (updateDocFlag) {
		updateDoc(root, markdown);
	}
	process.stdout.write(`${markdown}\n`);
}

main();
