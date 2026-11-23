#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
	accessSync,
	constants as fsConstants,
	existsSync,
	lstatSync,
	mkdirSync,
	mkdtempSync,
	readdirSync,
	readFileSync,
	readlinkSync,
	realpathSync,
	rmSync,
	symlinkSync,
	unlinkSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, isAbsolute, join, relative, resolve, sep } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { findExecutable } from "./find-executable.ts";

const SCRIPT_DIRECTORY = dirname(fileURLToPath(import.meta.url));
export const REPO_ROOT = resolve(SCRIPT_DIRECTORY, "..");

const CARGO_EXPAND_VERSION = "1.0.111";
const SBF_TOOLS_VERSION_DEFAULT = "v1.54";

export const MUTANT_PACKAGES = [
	"pina",
	"pina_cli",
	"pina_codama_renderer",
	"pina_macros",
	"pina_profile",
] as const;

const COVERAGE_PACKAGES = [
	"pina",
	"pina_abi",
	"pina_cli",
	"pina_cli_renderer",
	"pina_codama_renderer",
	"pina_cpi_renderer",
	"pina_macros",
	"pina_lints",
	"pina_profile",
	"pina_root",
	"prop_amm_program",
	"profile_program",
	"profile-program-client",
] as const;

const PROGRAM_E2E_UNIT_PACKAGES = [
	"declare_id_program",
	"declare_program",
	"duplicate_mutable_accounts_program",
	"custom_errors_program",
	"events_program",
	"float_accounts_program",
	"account_realloc_program",
	"compact_accounts_program",
	"system_accounts_program",
	"sysvar_checks_program",
	"escrow_program",
	"optional_accounts_program",
	"heap_alloc_program",
	"pina_bpf_program",
	"profile_program",
	"validation_program",
] as const;

const PROGRAM_E2E_SBF_TARGETS: ReadonlyArray<{
	manifest: string;
	features: string;
	alias: string;
}> = [
	{
		manifest: "examples/pina_bpf_program/Cargo.toml",
		features: "bpf-entrypoint,cpi-runtime-tests",
		alias: "build-pina-bpf-program",
	},
	{
		manifest: "examples/heap_alloc_program/Cargo.toml",
		features: "bpf-entrypoint",
		alias: "build-heap-alloc-program",
	},
	{
		manifest: "examples/escrow_program/Cargo.toml",
		features: "bpf-entrypoint",
		alias: "build-escrow-program",
	},
	{
		manifest: "examples/migrations_program/Cargo.toml",
		features: "bpf-entrypoint",
		alias: "build-migrations-program",
	},
	{
		manifest: "examples/optional_accounts_program/Cargo.toml",
		features: "bpf-entrypoint",
		alias: "build-optional-accounts-program",
	},
	{
		manifest: "examples/profile_program/Cargo.toml",
		features: "bpf-entrypoint",
		alias: "build-profile-program",
	},
	{
		manifest: "examples/role_registry_program/Cargo.toml",
		features: "bpf-entrypoint",
		alias: "build-role-registry-program",
	},
	{
		manifest: "examples/staking_rewards_program/Cargo.toml",
		features: "bpf-entrypoint",
		alias: "build-staking-rewards-program",
	},
	{
		manifest: "examples/vesting_program/Cargo.toml",
		features: "bpf-entrypoint",
		alias: "build-vesting-program",
	},
];

interface RunOptions {
	cwd?: string;
	env?: NodeJS.ProcessEnv;
	capture?: boolean;
	allowFailure?: boolean;
}

function run(
	program: string,
	args: readonly string[],
	options: RunOptions = {},
): { status: number; stdout: string } {
	const result = spawnSync(program, args, {
		cwd: options.cwd ?? REPO_ROOT,
		env: options.env ?? process.env,
		encoding: "utf8",
		stdio: options.capture ? ["ignore", "pipe", "inherit"] : "inherit",
		maxBuffer: 32 * 1024 * 1024,
	});

	if (result.error !== undefined) {
		throw result.error;
	}

	const status = result.status ?? 1;
	if (status !== 0 && !options.allowFailure) {
		throw new Error(
			`Command failed with exit status ${status}: ${program} ${args.join(" ")}`,
		);
	}

	return { status, stdout: result.stdout ?? "" };
}

/** Parse a `name = "..."` package entry from a Cargo.toml manifest. */
export function parseManifestPackageName(manifestText: string): string | null {
	for (const line of manifestText.split(/\r?\n/u)) {
		const match = /^name = "(.*)"$/u.exec(line);
		if (match?.[1] !== undefined && match[1].length > 0) {
			return match[1];
		}
	}
	return null;
}

