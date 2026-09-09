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
import { join, resolve } from "node:path";

import {
	DEFAULT_COMPUTE_UNIT_POLICY,
	type ExampleProgram,
	loadExampleInventory,
} from "./example-inventory.ts";

const OK_STATUS = "ok";

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

function resolveArtifact(
	workspaceRoot: string,
	artifactName: string,
): string | undefined {
	for (const candidate of artifactCandidates(workspaceRoot, artifactName)) {
		if (existsSync(candidate)) {
			return candidate;
		}
	}

	return undefined;
}

function artifactSearchRoots(workspaceRoot: string): string[] {
	return [
		...new Set([
			process.env.CARGO_TARGET_DIR,
			join(workspaceRoot, "target"),
		]),
	].filter((value): value is string => value !== undefined);
}

function artifactCandidates(
	workspaceRoot: string,
	artifactName: string,
): string[] {
	return artifactSearchRoots(workspaceRoot).flatMap((targetRoot) => [
		join(targetRoot, "deploy", `${artifactName}.so`),
		join(
			targetRoot,
			"sbpf-solana-solana",
			"release",
			`${artifactName}.so`,
		),
		join(
			targetRoot,
			"bpfel-unknown-none",
			"release",
			`${artifactName}.so`,
		),
	]);
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
	const bpfToolchain = process.env.PINA_BPF_TOOLCHAIN;
	if (bpfToolchain === undefined || bpfToolchain.length === 0) {
		throw new Error("PINA_BPF_TOOLCHAIN must be set (devenv sets it)");
	}
	mkdirSync(outputDirectory, { recursive: true });

	const toolchains = command("rustup", ["toolchain", "list"], {
		capture: true,
	});
	if (
		!toolchains.stdout.split("\n").some((line) => line.startsWith(bpfToolchain))
	) {
		const installed = command("rustup", [
			"toolchain",
			"install",
			bpfToolchain,
			"--profile",
			"minimal",
			"--component",
			"rust-src",
		]);
		if (installed.status !== 0) {
			return installed.status;
		}
	} else {
		const component = command("rustup", [
			"component",
			"add",
			"rust-src",
			"--toolchain",
			bpfToolchain,
		]);
		if (component.status !== 0) {
			return component.status;
		}
	}

	const { programs: trackedPrograms } = loadExampleInventory(workspaceRoot, {
		policyFile,
	});
	const buildGroups = new Map<string, ExampleProgram[]>();
	for (const program of trackedPrograms) {
		if (!/^[A-Za-z0-9_-]+$/u.test(program.name)) {
			throw new Error(`invalid tracked Cargo package name: ${program.name}`);
		}
		const pinaDependency = program.package.dependencies.find(
			(dependency) => dependency.name === "pina",
		);
		const groupKey = pinaDependency?.features.toSorted().join(",") ??
			program.name;
		buildGroups.set(groupKey, [...(buildGroups.get(groupKey) ?? []), program]);
	}

	const results: Record<string, ProfileResult> = {};
	for (const programs of buildGroups.values()) {
		for (const program of programs) {
			rmSync(join(outputDirectory, `${program.name}.json`), { force: true });
			rmSync(join(outputDirectory, `${program.name}.so`), { force: true });

			for (
				const artifact of artifactCandidates(
					workspaceRoot,
					program.artifactName,
				)
			) {
				rmSync(artifact, { force: true });
			}
		}
		process.stdout.write(
			`Building ${
				programs.map((program) => program.name).join(" ")
			} with cargo +${bpfToolchain} build-bpf\n`,
		);
		const build = command(
			"cargo",
			[
				`+${bpfToolchain}`,
				"build-bpf",
				"--locked",
				...programs.flatMap((program) => ["-p", program.name]),
			],
			{ cwd: workspaceRoot },
		);
		if (build.status !== 0) {
			for (const program of programs) {
				results[program.name] = {
					status: "unavailable",
					detail: `SBF build failed with status ${build.status}`,
				};
				process.stderr.write(
					`warning: failed to build tracked program ${program.name} for CU profiling\n`,
				);
			}
			continue;
		}

		for (const program of programs) {
			const artifact = resolveArtifact(workspaceRoot, program.artifactName);
			if (artifact === undefined) {
				results[program.name] = {
					status: "unavailable",
					detail: `built ELF ${program.artifactName}.so was not found under ${
						artifactSearchRoots(workspaceRoot).join(", ")
					}`,
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
	}

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
					toolchain: bpfToolchain,
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
