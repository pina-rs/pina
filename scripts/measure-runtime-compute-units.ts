#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync } from "node:fs";
import { dirname, resolve } from "node:path";

function command(
	program: string,
	args: string[],
	cwd: string,
	env: NodeJS.ProcessEnv,
): number {
	const result = spawnSync(program, args, { cwd, env, stdio: "inherit" });
	if (result.error !== undefined) {
		throw result.error;
	}
	return result.status ?? 1;
}

function revision(workspace: string): string {
	const result = spawnSync("git", ["rev-parse", "HEAD"], {
		cwd: workspace,
		encoding: "utf8",
		stdio: ["ignore", "pipe", "inherit"],
	});
	if (result.error !== undefined) {
		throw result.error;
	}
	if (result.status !== 0) {
		throw new Error(`git rev-parse failed in ${workspace}`);
	}
	return (result.stdout ?? "").trim();
}

function main(): number {
	const [harnessArgument, sourceArgument, elfArgument, outputArgument] = process
		.argv.slice(2);
	if (
		harnessArgument === undefined ||
		sourceArgument === undefined ||
		elfArgument === undefined ||
		outputArgument === undefined ||
		process.argv.length !== 6
	) {
		process.stderr.write(
			"Usage: measure-runtime-compute-units.ts <harness-workspace> <source-workspace> <elf-dir> <output-file>\n",
		);
		return 1;
	}

	const harnessWorkspace = resolve(harnessArgument);
	const sourceWorkspace = resolve(sourceArgument);
	const elfDirectory = resolve(elfArgument);
	const outputFile = resolve(outputArgument);
	for (
		const program of [
			"account_realloc_program",
			"counter_program",
			"profile_program",
		]
	) {
		const path = resolve(elfDirectory, `${program}.so`);
		if (!existsSync(path)) {
			throw new Error(`required runtime CU artifact is missing: ${path}`);
		}
	}
	mkdirSync(dirname(outputFile), { recursive: true });

	const env: NodeJS.ProcessEnv = {
		...process.env,
		PINA_CU_ELF_DIR: elfDirectory,
		PINA_CU_OUTPUT: outputFile,
		PINA_CU_SOURCE_LOCK_FILE: resolve(sourceWorkspace, "Cargo.lock"),
		PINA_CU_SOURCE_REVISION: revision(sourceWorkspace),
		PINA_CU_HARNESS_REVISION: revision(harnessWorkspace),
		PINA_CU_TOOLCHAIN: process.env.PINA_BPF_TOOLCHAIN ?? "unknown",
	};
	const status = command(
		"cargo",
		[
			"test",
			"--manifest-path",
			resolve(harnessWorkspace, "Cargo.toml"),
			"--locked",
			"-p",
			"pina_root",
			"--test",
			"compute_units",
			"--",
			"--exact",
			"measure_runtime_compute_units",
			"--nocapture",
		],
		harnessWorkspace,
		env,
	);
	if (status !== 0) {
		return status;
	}
	if (!existsSync(outputFile)) {
		throw new Error(`runtime CU harness did not produce ${outputFile}`);
	}
	return 0;
}

try {
	process.exitCode = main();
} catch (error: unknown) {
	process.stderr.write(
		`Error: ${error instanceof Error ? error.message : String(error)}\n`,
	);
	process.exitCode = 1;
}