/** Discover workspace package names for manifests directly under `parentDir/*`. */
export function discoverChildPackageNames(parentDir: string): string[] {
	if (!existsSync(parentDir)) {
		return [];
	}

	const names: string[] = [];
	for (const entry of readdirSync(parentDir, { withFileTypes: true })) {
		if (!entry.isDirectory()) {
			continue;
		}
		const manifestPath = join(parentDir, entry.name, "Cargo.toml");
		if (!existsSync(manifestPath)) {
			continue;
		}
		const name = parseManifestPackageName(readFileSync(manifestPath, "utf8"));
		if (name !== null) {
			names.push(name);
		}
	}

	return names.toSorted();
}

/** Discover package names for secure fixture manifests under `security/<lesson>/secure/Cargo.toml`. */
export function discoverSecureFixturePackageNames(rootDir: string): string[] {
	const securityDir = join(rootDir, "security");
	if (!existsSync(securityDir)) {
		return [];
	}

	const names: string[] = [];
	for (const entry of readdirSync(securityDir, { withFileTypes: true })) {
		if (!entry.isDirectory()) {
			continue;
		}
		const manifestPath = join(securityDir, entry.name, "secure", "Cargo.toml");
		if (!existsSync(manifestPath)) {
			continue;
		}
		const name = parseManifestPackageName(readFileSync(manifestPath, "utf8"));
		if (name !== null) {
			names.push(name);
		}
	}

	return names.toSorted();
}

