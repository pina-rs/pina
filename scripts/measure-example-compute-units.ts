#!/usr/bin/env node

import { spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
	existsSync,
	mkdirSync,
	readFileSync,
	realpathSync,
	rmSync,
	writeFileSync,
} from "node:fs";
import { basename, dirname, join, relative, resolve } from "node:path";
import { pathToFileURL } from "node:url";

import {
	type CargoMetadata,
	type ExampleProgram,
	loadExampleInventory,
} from "./example-inventory.ts";

// A batch that outlives this budget is terminated as a failure instead of
// stalling the workflow: Surfpool test binaries can hang indefinitely when the
// simulated runtime wedges, and an unbounded batch once consumed the entire
// instruction compute-unit job budget.
const BATCH_TIMEOUT_MINUTES = 30;

// How long a batch gets to honor SIGTERM before the watchdog escalates to
// SIGKILL, which no process can ignore.
const KILL_GRACE_MS = 5_000;

interface IdlInstruction {
	name: string;
	discriminators: Array<{
		constant?: {
			value?: {
				number?: number;
			};
		};
	}>;
}

interface CodamaIdl {
	program?: {
		instructions?: IdlInstruction[];
		publicKey?: string;
	};
}

interface RecordedSample {
	program: string;
	discriminator: number;
	computeUnits: number;
}

interface MeasuredProgram extends ExampleProgram {
	testPackage: string;
}

function command(
	program: string,
	args: string[],
	options: {
		cwd: string;
		env?: NodeJS.ProcessEnv;
		capture?: boolean;
	},
): { status: number; stdout: string } {
	const result = spawnSync(program, args, {
		cwd: options.cwd,
		env: options.env ?? process.env,
		encoding: "utf8",
		stdio: options.capture ? ["ignore", "pipe", "inherit"] : "inherit",
		maxBuffer: 32 * 1024 * 1024,
	});

	if (result.error !== undefined) {
		throw result.error;
	}

	return { status: result.status ?? 1, stdout: result.stdout ?? "" };
}

/**
 * Run a batch command, streaming its output while keeping a copy of stdout.
 *
 * libtest prints each binary's `test result:` line to stdout, and
 * `classifyBatchExit` reads those lines to tell a teardown crash apart from a
 * failed test. Stderr is inherited untouched.
 */
export function commandAsync(
	program: string,
	args: string[],
	options: {
		cwd: string;
		env: NodeJS.ProcessEnv;
		timeoutMinutes?: number;
	},
): Promise<{ status: number; stdout: string }> {
	return new Promise((resolve, reject) => {
		const child = spawn(program, args, {
			cwd: options.cwd,
			env: options.env,
			// Own process group so the watchdog can terminate the whole batch,
			// including the cargo test binaries and their Surfpool runtimes.
			detached: process.platform !== "win32",
			stdio: ["inherit", "pipe", "inherit"],
		});
		const stdoutChunks: Buffer[] = [];

		child.stdout.on("data", (chunk: Buffer) => {
			stdoutChunks.push(chunk);
			process.stdout.write(chunk);
		});

		const terminate = (signal: NodeJS.Signals) => {
			if (child.pid === undefined) {
				return;
			}

			if (process.platform === "win32") {
				child.kill(signal);
				return;
			}

			try {
				process.kill(-child.pid, signal);
			} catch {
				child.kill(signal);
			}
		};

		const timers: Array<NodeJS.Timeout> = [];
		const clearWatchdog = () => {
			for (const timer of timers) {
				clearTimeout(timer);
			}
		};

		if (options.timeoutMinutes !== undefined) {
			timers.push(
				setTimeout(() => {
					process.stdout.write(
						`Batch exceeded ${options.timeoutMinutes} minutes; terminating ${program}\n`,
					);
					terminate("SIGTERM");

					timers.push(
						setTimeout(() => {
							process.stdout.write(
								`Batch ignored SIGTERM for ${
									KILL_GRACE_MS / 1000
								}s; sending SIGKILL\n`,
							);
							terminate("SIGKILL");
						}, KILL_GRACE_MS),
					);
				}, options.timeoutMinutes * 60 * 1000),
			);
		}

		child.once("error", (error) => {
			clearWatchdog();
			reject(error);
		});
		child.once("close", (code) => {
			clearWatchdog();
			resolve({
				status: code ?? 1,
				stdout: Buffer.concat(stdoutChunks).toString("utf8"),
			});
		});
	});
}

/** How a finished Surfpool batch counts toward the measurement gate. */
export type BatchOutcome = "passed" | "incompleteTeardown" | "failed";

