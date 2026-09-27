// Filesystem layout and process helpers for the Pina skill evaluation harness.
//
// Everything the harness creates lives under `evals/pina-skill/.work/`, which is
// git-ignored: run workdirs are throwaway Rust crates, and each one carries its
// own `target/` directory.

import { spawnSync } from "node:child_process";
import {
	cpSync,
	existsSync,
	mkdirSync,
	readdirSync,
	readFileSync,
	rmSync,
	writeFileSync,
} from "node:fs";
import { dirname, join, resolve, sep } from "node:path";
import process from "node:process";

/// Repository root of the worktree this harness lives in.
export const HARNESS_ROOT = resolve(import.meta.dirname, "..");

/// Directory holding scenario definitions.
export const SCENARIO_DIR = join(HARNESS_ROOT, "scenarios");

/// Directory holding fixture projects copied per run.
export const FIXTURE_DIR = join(HARNESS_ROOT, "fixtures");

/// Scratch space for run workdirs and results.
export const WORK_DIR = join(HARNESS_ROOT, ".work");

/// Saved transcripts and machine-readable results.
export const RESULT_DIR = join(HARNESS_ROOT, "results");

/// Skill variants: each directory is a complete `pina` skill (SKILL.md plus
/// references). The runner copies the chosen variant into the run workdir as
/// `.claude/skills/pina`, which takes precedence over any globally installed
/// skill of the same name.
export const SKILL_VARIANT_DIR = join(HARNESS_ROOT, "skill-variants");

/// The `pina` CLI the fixtures are graded with.
///
/// The harness's own checkout is authoritative, and its `target/` is checked
/// first because this harness may run in a linked worktree whose build is newer
/// than the main checkout's. A `pina` found on `PATH` is frequently an older
/// release whose subcommands differ (`make` rather than `create`), so it is
/// never used.
export function resolvePinaCli(): string {
	const candidates = [
		join(HARNESS_ROOT, "..", "target", "debug", "pina"),
		join(HARNESS_ROOT, "..", "target", "release", "pina"),
		join(HARNESS_ROOT, "..", "..", "target", "debug", "pina"),
		join(HARNESS_ROOT, "..", "..", "target", "release", "pina"),
	];
	for (const candidate of candidates) {
		if (existsSync(candidate)) {
			return resolve(candidate);
		}
	}
	throw new Error(
		`No pina CLI found. Expected one of:\n  ${candidates.join("\n  ")}\n` +
			`Build it from the repository root with: devenv shell -- cargo build -p pina_cli`,
	);
}

export interface ExecOptions {
	cwd: string;
	/// Seconds before the command is killed.
	timeoutSeconds?: number;
	env?: Record<string, string>;
}

export interface ExecResult {
	exit: number;
	stdout: string;
	stderr: string;
	timedOut: boolean;
}

/// Run a command, capturing combined output without throwing on failure.
export function exec(command: string, options: ExecOptions): ExecResult {
	const result = spawnSync("sh", ["-c", command], {
		cwd: options.cwd,
		encoding: "utf8",
		env: { ...process.env, ...options.env },
		timeout: (options.timeoutSeconds ?? 300) * 1000,
		maxBuffer: 32 * 1024 * 1024,
	});

	const timedOut = result.signal === "SIGTERM" ||
		result.error?.name === "Error" &&
			String(result.error.message).includes("ETIMEDOUT");

	return {
		exit: result.status ?? (timedOut ? 124 : 1),
		stdout: result.stdout ?? "",
		stderr: result.stderr ?? "",
		timedOut,
	};
}

/// Create an empty directory, removing any previous contents.
export function resetDir(path: string): void {
	rmSync(path, { recursive: true, force: true });
	mkdirSync(path, { recursive: true });
}

/// Root of the repository this harness is evaluating.
///
/// The harness lives at `<repo>/evals/pina-skill`, and it often runs inside a
/// linked worktree, so the parent of the harness root is the checkout the
/// fixtures must resolve their `pina` dependency from.
export const REPO_ROOT = resolve(HARNESS_ROOT, "..", "..");

/// Copy a fixture tree into a fresh workdir.
///
/// `${PINA_ROOT}` in a copied file is rewritten to the repository root so the
/// fixture can depend on the local `pina` crates by path. A fixture committed
/// with an absolute path would break for every other checkout.
export function copyFixture(fixtureName: string, destination: string): void {
	const source = join(FIXTURE_DIR, fixtureName);
	if (!existsSync(source)) {
		throw new Error(`Fixture not found: ${source}`);
	}
	resetDir(destination);
	cpSync(source, destination, { recursive: true });
	substitutePlaceholders(destination);
}

/// Rewrite the `${PINA_ROOT}` placeholder in every text file under `root`.
function substitutePlaceholders(root: string): void {
	for (const file of walk(root)) {
		if (
			file.includes(`${sep}target${sep}`) || file.includes(`${sep}.git${sep}`)
		) {
			continue;
		}
		let contents: string;
		try {
			contents = readFileSync(file, "utf8");
		} catch {
			continue;
		}
		if (!contents.includes("${PINA_ROOT}")) {
			continue;
		}
		writeFileSync(file, contents.replaceAll("${PINA_ROOT}", REPO_ROOT));
	}
}

/// Recursively list files under `root`.
function walk(root: string): string[] {
	const found: string[] = [];
	for (const entry of readdirSync(root, { withFileTypes: true })) {
		const path = join(root, entry.name);
		if (entry.isDirectory()) {
			found.push(...walk(path));
		} else if (entry.isFile()) {
			found.push(path);
		}
	}
	return found;
}

/// Resolve a path relative to the harness root.
export function harnessPath(...segments: string[]): string {
	return join(HARNESS_ROOT, ...segments);
}

/// Ensure the parent directory of a file exists.
export function ensureParent(path: string): void {
	mkdirSync(dirname(path), { recursive: true });
}
