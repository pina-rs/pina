import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const ROOT_LOCKFILE = "Cargo.lock";

/**
 * Names of the packages a `Cargo.lock` records without a `source`.
 *
 * Cargo writes a `source` line for every registry and git package and omits it
 * for path packages, so these are the crates the lockfile's workspace builds
 * from local source. The file is machine-written with one `key = "value"` per
 * line inside each `[[package]]` table, which is all this reader relies on.
 */
export function pathPackageNames(lockfile: string): Set<string> {
	const names = new Set<string>();

	for (const table of lockfile.split("[[package]]").slice(1)) {
		const name = /^name = "([^"]+)"$/m.exec(table)?.[1];
		if (name !== undefined && !/^source = /m.test(table)) {
			names.add(name);
		}
	}

	return names;
}

/**
 * The standalone lockfiles that record a crate of the root workspace.
 *
 * A release changes the version of every root-workspace crate. A nested
 * workspace that depends on one by path records that version in its own
 * lockfile, so its lockfile is stale until it is refreshed and every
 * `--locked` build of it fails. Lockfiles that record no root-workspace crate
 * are left alone: refreshing them would only add network work to the release.
 */
export function staleStandaloneLockfiles(
	lockfiles: ReadonlyMap<string, string>,
): string[] {
	const rootLockfile = lockfiles.get(ROOT_LOCKFILE);
	if (rootLockfile === undefined) {
		throw new Error(`${ROOT_LOCKFILE} is not tracked at the repository root`);
	}

	const workspaceCrates = pathPackageNames(rootLockfile);

	return [...lockfiles]
		.filter(([path, contents]) =>
			path !== ROOT_LOCKFILE &&
			[...pathPackageNames(contents)].some((name) => workspaceCrates.has(name))
		)
		.map(([path]) => path)
		.sort();
}

/**
 * The environment for a command that must address the repository in its
 * working directory.
 *
 * A git hook exports `GIT_DIR` and its siblings for the repository that fired
 * it, and every process started beneath the hook inherits them. They outrank
 * the working directory, so an inherited `GIT_DIR` points `git ls-files` at the
 * hook's repository and makes `git init` in a scratch directory rewrite that
 * repository's configuration. Git itself names the variables to drop.
 */
export function repositoryEnvironment(
	env: NodeJS.ProcessEnv = process.env,
): NodeJS.ProcessEnv {
	const listed = spawnSync("git", ["rev-parse", "--local-env-vars"], {
		encoding: "utf8",
		env,
	});
	if (listed.status !== 0) {
		const reason = listed.error?.message ?? listed.stderr;
		throw new Error(`git rev-parse --local-env-vars failed: ${reason}`);
	}

	const repositoryVariables = new Set(listed.stdout.split("\n"));

	return Object.fromEntries(
		Object.entries(env).filter(([name]) => !repositoryVariables.has(name)),
	);
}

/** Read every tracked `Cargo.lock`, keyed by its path from the repository root. */
export function trackedLockfiles(root: string): Map<string, string> {
	const listed = spawnSync(
		"git",
		["ls-files", "-z", "--", ROOT_LOCKFILE, `*/${ROOT_LOCKFILE}`],
		{ cwd: root, encoding: "utf8", env: repositoryEnvironment() },
	);
	if (listed.status !== 0) {
		const reason = listed.error?.message ?? listed.stderr;
		throw new Error(`git ls-files failed in ${root}: ${reason}`);
	}

	return new Map(
		listed.stdout
			.split("\0")
			.filter((path) => path.length > 0)
			.map((path) => [path, readFileSync(join(root, path), "utf8")]),
	);
}

function main(): void {
	const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
	// Cargo shells out to git for git dependencies.
	const env = repositoryEnvironment();

	for (const lockfile of staleStandaloneLockfiles(trackedLockfiles(root))) {
		const manifest = join(dirname(lockfile), "Cargo.toml");
		console.log(`Refreshing ${lockfile}`);

		const updated = spawnSync(
			"cargo",
			["update", "--workspace", "--manifest-path", manifest],
			{ cwd: root, env, stdio: "inherit" },
		);
		if (updated.status !== 0) {
			throw new Error(`cargo update failed for ${manifest}`);
		}
	}
}

if (
	process.argv[1] !== undefined &&
	import.meta.url === pathToFileURL(process.argv[1]).href
) {
	main();
}