/** Extract `RUSTSEC-YYYY-NNNN` IDs from `[advisories] ignore = [...]` in `deny.toml`. */
export function parseAdvisoryIgnores(denyToml: string): string[] {
	const ignores: string[] = [];
	let inAdvisories = false;
	let inIgnore = false;

	for (const rawLine of denyToml.split(/\r?\n/u)) {
		const line = rawLine.replace(/#.*$/u, "");
		if (/^\[advisories\]/u.test(line)) {
			inAdvisories = true;
			inIgnore = false;
			continue;
		}
		if (/^\[/u.test(line)) {
			inAdvisories = false;
			inIgnore = false;
			continue;
		}
		if (inAdvisories && /^ignore[ \t]*=[ \t]*\[/u.test(line)) {
			inIgnore = true;
			for (const match of line.matchAll(/"(RUSTSEC-\d+-\d+)"/gu)) {
				if (match[1] !== undefined) {
					ignores.push(match[1]);
				}
			}
			if (line.includes("]")) {
				inIgnore = false;
			}
			continue;
		}
		if (inIgnore) {
			for (const match of line.matchAll(/"(RUSTSEC-\d+-\d+)"/gu)) {
				if (match[1] !== undefined) {
					ignores.push(match[1]);
				}
			}
			if (line.includes("]")) {
				inIgnore = false;
			}
		}
	}

	return ignores;
}

/** Return true when `candidate` is located inside (or equal to) `directory`. */
export function isPathInside(directory: string, candidate: string): boolean {
	const rel = relative(resolve(directory), resolve(candidate));
	return (
		rel === "" ||
		(rel !== ".." && !rel.startsWith(`..${sep}`) && !isAbsolute(rel))
	);
}

function ensureHomeOutsideRepo(env: NodeJS.ProcessEnv): NodeJS.ProcessEnv {
	const currentHome = env.HOME;
	if (
		currentHome !== undefined &&
		currentHome.length > 0 &&
		!isPathInside(REPO_ROOT, currentHome)
	) {
		mkdirSync(currentHome, { recursive: true });
		return env;
	}

	const baseTmp = isPathInside(REPO_ROOT, tmpdir()) ? "/tmp" : tmpdir();
	const createdHome = mkdtempSync(join(baseTmp, "pina-home-"));
	mkdirSync(createdHome, { recursive: true });
	return { ...env, HOME: createdHome };
}

function ensureCargoExpand(env: NodeJS.ProcessEnv): void {
	if (findExecutable("cargo-expand", env) !== undefined) {
		return;
	}

	run(
		"cargo",
		[
			"install",
			"--locked",
			"--version",
			CARGO_EXPAND_VERSION,
			"cargo-expand",
		],
		{ env },
	);
}

function generatedRustClientExcludes(): string[] {
	return discoverChildPackageNames(join(REPO_ROOT, "codama/clients/rust"))
		.flatMap((name) => ["--exclude", name]);
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

function isExecutableFile(path: string): boolean {
	if (!existsSync(path)) {
		return false;
	}
	try {
		accessSync(path, fsConstants.X_OK);
		return true;
	} catch {
		return false;
	}
}

function hasCompletePreinstalledPlatformTools(
	env: NodeJS.ProcessEnv,
	toolsVersion: string,
): boolean {
	if (
		env.PINA_PLATFORM_TOOLS_PREINSTALLED === "1" &&
		(env.SBF_TOOLS_VERSION ?? SBF_TOOLS_VERSION_DEFAULT) === toolsVersion
	) {
		return true;
	}

	const roots = new Set<string>();
	if (env.SOLANA_PLATFORM_TOOLS_DIR) {
		roots.add(env.SOLANA_PLATFORM_TOOLS_DIR);
	}
	if (env.HOME) {
		roots.add(join(env.HOME, ".cache", "solana"));
	}

	for (const root of roots) {
		const ptDir = join(root, toolsVersion, "platform-tools");
		if (isSymlink(ptDir)) {
			continue;
		}
		if (isExecutableFile(join(ptDir, "rust", "bin", "rustc"))) {
			return true;
		}
	}

	return false;
}

function resolveLinuxCargoBuildSbf(
	env: NodeJS.ProcessEnv,
	toolsVersion: string,
): { executable: string; preinstalled: boolean } {
	const located = findExecutable("cargo-build-sbf", env);
	if (located === undefined) {
		throw new Error("cargo-build-sbf is not available on PATH");
	}
	const resolved = realpathSync(located);
	const unwrapped = join(dirname(resolved), ".cargo-build-sbf-wrapped");
	const executable = existsSync(unwrapped) ? unwrapped : resolved;

	const home = env.HOME;
	if (home !== undefined && home.length > 0) {
		const cacheHome = env.XDG_CACHE_HOME ?? join(home, ".cache");
		for (
			const link of new Set([
				join(home, ".cache", "solana", toolsVersion, "platform-tools"),
				join(cacheHome, "solana", toolsVersion, "platform-tools"),
			])
		) {
			if (!isSymlink(link)) {
				continue;
			}
			const target = readlinkSync(link);
			if (
				target.startsWith("/nix/store/") &&
				target.endsWith("/lib/platform-tools")
			) {
				unlinkSync(link);
				continue;
			}
			throw new Error(
				`refusing to replace unexpected platform-tools link: ${target}`,
			);
		}
	}

	return {
		executable,
		preinstalled: hasCompletePreinstalledPlatformTools(env, toolsVersion),
	};
}

function taskLintClippy(): void {
	run("cargo", [
		"clippy",
		"--workspace",
		"--all-features",
		"--locked",
		...generatedRustClientExcludes(),
		"--",
		"-D",
		"warnings",
	]);
}

function taskDocsApi(): void {
	run(
		"cargo",
		[
			"doc",
			"--workspace",
			"--all-features",
			"--no-deps",
			"--locked",
			"--document-private-items",
			...generatedRustClientExcludes(),
		],
		{
			env: { ...process.env, RUSTDOCFLAGS: "-D warnings" },
		},
	);
}

function taskDocsBuild(): void {
	run("mdbook", ["build", join(REPO_ROOT, "docs")]);
	run(process.execPath, [
		join(REPO_ROOT, "scripts/docs/build-landing.ts"),
		join(REPO_ROOT, "docs/book"),
	]);
}

function taskVerifyDocs(): void {
	run("mdt", ["check", "--path", REPO_ROOT]);

	for (const required of ["docs/book.toml", "docs/src/SUMMARY.md"]) {
		if (!existsSync(join(REPO_ROOT, required))) {
			throw new Error(`Missing required docs file: ${required}`);
		}
	}

	run("mdbook", [
		"build",
		join(REPO_ROOT, "docs"),
		"-d",
		join(REPO_ROOT, "target/mdbook"),
	]);
	run(process.execPath, [
		join(REPO_ROOT, "scripts/docs/build-landing.ts"),
		join(REPO_ROOT, "target/mdbook"),
	]);
	taskDocsApi();
	run(process.execPath, [join(REPO_ROOT, "scripts/check-migration-docs.mjs")]);
	run(process.execPath, [
		join(REPO_ROOT, "scripts/docs/sync-skill.mjs"),
		"--check",
	]);
	run("python3", [
		join(REPO_ROOT, "scripts/docs/generate-lint-reference.py"),
		"--check",
	]);
}

function taskSecurityPinaLint(): void {
	run("cargo", ["build", "-p", "pina_lints", "--bin", "pina_lint_driver"]);

	const targetDir = process.env.CARGO_TARGET_DIR ?? join(REPO_ROOT, "target");
	const driver = join(targetDir, "debug", "pina_lint_driver");
	if (!existsSync(driver)) {
		throw new Error(`Expected built lint driver at ${driver}`);
	}

	const packages = [
		...discoverChildPackageNames(join(REPO_ROOT, "examples")),
		...discoverSecureFixturePackageNames(REPO_ROOT),
	];
	if (packages.length === 0) {
		throw new Error(
			"Could not discover any example or security packages to lint.",
		);
	}

	const driverHash = createHash("sha256")
		.update(readFileSync(driver))
		.digest("hex");
	const sysroot = run("rustc", ["--print", "sysroot"], { capture: true }).stdout
		.trim();
	const sysrootLib = join(sysroot, "lib");

	const env: NodeJS.ProcessEnv = {
		...process.env,
		PINA_LINT_NO_DEPS: "1",
		PINA_LINT_DRIVER_BUILD: driverHash,
		RUSTC_WORKSPACE_WRAPPER: driver,
		LD_LIBRARY_PATH: process.env.LD_LIBRARY_PATH
			? `${sysrootLib}:${process.env.LD_LIBRARY_PATH}`
			: sysrootLib,
		DYLD_LIBRARY_PATH: process.env.DYLD_LIBRARY_PATH
			? `${sysrootLib}:${process.env.DYLD_LIBRARY_PATH}`
			: sysrootLib,
	};
	// Cargo chains `RUSTC_WRAPPER` in front of `RUSTC_WORKSPACE_WRAPPER`, which
	// causes `sccache` to reject `pina_lint_driver` as an unsupported compiler.
	delete env.RUSTC_WRAPPER;
	delete env.CARGO_BUILD_RUSTC_WRAPPER;

	run(
		"cargo",
		[
			"check",
			"--locked",
			...packages.flatMap((pkg) => ["--package", pkg]),
			"--config",
			'env.PINA_LINT_LIST.value="0"',
			"--config",
			"env.PINA_LINT_LIST.force=true",
			"--config",
			"env.PINA_LINT_LIST.relative=false",
			"--config",
			'env.PINA_LINT_ONLY.value=""',
			"--config",
			"env.PINA_LINT_ONLY.force=true",
			"--config",
			"env.PINA_LINT_ONLY.relative=false",
			"--config",
			'env.PINA_LINT_LEVELS.value=""',
			"--config",
			"env.PINA_LINT_LEVELS.force=true",
			"--config",
			"env.PINA_LINT_LEVELS.relative=false",
		],
		{ env },
	);
}

function taskSecurityDeny(): void {
	const dbDir = join(REPO_ROOT, "target/advisory-db");
	if (existsSync(dbDir) && !existsSync(join(dbDir, "crates"))) {
		rmSync(dbDir, { recursive: true, force: true });
	}

	run("cargo-deny", ["--workspace", "check", "advisories"]);
	run("cargo-deny", ["check", "bans", "licenses", "sources"]);
}

function taskSecurityAudit(): void {
	const auditDb = join(REPO_ROOT, "target/advisory-db-audit");
	rmSync(auditDb, { recursive: true, force: true });

	const denyToml = readFileSync(join(REPO_ROOT, "deny.toml"), "utf8");
	const ignoreArgs = parseAdvisoryIgnores(denyToml).flatMap((id) => [
		"--ignore",
		id,
	]);

	run("cargo-audit", [
		"audit",
		"--db",
		auditDb,
		"--url",
		"https://github.com/RustSec/advisory-db.git",
		"--deny",
		"yanked",
		...ignoreArgs,
		"--file",
		join(REPO_ROOT, "Cargo.lock"),
	]);
}

function taskVerifyPinaTest(): void {
	const manifest = join(REPO_ROOT, "crates/pina_test/Cargo.toml");
	// `cargo test --all-targets` already compiles and checks every target,
	// avoiding a redundant leading `cargo check --all-targets` pass.
	run("cargo", [
		"test",
		"--manifest-path",
		manifest,
		"--all-targets",
		"--locked",
	]);
	run("cargo", [
		"clippy",
		"--manifest-path",
		manifest,
		"--all-targets",
		"--locked",
		"--",
		"-D",
		"warnings",
	]);
	run(
		"cargo",
		[
			"doc",
			"--manifest-path",
			manifest,
			"--no-deps",
			"--locked",
		],
		{
			env: { ...process.env, RUSTDOCFLAGS: "-D warnings" },
		},
	);
}

function taskTestNpmPackages(env: NodeJS.ProcessEnv = process.env): void {
	run("pnpm", ["--dir", REPO_ROOT, "install", "--frozen-lockfile"], { env });
	run("pnpm", ["--dir", REPO_ROOT, "run", "build:codama-nodes"], { env });
	run("pnpm", ["--dir", REPO_ROOT, "run", "build:codama-renderer-cpi"], {
		env,
	});
	run("pnpm", ["--dir", REPO_ROOT, "run", "check:npm-packages"], { env });
	run("pnpm", ["--dir", REPO_ROOT, "run", "test:npm-packages"], { env });
	run("pnpm", ["--dir", REPO_ROOT, "run", "test:codama-renderer-cpi"], { env });
	run("pnpm", ["--dir", REPO_ROOT, "run", "test:codama-renderer-cli"], { env });
}

function taskTestAll(): void {
	const env = ensureHomeOutsideRepo(process.env);
	run("pnpm", ["--dir", REPO_ROOT, "install", "--frozen-lockfile"], { env });
	run("pnpm", ["--dir", REPO_ROOT, "run", "build:codama-renderer-cli"], {
		env,
	});
	ensureCargoExpand(env);

	run("cargo", ["test", "--workspace", "--locked"], { env });
	run("cargo", ["test", "--all-features", "--locked"], { env });
	run(
		"cargo",
		[
			"test",
			"--locked",
			"--all-features",
			"-p",
			"pina_abi",
			"-p",
			"pina_cli",
			"-p",
			"pina_cli_renderer",
			"-p",
			"pina_codama_renderer",
			"-p",
			"pina_cpi_renderer",
		],
		{ env },
	);
	run(
		"cargo",
		[
			"check",
			"--manifest-path",
			join(REPO_ROOT, "crates/pina_fuzz/fuzz/Cargo.toml"),
			"--all-targets",
			"--locked",
		],
		{ env },
	);
	run("pnpm", ["run", "check:scripts"], { env });
	run("pnpm", ["run", "test:compute-units"], { env });
	taskTestNpmPackages(env);
}

function taskCoveragePinaTest(env: NodeJS.ProcessEnv = process.env): void {
	const coverageDir = join(REPO_ROOT, "target/coverage");
	mkdirSync(coverageDir, { recursive: true });

	const targetDir = join(REPO_ROOT, "target/pina-test-llvm-cov-target");
	const manifest = join(REPO_ROOT, "crates/pina_test/Cargo.toml");
	const covEnv = { ...env, CARGO_TARGET_DIR: targetDir };

	// Clear only `.profraw` profiling artifacts instead of deleting the entire
	// target directory so cached compiled dependencies stay warm across runs.
	if (existsSync(targetDir)) {
		run(
			"cargo",
			["llvm-cov", "clean", "--profraw-only", "--manifest-path", manifest],
			{ env: covEnv },
		);
	}

	run(
		"cargo",
		[
			"llvm-cov",
			"--manifest-path",
			manifest,
			"--all-targets",
			"--locked",
			"--lcov",
			"--output-path",
			join(coverageDir, "pina-test.info"),
		],
		{ env: covEnv },
	);
}

function taskCoverageAll(): void {
	const env = ensureHomeOutsideRepo(process.env);
	run("pnpm", ["--dir", REPO_ROOT, "install", "--frozen-lockfile"], { env });
	run("pnpm", ["--dir", REPO_ROOT, "run", "build:codama-renderer-cli"], {
		env,
	});
	ensureCargoExpand(env);

	const coverageDir = join(REPO_ROOT, "target/coverage");
	mkdirSync(coverageDir, { recursive: true });

	// Clear stale profiling data without wiping compiled crate artifacts.
	if (existsSync(join(REPO_ROOT, "target/llvm-cov-target"))) {
		run("cargo", ["llvm-cov", "clean", "--profraw-only", "--workspace"], {
			env,
		});
	}

	run(
		"cargo",
		[
			"llvm-cov",
			"--all-features",
			"--locked",
			...COVERAGE_PACKAGES.flatMap((pkg) => ["-p", pkg]),
			"--lcov",
			"--output-path",
			join(coverageDir, "lcov.info"),
		],
		{ env },
	);

	taskCoveragePinaTest(env);

	// Keep CARGO_HOME outside the workspace so trybuild normalizes registry paths
	// consistently without discarding the warm ~/.cargo registry cache.
	const defaultCargoHome = join(env.HOME ?? tmpdir(), ".cargo");
	const existingCargoHome = env.CARGO_HOME ?? defaultCargoHome;
	const uiCargoHome = isPathInside(REPO_ROOT, existingCargoHome)
		? "/tmp/pina-ui-cargo-home"
		: existingCargoHome;

	run(
		"cargo",
		["test", "--locked", "-p", "pina_root", "--test", "ui"],
		{
			env: {
				...env,
				CARGO_TARGET_DIR: join(REPO_ROOT, "target/ui-llvm-cov-target"),
				CARGO_HOME: uiCargoHome,
			},
		},
	);

	run(
		"cargo",
		[
			"check",
			"--manifest-path",
			join(REPO_ROOT, "crates/pina_fuzz/fuzz/Cargo.toml"),
			"--all-targets",
			"--locked",
		],
		{ env },
	);
	run("pnpm", ["run", "check:scripts"], { env });
	run("pnpm", ["run", "test:compute-units"], { env });
	taskTestNpmPackages(env);
}

function taskTestSurfpool(): void {
	const env = ensureHomeOutsideRepo(process.env);
	const outDir = env.SBF_OUT_DIR ?? join(REPO_ROOT, "target/surfpool/examples");

	run("pnpm", ["install", "--frozen-lockfile"], { env });
	run("bash", [join(REPO_ROOT, "scripts/build-surfpool-examples.sh")], {
		env: { ...env, SBF_OUT_DIR: outDir },
	});

	// Compile the shared workspace Surfpool test binaries in a single cargo
	// invocation so dependency resolution and crate compilation parallelize
	// cleanly instead of repeating across five serial test commands.
	run(
		"cargo",
		[
			"test",
			"--locked",
			"--no-run",
			"-p",
			"pina_cli",
			"--test",
			"generated_surfpool",
			"--test",
			"rehearse_surfpool",
			"-p",
			"counter-program-surfpool-tests",
			"--test",
			"compute_unit_budget",
			"-p",
			"counter_program_cli",
			"--test",
			"surfpool",
			"-p",
			"migrations-surfpool-tests",
		],
		{ env },
	);

	run(
		"cargo",
		[
			"test",
			"--locked",
			"-p",
			"privacy_pool_program",
			"--features",
			"prover",
			"--test",
			"e2e",
			"copied_transfer_cannot_replace_successor_disclosure_metadata",
			"--",
			"--exact",
			"--ignored",
		],
		{ env: { ...env, SBF_OUT_DIR: outDir } },
	);

	run(
		"pnpm",
		["--dir", join(REPO_ROOT, "codama/tests/surfpool"), "run", "test:types"],
		{ env },
	);
	run(
		"pnpm",
		["--dir", join(REPO_ROOT, "codama/tests/surfpool"), "run", "test"],
		{ env },
	);

	run(
		"cargo",
		[
			"test",
			"--locked",
			"-p",
			"pina_cli",
			"--test",
			"generated_surfpool",
			"--",
			"--ignored",
			"--nocapture",
		],
		{ env },
	);

	run(
		"cargo",
		[
			"test",
			"--locked",
			"-p",
			"counter-program-surfpool-tests",
			"--test",
			"compute_unit_budget",
			"--",
			"--ignored",
			"--nocapture",
		],
		{
			env: {
				...env,
				PINA_SBF_ARTIFACT: join(outDir, "counter_program.so"),
			},
		},
	);

	run(
		"cargo",
		[
			"test",
			"--locked",
			"-p",
			"counter_program_cli",
			"--test",
			"surfpool",
			"--",
			"--ignored",
			"--nocapture",
		],
		{ env },
	);

	run(
		"cargo",
		[
			"test",
			"--locked",
			"-p",
			"pina_cli",
			"--test",
			"rehearse_surfpool",
			"--",
			"--ignored",
			"--nocapture",
		],
		{ env },
	);

	const migrationsCandidates = [
		join(outDir, "migrations_program.so"),
		join(outDir, "libmigrations_program.so"),
	];
	const migrationsArtifact = migrationsCandidates.find((path) =>
		existsSync(path)
	);
	if (migrationsArtifact === undefined) {
		throw new Error(`missing migrations_program SBF artifact under ${outDir}`);
	}

	run(
		"cargo",
		[
			"test",
			"--locked",
			"-p",
			"migrations-surfpool-tests",
			"--",
			"--ignored",
			"--nocapture",
		],
		{ env: { ...env, PINA_SBF_ARTIFACT: migrationsArtifact } },
	);

	run(
		"cargo",
		[
			"test",
			"--locked",
			"-p",
			"migrations-surfpool-tests",
			"--",
			"--ignored",
			"--nocapture",
		],
		{
			env: {
				...env,
				PINA_SBF_ARTIFACT: migrationsArtifact,
				PINA_COMPATIBILITY: "1",
			},
		},
	);
}

function taskTestProgramE2e(): void {
	run("cargo", [
		"test",
		"--locked",
		...PROGRAM_E2E_UNIT_PACKAGES.flatMap((pkg) => ["-p", pkg]),
	]);

	const env = ensureHomeOutsideRepo(process.env);
	const toolsVersion = env.SBF_TOOLS_VERSION ?? SBF_TOOLS_VERSION_DEFAULT;

	if (process.platform === "linux") {
		const { executable, preinstalled } = resolveLinuxCargoBuildSbf(
			env,
			toolsVersion,
		);
		let toolsFlag = preinstalled
			? "--skip-tools-install"
			: "--force-tools-install";
		for (const target of PROGRAM_E2E_SBF_TARGETS) {
			run(
				executable,
				[
					toolsFlag,
					"--tools-version",
					toolsVersion,
					"--manifest-path",
					target.manifest,
					"--sbf-out-dir",
					"target/deploy",
					"--features",
					target.features,
				],
				{ env },
			);
			toolsFlag = "--skip-tools-install";
		}
	} else {
		for (const target of PROGRAM_E2E_SBF_TARGETS) {
			run("cargo", [target.alias], { env });
		}
	}

	run(
		"cargo",
		[
			"test",
			"--locked",
			"-p",
			"pina_bpf_program",
			"sbf_build_",
			"--",
			"--ignored",
		],
		{ env },
	);

	const deployDir = join(REPO_ROOT, "target/deploy");
	const sbfEnv = { ...env, SBF_OUT_DIR: deployDir };

	run(
		"cargo",
		[
			"test",
			"--locked",
			"-p",
			"heap_alloc_program",
			"--test",
			"heap_frames",
			"-p",
			"migrations_program",
			"--test",
			"e2e",
			"-p",
			"profile_program",
			"--test",
			"e2e",
			"-p",
			"role_registry_program",
			"--test",
			"e2e",
			"-p",
			"staking_rewards_program",
			"--test",
			"e2e",
			"-p",
			"vesting_program",
			"--test",
			"e2e",
			"--",
			"--include-ignored",
			"--nocapture",
		],
		{ env: sbfEnv },
	);

	run(
		"cargo",
		[
			"test",
			"--locked",
			"-p",
			"optional_accounts_program",
			"--test",
			"on_chain",
			"--",
			"--include-ignored",
			"--nocapture",
		],
		{ env: sbfEnv },
	);

	const litesvmDir = join(REPO_ROOT, "codama/tests/litesvm");
	run("pnpm", ["--dir", litesvmDir, "install", "--frozen-lockfile"], { env });
	run(
		process.execPath,
		[
			join(litesvmDir, "node_modules/vitest/vitest.mjs"),
			"run",
			"--pool",
			"threads",
		],
		{ cwd: litesvmDir, env: sbfEnv },
	);

	const quasarSvmDir = join(REPO_ROOT, "codama/tests/quasar-svm");
	run("pnpm", ["--dir", quasarSvmDir, "install", "--frozen-lockfile"], { env });
	run(
		process.execPath,
		[
			join(quasarSvmDir, "node_modules/vitest/vitest.mjs"),
			"run",
			"--pool",
			"threads",
		],
		{ cwd: quasarSvmDir, env: sbfEnv },
	);
}

function taskWalkthroughMigrations(extraArgs: readonly string[]): void {
	let env = ensureHomeOutsideRepo(process.env);
	const toolsVersion = env.SBF_TOOLS_VERSION ?? SBF_TOOLS_VERSION_DEFAULT;
	let shimDir: string | undefined;

	try {
		if (process.platform === "linux") {
			const { executable } = resolveLinuxCargoBuildSbf(env, toolsVersion);
			if (executable.endsWith("/.cargo-build-sbf-wrapped")) {
				shimDir = mkdtempSync(join(tmpdir(), "pina-sbf-shim-"));
				symlinkSync(executable, join(shimDir, "cargo-build-sbf"));
				env = {
					...env,
					PATH: env.PATH ? `${shimDir}:${env.PATH}` : shimDir,
				};
			}
		}

		run("pnpm", ["--dir", REPO_ROOT, "install", "--frozen-lockfile"], { env });
		run("pnpm", ["--dir", REPO_ROOT, "run", "build:codama-renderer-cli"], {
			env,
		});
		run(
			"pnpm",
			[
				"--dir",
				REPO_ROOT,
				"exec",
				"tsx",
				join(REPO_ROOT, "scripts/migrations-walkthrough.ts"),
				...extraArgs,
			],
			{ env },
		);
	} finally {
		if (shimDir !== undefined) {
			rmSync(shimDir, { recursive: true, force: true });
		}
	}
}

function taskMutantsPrepare(): void {
	const env = ensureHomeOutsideRepo(process.env);
	run("pnpm", ["--dir", REPO_ROOT, "install", "--frozen-lockfile"], { env });
	run("pnpm", ["--dir", REPO_ROOT, "run", "build:codama-renderer-cli"], {
		env,
	});
}

function taskMutantsRun(args: readonly string[]): number {
	const env = ensureHomeOutsideRepo(process.env);
	const listResult = run("cargo", ["mutants", "--list", ...args], {
		env,
		capture: true,
	});
	const lines = listResult.stdout
		.split(/\r?\n/u)
		.filter((line) => line.trim().length > 0);

	if (lines.length === 0) {
		let emptyAllowed = args.some(
			(arg) => arg === "--in-diff" || arg.startsWith("--in-diff="),
		);

		if (!emptyAllowed) {
			const inventory = new Set<string>(MUTANT_PACKAGES);
			for (let index = 0; index < args.length; index += 1) {
				if (args[index] === "-p" || args[index] === "--package") {
					const pkg = args[index + 1];
					if (pkg !== undefined) {
						if (inventory.has(pkg)) {
							emptyAllowed = false;
							break;
						}
						emptyAllowed = true;
					}
				}
			}
		}

		if (emptyAllowed) {
			process.stdout.write(
				"The selection contains no mutants; nothing to test.\n",
			);
			return 0;
		}

		process.stderr.write(
			`cargo-mutants found no mutants for: ${args.join(" ")}\n`,
		);
		process.stderr.write(
			"A run that tests nothing is not a pass; check .cargo/mutants.toml and the package selection.\n",
		);
		return 1;
	}

	process.stdout.write(`Testing ${lines.length} mutants.\n`);
	const outDir = join(REPO_ROOT, "target/mutants");
	mkdirSync(outDir, { recursive: true });
	const result = run(
		"cargo",
		["mutants", "--output", outDir, ...args],
		{ env, allowFailure: true },
	);
	return result.status;
}

function taskMutantsCrate(args: readonly string[]): number {
	const [pkg, ...rest] = args;
	if (pkg === undefined) {
		process.stderr.write(
			"Usage: ci-tasks.ts mutants:crate <package-name> [cargo-mutants args...]\n",
		);
		return 1;
	}

	const metadataRaw = run(
		"cargo",
		["metadata", "--no-deps", "--format-version", "1"],
		{ capture: true },
	).stdout;
	const metadata = JSON.parse(metadataRaw) as {
		packages: Array<{ name: string }>;
	};
	if (!metadata.packages.some((entry) => entry.name === pkg)) {
		process.stderr.write(
			`Package '${pkg}' does not belong to the root workspace.\n`,
		);
		return 1;
	}

	return taskMutantsRun(["-p", pkg, ...rest]);
}

function taskMutantsAll(extraArgs: readonly string[]): number {
	return taskMutantsRun([
		...MUTANT_PACKAGES.flatMap((pkg) => ["-p", pkg]),
		...extraArgs,
	]);
}

function taskFixFormat(): void {
	run("dprint", ["fmt", "--config", join(REPO_ROOT, "dprint.json")]);
	run("mdt", ["update", "--path", REPO_ROOT]);
}

function main(): number {
	const [task, ...args] = process.argv.slice(2);
	switch (task) {
		case "fix:format":
			taskFixFormat();
			return 0;
		case "lint:clippy":
			taskLintClippy();
			return 0;
		case "docs:api":
			taskDocsApi();
			return 0;
		case "docs:build":
			taskDocsBuild();
			return 0;
		case "verify:docs":
			taskVerifyDocs();
			return 0;
		case "security:pina-lint":
			taskSecurityPinaLint();
			return 0;
		case "security:deny":
			taskSecurityDeny();
			return 0;
		case "security:audit":
			taskSecurityAudit();
			return 0;
		case "verify:pina-test":
			taskVerifyPinaTest();
			return 0;
		case "test:npm-packages":
			taskTestNpmPackages();
			return 0;
		case "test:all":
			taskTestAll();
			return 0;
		case "coverage:pina-test":
			taskCoveragePinaTest();
			return 0;
		case "coverage:all":
			taskCoverageAll();
			return 0;
		case "test:surfpool":
			taskTestSurfpool();
			return 0;
		case "test:program-e2e":
			taskTestProgramE2e();
			return 0;
		case "walkthrough:migrations":
			taskWalkthroughMigrations(args);
			return 0;
		case "mutants:prepare":
			taskMutantsPrepare();
			return 0;
		case "mutants:packages":
			process.stdout.write(`${MUTANT_PACKAGES.join("\n")}\n`);
			return 0;
		case "mutants:run":
			return taskMutantsRun(args);
		case "mutants:crate":
			return taskMutantsCrate(args);
		case "mutants:all":
			return taskMutantsAll(args);
		default:
			process.stderr.write(`Unknown CI task: ${task ?? "<none>"}\n`);
			return 1;
	}
}

if (
	process.argv[1] !== undefined &&
	import.meta.url === pathToFileURL(process.argv[1]).href
) {
	try {
		process.exitCode = main();
	} catch (error: unknown) {
		process.stderr.write(
			`Error: ${error instanceof Error ? error.message : String(error)}\n`,
		);
		process.exitCode = 1;
	}
}
