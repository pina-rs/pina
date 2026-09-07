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

const OK_STATUS = "ok";

interface Policy {
	trackedPrograms: string[];
}

interface CargoDependency {
	name: string;
	features: string[];
}

interface CargoPackage {
	name: string;
	dependencies: CargoDependency[];
}

interface CargoMetadata {
	packages: CargoPackage[];
}

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
	program: string,
): string | undefined {
	for (
		const candidate of [
			join(workspaceRoot, "target", "deploy", `${program}.so`),
			join(
				workspaceRoot,
				"target",
				"sbpf-solana-solana",
				"release",
				`${program}.so`,
			),
			join(
				workspaceRoot,
				"target",
				"bpfel-unknown-none",
				"release",
				`${program}.so`,
			),
		]
	) {
		if (existsSync(candidate)) {
			return candidate;
		}
	}
	return undefined;
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

	const scriptDirectory = dirname(fileURLToPath(import.meta.url));
	const workspaceRoot = realpathSync(workspaceArgument);
	const outputDirectory = resolve(outputArgument);
	const policyFile = policyArgument ??
		join(scriptDirectory, "compute-unit-policy.json");
	const bpfToolchain = process.env.PINA_BPF_TOOLCHAIN;
	if (bpfToolchain === undefined || bpfToolchain.length === 0) {
		throw new Error("PINA_BPF_TOOLCHAIN must be set (devenv sets it)");
	}
	const policy: Policy = JSON.parse(readFileSync(policyFile, "utf8")) as Policy;
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

	const metadataResult = command(
		"cargo",
		["metadata", "--format-version", "1", "--no-deps", "--locked"],
		{ cwd: workspaceRoot, capture: true },
	);
	if (metadataResult.status !== 0) {
		return metadataResult.status;
	}
	const metadata = JSON.parse(metadataResult.stdout) as CargoMetadata;
	const packages = new Map(metadata.packages.map((item) => [item.name, item]));
	const buildGroups = new Map<string, string[]>();
	for (const program of policy.trackedPrograms) {
		const pinaDependency = packages
			.get(program)
			?.dependencies.find((dependency) => dependency.name === "pina");
		const groupKey = pinaDependency?.features.toSorted().join(",") ?? program;
		buildGroups.set(groupKey, [...(buildGroups.get(groupKey) ?? []), program]);
	}

	const results: Record<string, ProfileResult> = {};
	for (const programs of buildGroups.values()) {
		for (const program of programs) {
			rmSync(join(outputDirectory, `${program}.json`), { force: true });
			rmSync(join(outputDirectory, `${program}.so`), { force: true });
			rmSync(join(workspaceRoot, "target", "deploy", `${program}.so`), {
				force: true,
			});
		}
		process.stdout.write(
			`Building ${programs.join(" ")} with cargo +${bpfToolchain} build-bpf\n`,
		);
		const build = command(
			"cargo",
			[
				`+${bpfToolchain}`,
				"build-bpf",
				"--locked",
				...programs.flatMap((program) => ["-p", program]),
			],
			{ cwd: workspaceRoot },
		);
		if (build.status !== 0) {
			for (const program of programs) {
				results[program] = {
					status: "unavailable",
					detail: `SBF build failed with status ${build.status}`,
				};
				process.stderr.write(
					`warning: failed to build tracked program ${program} for CU profiling\n`,
				);
			}
			continue;
		}

		for (const program of programs) {
			const artifact = resolveArtifact(workspaceRoot, program);
			if (artifact === undefined) {
				results[program] = {
					status: "unavailable",
					detail: `built ELF was not found under ${
						join(workspaceRoot, "target")
					}`,
				};
				continue;
			}
			copyFileSync(artifact, join(outputDirectory, `${program}.so`));
			process.stdout.write(`Profiling ${program} from ${artifact}\n`);
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
					join(outputDirectory, `${program}.json`),
				],
				{ cwd: workspaceRoot },
			);
			if (profile.status !== 0) {
				rmSync(join(outputDirectory, `${program}.json`), { force: true });
				results[program] = {
					status: "unavailable",
					detail: `static profiler failed with status ${profile.status}`,
				};
				continue;
			}
			results[program] = {
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
					trackedPrograms: policy.trackedPrograms,
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
