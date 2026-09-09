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

import {
	type CargoMetadata,
	type ExampleProgram,
	loadExampleInventory,
} from "./example-inventory.ts";

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

function commandAsync(
	program: string,
	args: string[],
	options: {
		cwd: string;
		env: NodeJS.ProcessEnv;
	},
): Promise<number> {
	return new Promise((resolve, reject) => {
		const child = spawn(program, args, {
			cwd: options.cwd,
			env: options.env,
			stdio: "inherit",
		});
		child.once("error", reject);
		child.once("close", (code) => resolve(code ?? 1));
	});
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
	const testFailures: string[] = [];
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

	const recordFiles = await Promise.all(
		batches.map(async (batch, batchIndex) => {
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
			const status = await commandAsync(
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
				},
			);

			if (status !== 0) {
				testFailures.push(
					`Surfpool batch ${batchIndex + 1} exited with ${status}`,
				);
			}

			return recordFile;
		}),
	);

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
		[...(instructionNamesByProgram.get(program.name) ??
			new Map<number, string>()).values()]
			.map((name) => `${program.name}/${name}`)
			.filter((id) => !measuredCases.has(id))
	).toSorted();
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

main().then((status) => {
	process.exitCode = status;
}).catch((error: unknown) => {
	process.stderr.write(
		`Error: ${error instanceof Error ? error.message : String(error)}\n`,
	);
	process.exitCode = 1;
});
