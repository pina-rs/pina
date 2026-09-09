#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import {
	cpSync,
	existsSync,
	lstatSync,
	mkdirSync,
	readFileSync,
	readlinkSync,
	realpathSync,
	renameSync,
	rmSync,
	unlinkSync,
	writeFileSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { findExecutable } from "./find-executable.ts";

const TOOLS_VERSION = "v1.54";
const PROGRAMS = [
	"account_realloc_program",
	"counter_program",
	"profile_program",
	"token_loader_cu_program",
] as const;

const SCRIPT_DIRECTORY = dirname(fileURLToPath(import.meta.url));
const TOKEN_LOADER_FIXTURE = join(
	SCRIPT_DIRECTORY,
	"..",
	"tests",
	"fixtures",
	"token_loader_cu_program",
);

// Baseline ELF builds may run against an older revision where a tracked
// example was renamed. The alias map records the historical package name so
// the base workspace can still be built; artifacts are always emitted under
// the canonical (head) program name.
const BASELINE_ALIASES: Record<string, string> = (() => {
	const policyPath = join(
		dirname(fileURLToPath(import.meta.url)),
		"compute-unit-policy.json",
	);
	const policy = JSON.parse(readFileSync(policyPath, "utf8")) as {
		baselineProgramAliases?: Record<string, string>;
	};
	return policy.baselineProgramAliases ?? {};
})();

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
	program: (typeof PROGRAMS)[number],
	env: NodeJS.ProcessEnv,
	linux: boolean,
): number {
	if (program === "token_loader_cu_program") {
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
			...(linux
				? ["--skip-tools-install", "--tools-version", TOOLS_VERSION]
				: []),
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

	// Resolve the example directory for this workspace, falling back to the
	// historical (aliased) name when the revision predates a rename.
	const manifestDirectory = existsSync(join(workspace, "examples", program))
		? program
		: BASELINE_ALIASES[program] ?? program;
	const args = [
		...(linux
			? ["--skip-tools-install", "--tools-version", TOOLS_VERSION]
			: []),
		"--manifest-path",
		join(workspace, "examples", manifestDirectory, "Cargo.toml"),
		"--sbf-out-dir",
		output,
		"--features",
		"bpf-entrypoint",
		"--",
		"--locked",
	];
	const status = linux
		? command(executable, args, { cwd: workspace, env })
		: command("cargo", ["build-sbf", ...args], { cwd: workspace, env });
	if (status !== 0) {
		return status;
	}
	if (manifestDirectory !== program) {
		const built = join(output, `${manifestDirectory}.so`);
		const canonical = join(output, `${program}.so`);
		rmSync(canonical, { force: true });
		renameSync(built, canonical);
	}
	return 0;
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
		for (const program of PROGRAMS) {
			const artifact = join(output, `${program}.so`);
			rmSync(artifact, { force: true });
			process.stdout.write(
				`Building runtime CU ELF for ${program} at ${workspace}\n`,
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
