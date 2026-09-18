#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
	copyFileSync,
	existsSync,
	mkdirSync,
	readFileSync,
	realpathSync,
	rmSync,
	writeFileSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import {
	DEFAULT_COMPUTE_UNIT_POLICY,
	loadExampleInventory,
} from "./example-inventory.ts";

const OK_STATUS = "ok";
const SCRIPT_DIRECTORY = dirname(fileURLToPath(import.meta.url));

interface ProfileResult {
	status: "ok" | "unavailable";
	detail: string;
}

function command(
	program: string,
	args: string[],
	options: { cwd?: string; capture?: boolean } = {},
): { status: number; stdout: string } {
	const result = spawnSync(program, args, {
		cwd: options.cwd,
		encoding: "utf8",
		stdio: options.capture ? ["ignore", "pipe", "inherit"] : "inherit",
		maxBuffer: 16 * 1024 * 1024,
	});
	if (result.error !== undefined) {
		throw result.error;
	}
	return { status: result.status ?? 1, stdout: result.stdout ?? "" };
}

function sha256(path: string): string {
	return createHash("sha256").update(readFileSync(path)).digest("hex");
}

function main(): number {
	const [workspaceArgument, outputArgument, policyArgument] = process.argv
		.slice(2);
	if (
		workspaceArgument === undefined || outputArgument === undefined ||
		process.argv.length > 5
	) {
		process.stderr.write(
			"Usage: profile-tracked-examples.ts <workspace-root> <output-dir> [policy-file]\n",
		);
		return 1;
	}

	const workspaceRoot = realpathSync(workspaceArgument);
	const outputDirectory = resolve(outputArgument);
	const policyFile = policyArgument ?? DEFAULT_COMPUTE_UNIT_POLICY;
	mkdirSync(outputDirectory, { recursive: true });

	const { programs: trackedPrograms } = loadExampleInventory(workspaceRoot, {
		policyFile,
	});
	const buildDirectory = join(outputDirectory, ".sbf-build");
	rmSync(buildDirectory, { force: true, recursive: true });
	const build = command(process.execPath, [
		join(SCRIPT_DIRECTORY, "build-runtime-compute-units.ts"),
		"--examples-only",
		workspaceRoot,
		buildDirectory,
	]);
	if (build.status !== 0) {
		return build.status;
	}

	const results: Record<string, ProfileResult> = {};
	for (const program of trackedPrograms) {
		rmSync(join(outputDirectory, `${program.name}.json`), { force: true });
		rmSync(join(outputDirectory, `${program.name}.so`), { force: true });
		const artifact = join(buildDirectory, `${program.name}.so`);
		if (!existsSync(artifact)) {
			results[program.name] = {
				status: "unavailable",
				detail:
					`built ELF ${program.name}.so was not found under ${buildDirectory}`,
			};
			continue;
		}
		copyFileSync(artifact, join(outputDirectory, `${program.name}.so`));
		process.stdout.write(`Profiling ${program.name} from ${artifact}\n`);
		const profile = command(
			"cargo",
			[
				"run",
				"--quiet",
				"--locked",
				"-p",
				"pina_cli",
				"--",
				"profile",
				artifact,
				"--json",
				"--output",
				join(outputDirectory, `${program.name}.json`),
			],
			{ cwd: workspaceRoot },
		);
		if (profile.status !== 0) {
			rmSync(join(outputDirectory, `${program.name}.json`), { force: true });
			results[program.name] = {
				status: "unavailable",
				detail: `static profiler failed with status ${profile.status}`,
			};
			continue;
		}
		results[program.name] = {
			status: OK_STATUS,
			detail: "profile and ELF available",
		};
	}
	rmSync(buildDirectory, { force: true, recursive: true });

	const revision = command("git", ["rev-parse", "HEAD"], {
		cwd: workspaceRoot,
		capture: true,
	});
	const lockFile = join(workspaceRoot, "Cargo.lock");
	writeFileSync(
		join(outputDirectory, "manifest.json"),
		`${
			JSON.stringify(
				{
					toolchain: "cargo-build-sbf v1.54",
					sourceRevision: revision.stdout.trim(),
					cargoLockSha256: sha256(lockFile),
					trackedPrograms: trackedPrograms.map((program) => program.name),
					results,
				},
				null,
				2,
			)
		}\n`,
		"utf8",
	);
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