/** What a finished Surfpool batch left behind. */
export interface BatchEvidence {
	/** Exit status of the `cargo test` process that ran the batch. */
	status: number;
	/** The batch's stdout, where libtest prints each `test result:` line. */
	stdout: string;
	/** Test binaries the batch runs: one `--lib` target per test package. */
	testBinaries: number;
	/** Every `program/instruction` case the batch's programs declare. */
	ownedCases: readonly string[];
	/** Recorded sample count per case, across every batch. */
	sampleCounts: ReadonlyMap<string, number>;
}

// libtest's per-binary summary, e.g.
// `test result: ok. 5 passed; 0 failed; 0 ignored; ...`.
const TEST_RESULT_LINE = /^test result: (ok|FAILED)\./gmu;

// libtest only colors a terminal, but strip escapes so a forced `--color`
// cannot hide a summary line.
const ANSI_ESCAPE = /\u001b\[[0-9;]*m/gu;

/**
 * Decide whether a batch's exit status fails the measurement.
 *
 * A nonzero exit is tolerated as `incompleteTeardown` only when the process
 * died after its work was provably done: every test binary printed
 * `test result: ok`, none printed `FAILED`, and every case the batch owns has
 * at least one recorded sample. That is a crash or hang while the Surfpool
 * runtime shut down, not a missing or bad measurement. Anything short of that
 * (a failed test, a binary that died before its summary, a build error, a
 * case with no samples) still fails.
 */
export function classifyBatchExit(evidence: BatchEvidence): BatchOutcome {
	if (evidence.status === 0) {
		return "passed";
	}

	const results = [
		...evidence.stdout.replace(ANSI_ESCAPE, "").matchAll(TEST_RESULT_LINE),
	].map((match) => match[1]);
	const everyBinaryPassed = evidence.testBinaries > 0 &&
		results.length === evidence.testBinaries &&
		results.every((result) => result === "ok");
	const everyCaseMeasured = evidence.ownedCases.length > 0 &&
		evidence.ownedCases.every((id) => (evidence.sampleCounts.get(id) ?? 0) > 0);

	return everyBinaryPassed && everyCaseMeasured
		? "incompleteTeardown"
		: "failed";
}

function attachTestPackages(
	metadata: CargoMetadata,
	workspace: string,
	programs: ExampleProgram[],
	unavailablePrograms: Set<string>,
): MeasuredProgram[] {
	const packagesByManifest = new Map(
		metadata.packages.map((package_) => [
			relative(workspace, package_.manifest_path),
			package_,
		]),
	);
	const measuredPrograms: MeasuredProgram[] = [];

	for (const program of programs) {
		const testManifest = join(
			"examples",
			program.directory,
			"tests",
			"surfpool",
			"Cargo.toml",
		);
		const testPackage = packagesByManifest.get(testManifest);

		if (testPackage === undefined) {
			unavailablePrograms.add(program.name);
			continue;
		}

		measuredPrograms.push({
			...program,
			testPackage: testPackage.name,
		});
	}

	return measuredPrograms;
}

function instructionNames(
	workspace: string,
	program: MeasuredProgram,
): Map<number, string> {
	const path = join(
		workspace,
		"codama",
		"idls",
		`${program.directory}.json`,
	);
	const idl = JSON.parse(readFileSync(path, "utf8")) as CodamaIdl;
	const instructions = idl.program?.instructions;

	if (instructions === undefined) {
		throw new Error(`Codama IDL has no instructions: ${path}`);
	}

	const names = new Map<number, string>();

	for (const instruction of instructions) {
		const discriminator = instruction.discriminators[0]?.constant?.value
			?.number;

		if (discriminator === undefined) {
			throw new Error(
				`${program.name}/${instruction.name} has no numeric discriminator`,
			);
		}

		if (names.has(discriminator)) {
			throw new Error(
				`${program.name} reuses instruction discriminator ${discriminator}`,
			);
		}

		names.set(discriminator, instruction.name);
	}

	return names;
}

function programPublicKey(
	workspace: string,
	program: MeasuredProgram,
): string {
	const path = join(workspace, "codama", "idls", `${program.directory}.json`);
	const idl = JSON.parse(readFileSync(path, "utf8")) as CodamaIdl;
	const publicKey = idl.program?.publicKey;

	if (publicKey === undefined) {
		throw new Error(`Codama IDL has no program public key: ${path}`);
	}

	return publicKey;
}

function isMissingFile(error: unknown): boolean {
	return (error as NodeJS.ErrnoException).code === "ENOENT";
}

function readSamples(path: string): RecordedSample[] {
	if (!existsSync(path)) {
		return [];
	}

	return readFileSync(path, "utf8")
		.split(/\r?\n/u)
		.filter((line) => line.length > 0)
		.map((line) => JSON.parse(line) as RecordedSample);
}

function revision(workspace: string): string {
	const result = command("git", ["rev-parse", "HEAD"], {
		cwd: workspace,
		capture: true,
	});

	if (result.status !== 0) {
		throw new Error(`git rev-parse failed in ${workspace}`);
	}

	return result.stdout.trim();
}

function sha256(path: string): string {
	return createHash("sha256").update(readFileSync(path)).digest("hex");
}

async function main(): Promise<number> {
	const values = process.argv.slice(2);
	const allowIncompleteIndex = values.indexOf("--allow-incomplete");
	const allowIncomplete = allowIncompleteIndex !== -1;

	if (allowIncomplete) {
		values.splice(allowIncompleteIndex, 1);
	}

	const [harnessArgument, sourceArgument, elfArgument, outputArgument] = values;

	if (
		harnessArgument === undefined || sourceArgument === undefined ||
		elfArgument === undefined || outputArgument === undefined ||
		values.length !== 4
	) {
		process.stderr.write(
			"Usage: measure-example-compute-units.ts <harness-workspace> <source-workspace> <elf-dir> <output-file> [--allow-incomplete]\n",
		);
		return 1;
	}

	const harnessWorkspace = realpathSync(harnessArgument);
	const sourceWorkspace = realpathSync(sourceArgument);
	const elfDirectory = resolve(elfArgument);
	const outputFile = resolve(outputArgument);
	const inventory = loadExampleInventory(harnessWorkspace);
	const unavailablePrograms = new Set<string>();
	const programs = attachTestPackages(
		inventory.metadata,
		harnessWorkspace,
		inventory.programs,
		unavailablePrograms,
	);
	const instructionNamesByProgram = new Map<string, Map<number, string>>();
	mkdirSync(dirname(outputFile), { recursive: true });

	for (const program of programs) {
		try {
			instructionNamesByProgram.set(
				program.name,
				instructionNames(sourceWorkspace, program),
			);
		} catch (error: unknown) {
			if (!isMissingFile(error)) {
				throw error;
			}

			instructionNamesByProgram.set(program.name, new Map<number, string>());

			if (sourceWorkspace === harnessWorkspace) {
				unavailablePrograms.add(program.name);
			}
		}
	}

	const batches: Array<{
		manifest: Record<string, { artifact: string; program: string }>;
		programs: MeasuredProgram[];
	}> = [];

	for (const program of programs) {
		if (unavailablePrograms.has(program.name)) {
			continue;
		}

		const artifact = join(elfDirectory, `${program.name}.so`);

		if (!existsSync(artifact)) {
			unavailablePrograms.add(program.name);
			continue;
		}

		let publicKey: string;

		try {
			publicKey = programPublicKey(harnessWorkspace, program);
		} catch (error: unknown) {
			if (!isMissingFile(error)) {
				throw error;
			}

			unavailablePrograms.add(program.name);
			continue;
		}

		let batch = batches
			.filter((item) => item.manifest[publicKey] === undefined)
			.toSorted((left, right) =>
				left.programs.length - right.programs.length
			)[0];

		if (batch === undefined) {
			batch = { manifest: {}, programs: [] };
			batches.push(batch);
		}
		batch.programs.push(program);
		batch.manifest[publicKey] = {
			artifact,
			program: program.name,
		};
	}

	// Batches run one at a time. Surfpool's SDK asks the kernel for a free
	// port by binding `127.0.0.1:0`, reading the assigned port, and dropping
	// the listener before the surfnet rebinds it (`surfpool-sdk`'s
	// `get_free_port`). Two batches starting concurrently can therefore be
	// handed the same port, and the loser's client talks to the winner's
	// surfnet: the symptoms are transaction-construction failures like
	// "account has not been marked as writable" in tests that pass on their
	// own, differing from run to run. Serializing removes the race, and the
	// batches have to share one machine's CPU regardless.
	const recordFiles: string[] = [];
	const batchRuns: Array<{
		number: number;
		status: number;
		stdout: string;
		programs: MeasuredProgram[];
	}> = [];

	for (const [batchIndex, batch] of batches.entries()) {
		const benchmarkManifest = join(
			dirname(outputFile),
			`${basename(outputFile)}.batch-${batchIndex + 1}.manifest.json`,
		);
		writeFileSync(
			benchmarkManifest,
			`${JSON.stringify(batch.manifest, null, 2)}\n`,
			"utf8",
		);
		const recordFile = join(
			dirname(outputFile),
			`${basename(outputFile)}.batch-${batchIndex + 1}.jsonl`,
		);
		rmSync(recordFile, { force: true });
		process.stdout.write(
			`Measuring instruction CU batch ${batchIndex + 1}/${batches.length}: ${
				batch.programs.map((program) => program.name).join(", ")
			}\n`,
		);
		const { status, stdout } = await commandAsync(
			"cargo",
			[
				"test",
				"--no-fail-fast",
				"--locked",
				...batch.programs.flatMap((program) => ["-p", program.testPackage]),
				"--lib",
				"--",
				"--ignored",
				"--nocapture",
				"--test-threads=1",
			],
			{
				cwd: harnessWorkspace,
				env: {
					...process.env,
					PINA_CU_MANIFEST: benchmarkManifest,
					PINA_CU_RECORD_FILE: recordFile,
				},
				timeoutMinutes: BATCH_TIMEOUT_MINUTES,
			},
		);

		batchRuns.push({
			number: batchIndex + 1,
			status,
			stdout,
			programs: batch.programs,
		});
		recordFiles.push(recordFile);
	}

	const caseIds = (program: MeasuredProgram): string[] =>
		[...(instructionNamesByProgram.get(program.name) ??
			new Map<number, string>()).values()]
			.map((name) => `${program.name}/${name}`);

	const samples = recordFiles.flatMap(readSamples);
	const samplesByCase = new Map<string, number[]>();

	for (const program of programs) {
		const names = instructionNamesByProgram.get(program.name) ??
			new Map<number, string>();

		for (
			const sample of samples.filter((item) => item.program === program.name)
		) {
			const name = names.get(sample.discriminator);

			if (name === undefined) {
				continue;
			}

			const id = `${program.name}/${name}`;
			samplesByCase.set(id, [
				...(samplesByCase.get(id) ?? []),
				sample.computeUnits,
			]);
		}
	}

	const cases = [...samplesByCase]
		.map(([id, values]) => ({
			id,
			computeUnits: Math.max(...values),
			minimumComputeUnits: Math.min(...values),
			sampleCount: values.length,
		}))
		.toSorted((left, right) => left.id.localeCompare(right.id));
	const measuredCases = new Set(cases.map((item) => item.id));
	const missingCases = programs.flatMap((program) =>
		caseIds(program).filter((id) => !measuredCases.has(id))
	).toSorted();
	const sampleCounts = new Map(
		cases.map((item) => [item.id, item.sampleCount]),
	);
	const testFailures: string[] = [];
	const incompleteTeardowns: string[] = [];

	for (const batchRun of batchRuns) {
		const outcome = classifyBatchExit({
			status: batchRun.status,
			stdout: batchRun.stdout,
			testBinaries: batchRun.programs.length,
			ownedCases: batchRun.programs.flatMap(caseIds),
			sampleCounts,
		});
		const exit =
			`Surfpool batch ${batchRun.number} exited with ${batchRun.status}`;

		if (outcome === "passed") {
			continue;
		}

		if (outcome === "failed") {
			testFailures.push(exit);
			continue;
		}

		const note =
			`${exit} after every test binary passed and every case it owns was measured`;
		process.stdout.write(`Tolerating teardown exit: ${note}\n`);
		incompleteTeardowns.push(note);
	}

	const lockFile = join(sourceWorkspace, "Cargo.lock");
	const report = {
		provenance: {
			sourceRevision: revision(sourceWorkspace),
			harnessRevision: revision(harnessWorkspace),
			cargoLockSha256: sha256(lockFile),
			measurement: "Surfpool transaction simulation maximum",
		},
		cases,
		missingCases,
		testFailures: testFailures.toSorted(),
		incompleteTeardowns: incompleteTeardowns.toSorted(),
		unavailablePrograms: [...unavailablePrograms].toSorted(),
	};
	writeFileSync(outputFile, `${JSON.stringify(report, null, 2)}\n`, "utf8");

	if (
		!allowIncomplete &&
		(missingCases.length > 0 || testFailures.length > 0 ||
			unavailablePrograms.size > 0)
	) {
		return 2;
	}

	return 0;
}

if (
	process.argv[1] !== undefined &&
	import.meta.url === pathToFileURL(process.argv[1]).href
) {
	main().then((status) => {
		process.exitCode = status;
	}).catch((error: unknown) => {
		process.stderr.write(
			`Error: ${error instanceof Error ? error.message : String(error)}\n`,
		);
		process.exitCode = 1;
	});
}
