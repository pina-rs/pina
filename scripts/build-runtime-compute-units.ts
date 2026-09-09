#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import {
	copyFileSync,
	cpSync,
	existsSync,
	lstatSync,
	mkdirSync,
	readdirSync,
	readFileSync,
	readlinkSync,
	realpathSync,
	rmSync,
	unlinkSync,
	writeFileSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import {
	type ExampleProgram,
	loadExampleInventory,
} from "./example-inventory.ts";
import { findExecutable } from "./find-executable.ts";

const TOOLS_VERSION = "v1.54";
const SCRIPT_DIRECTORY = dirname(fileURLToPath(import.meta.url));
const TOKEN_LOADER_FIXTURE = join(
	SCRIPT_DIRECTORY,
	"..",
	"tests",
	"fixtures",
	"token_loader_cu_program",
);

interface CommandOptions {
	cwd?: string;
	env: NodeJS.ProcessEnv;
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

	return executable;
}

function linuxToolsArgs(installTools: boolean): string[] {
	return [
		installTools ? "--force-tools-install" : "--skip-tools-install",
		"--tools-version",
		TOOLS_VERSION,
	];
}

function buildProgram(
	executable: string,
	workspace: string,
	output: string,
	program: ExampleProgram,
	env: NodeJS.ProcessEnv,
	linux: boolean,
	installTools: boolean,
): number {
	const features = ["bpf-entrypoint"];
	const args = [
		...(linux ? linuxToolsArgs(installTools) : []),
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

function buildTokenLoaderProgram(
	executable: string,
	workspace: string,
	output: string,
	env: NodeJS.ProcessEnv,
	linux: boolean,
	installTools: boolean,
): number {
	const generated = join(output, ".token-loader-cu-program");
	rmSync(generated, { force: true, recursive: true });
	mkdirSync(join(generated, "src"), { recursive: true });
	cpSync(
		join(TOKEN_LOADER_FIXTURE, "src", "lib.rs"),
		join(generated, "src", "lib.rs"),
	);

	const manifest = readFileSync(
		join(TOKEN_LOADER_FIXTURE, "Cargo.template.toml"),
		"utf8",
	).replace(
		"__PINA_PATH__",
		JSON.stringify(join(workspace, "crates", "pina")),
	);
	writeFileSync(join(generated, "Cargo.toml"), manifest);

	const args = [
		...(linux ? linuxToolsArgs(installTools) : []),
		"--manifest-path",
		join(generated, "Cargo.toml"),
		"--sbf-out-dir",
		output,
		"--features",
		"bpf-entrypoint",
	];

	return linux
		? command(executable, args, { cwd: workspace, env })
		: command("cargo", ["build-sbf", ...args], { cwd: workspace, env });
}

function main(): number {
	const values = process.argv.slice(2);
	const tokenLoaderOnlyIndex = values.indexOf("--token-loader-only");
	const examplesOnlyIndex = values.indexOf("--examples-only");
	const tokenLoaderOnly = tokenLoaderOnlyIndex !== -1;
	const examplesOnly = examplesOnlyIndex !== -1;

	if (tokenLoaderOnly) {
		values.splice(tokenLoaderOnlyIndex, 1);
	}
	if (examplesOnly) {
		values.splice(values.indexOf("--examples-only"), 1);
	}
	if (tokenLoaderOnly && examplesOnly) {
		process.stderr.write(
			"--token-loader-only and --examples-only cannot be used together\n",
		);
		return 1;
	}
	if (values.length === 0 || values.length % 2 !== 0) {
		process.stderr.write(
			"Usage: build-runtime-compute-units.ts [--examples-only | --token-loader-only] <workspace-root> <output-dir> [<workspace-root> <output-dir>...]\n",
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
	let installTools = linux;

	for (let index = 0; index < values.length; index += 2) {
		const workspace = realpathSync(values[index] ?? ".");
		const output = resolve(values[index + 1] ?? ".");
		mkdirSync(output, { recursive: true });
		const programs = tokenLoaderOnly
			? []
			: loadExampleInventory(workspace, { env }).programs;

		for (const program of programs) {
			const artifact = join(output, `${program.name}.so`);
			const cargoArtifact = join(output, `${program.artifactName}.so`);
			rmSync(artifact, { force: true });
			rmSync(cargoArtifact, { force: true });
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
				installTools,
			);
			if (status !== 0) {
				return status;
			}
			installTools = false;
			if (!existsSync(cargoArtifact)) {
				const outputFiles = readdirSync(output).toSorted().join(", ");
				throw new Error(
					`cargo-build-sbf did not produce ${cargoArtifact}; output contains: ${
						outputFiles.length === 0 ? "nothing" : outputFiles
					}`,
				);
			}

			if (cargoArtifact !== artifact) {
				copyFileSync(cargoArtifact, artifact);
			}
		}

		if (examplesOnly) {
			continue;
		}

		const tokenLoaderArtifact = join(output, "token_loader_cu_program.so");
		rmSync(tokenLoaderArtifact, { force: true });
		process.stdout.write(
			`Building runtime CU ELF for token_loader_cu_program at ${workspace}\n`,
		);
		const tokenLoaderStatus = buildTokenLoaderProgram(
			executable,
			workspace,
			output,
			env,
			linux,
			installTools,
		);

		if (tokenLoaderStatus !== 0) {
			return tokenLoaderStatus;
		}
		installTools = false;

		if (!existsSync(tokenLoaderArtifact)) {
			const outputFiles = readdirSync(output).toSorted().join(", ");
			throw new Error(
				`cargo-build-sbf did not produce ${tokenLoaderArtifact}; output contains: ${
					outputFiles.length === 0 ? "nothing" : outputFiles
				}`,
			);
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
