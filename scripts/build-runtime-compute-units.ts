#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import {
	copyFileSync,
	existsSync,
	lstatSync,
	mkdirSync,
	readlinkSync,
	realpathSync,
	rmSync,
	unlinkSync,
} from "node:fs";
import { dirname, join, relative, resolve, sep } from "node:path";

import { findExecutable } from "./find-executable.ts";

const TOOLS_VERSION = "v1.54";
interface CommandOptions {
	cwd?: string;
	env: NodeJS.ProcessEnv;
}

interface ExampleProgram {
	manifest: string;
	name: string;
}

interface CargoMetadata {
	packages: Array<{
		features: Record<string, string[]>;
		manifest_path: string;
		name: string;
	}>;
}

function discoverPrograms(
	workspace: string,
	env: NodeJS.ProcessEnv,
): ExampleProgram[] {
	const result = spawnSync(
		"cargo",
		["metadata", "--format-version", "1", "--no-deps", "--locked"],
		{
			cwd: workspace,
			env,
			encoding: "utf8",
			stdio: ["ignore", "pipe", "inherit"],
		},
	);

	if (result.error !== undefined) {
		throw result.error;
	}

	if (result.status !== 0) {
		throw new Error(`cargo metadata failed with status ${result.status ?? 1}`);
	}

	const metadata = JSON.parse(result.stdout ?? "") as CargoMetadata;

	return metadata.packages
		.filter((package_) => {
			const parts = relative(workspace, package_.manifest_path).split(sep);

			return parts.length === 3 && parts[0] === "examples" &&
				parts[2] === "Cargo.toml" &&
				Object.hasOwn(package_.features, "bpf-entrypoint");
		})
		.map((package_) => ({
			manifest: package_.manifest_path,
			name: package_.name,
		}))
		.toSorted((left, right) => left.name.localeCompare(right.name));
}

function command(
	program: string,
	args: string[],
	options: CommandOptions,
): number {
	const result = spawnSync(program, args, {
		cwd: options.cwd,
		env: options.env,
		encoding: "utf8",
		stdio: "inherit",
	});
	if (result.error !== undefined) {
		throw result.error;
	}
	return result.status ?? 1;
}

function findCargoBuildSbf(env: NodeJS.ProcessEnv): string {
	const located = findExecutable("cargo-build-sbf", env);
	if (located === undefined) {
		throw new Error("cargo-build-sbf is not available in PATH");
	}
	return located;
}

function isSymlink(path: string): boolean {
	try {
		return lstatSync(path).isSymbolicLink();
	} catch (error: unknown) {
		if ((error as NodeJS.ErrnoException).code === "ENOENT") {
			return false;
		}
		throw error;
	}
}

function prepareLinuxTools(env: NodeJS.ProcessEnv): string {
	const resolved = realpathSync(findCargoBuildSbf(env));
	const unwrapped = join(dirname(resolved), ".cargo-build-sbf-wrapped");
	const executable = existsSync(unwrapped) ? unwrapped : resolved;
	const home = env.HOME;
	if (home === undefined || home.length === 0) {
		throw new Error("HOME must be set before preparing cargo-build-sbf");
	}

	const cacheHome = env.XDG_CACHE_HOME ?? join(home, ".cache");
	for (
		const link of new Set([
			join(home, ".cache", "solana", TOOLS_VERSION, "platform-tools"),
			join(cacheHome, "solana", TOOLS_VERSION, "platform-tools"),
		])
	) {
		if (!isSymlink(link)) {
			continue;
		}
		const target = readlinkSync(link);
		if (
			target.startsWith("/nix/store/") && target.endsWith("/lib/platform-tools")
		) {
			unlinkSync(link);
			continue;
		}
		throw new Error(
			`refusing to replace unexpected platform-tools link: ${target}`,
		);
	}

	const installed = command(
		executable,
		[
			"--force-tools-install",
			"--install-only",
			"--tools-version",
			TOOLS_VERSION,
		],
		{ env },
	);
	if (installed !== 0) {
		throw new Error(
			`cargo-build-sbf tool installation failed with status ${installed}`,
		);
	}
	return executable;
}

function buildProgram(
	executable: string,
	workspace: string,
	output: string,
	program: ExampleProgram,
	env: NodeJS.ProcessEnv,
	linux: boolean,
): number {
	const features = ["bpf-entrypoint"];
	const args = [
		...(linux
			? ["--skip-tools-install", "--tools-version", TOOLS_VERSION]
			: []),
		"--manifest-path",
		program.manifest,
		"--sbf-out-dir",
		output,
		"--features",
		features.join(","),
		"--",
		"--locked",
	];
	return linux
		? command(executable, args, { cwd: workspace, env })
		: command("cargo", ["build-sbf", ...args], { cwd: workspace, env });
}

function main(): number {
	const values = process.argv.slice(2);
	if (values.length === 0 || values.length % 2 !== 0) {
		process.stderr.write(
			"Usage: build-runtime-compute-units.ts <workspace-root> <output-dir> [<workspace-root> <output-dir>...]\n",
		);
		return 1;
	}

	const firstWorkspace = resolve(values[0] ?? ".");
	const home = process.env.HOME?.length
		? process.env.HOME
		: join(firstWorkspace, ".cache", "home");
	mkdirSync(home, { recursive: true });
	const env: NodeJS.ProcessEnv = { ...process.env, HOME: home };
	const linux = process.platform === "linux";
	const executable = linux ? prepareLinuxTools(env) : findCargoBuildSbf(env);

	for (let index = 0; index < values.length; index += 2) {
		const workspace = realpathSync(values[index] ?? ".");
		const output = resolve(values[index + 1] ?? ".");
		mkdirSync(output, { recursive: true });
		for (const program of discoverPrograms(workspace, env)) {
			const artifact = join(output, `${program.name}.so`);
			const libraryArtifact = join(output, `lib${program.name}.so`);
			rmSync(artifact, { force: true });
			rmSync(libraryArtifact, { force: true });
			process.stdout.write(
				`Building runtime CU ELF for ${program.name} at ${workspace}\n`,
			);
			const status = buildProgram(
				executable,
				workspace,
				output,
				program,
				env,
				linux,
			);
			if (status !== 0) {
				return status;
			}
			if (!existsSync(artifact) && existsSync(libraryArtifact)) {
				copyFileSync(libraryArtifact, artifact);
			}
			if (!existsSync(artifact)) {
				throw new Error(`cargo-build-sbf did not produce ${artifact}`);
			}
		}
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
