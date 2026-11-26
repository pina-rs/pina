import { spawnSync } from "node:child_process";
import { resolve } from "node:path";

interface CargoMetadata {
	target_directory?: unknown;
}

/**
 * Resolve Cargo's effective target directory from `cargo metadata`.
 *
 * Unlike checking `CARGO_TARGET_DIR` alone, this honors `CARGO_BUILD_TARGET_DIR`
 * and `build.target-dir` from ancestor or user `.cargo/config.toml` files, and
 * normalizes any `..` segments Cargo leaves in `target_directory`.
 */
export function resolveCargoTargetDir(
	cwd: string,
	env: NodeJS.ProcessEnv = process.env,
): string {
	const result = spawnSync(
		"cargo",
		["metadata", "--format-version", "1", "--no-deps"],
		{
			cwd,
			env,
			encoding: "utf8",
			stdio: ["ignore", "pipe", "inherit"],
			maxBuffer: 32 * 1024 * 1024,
		},
	);

	if (result.error !== undefined) {
		throw result.error;
	}

	const status = result.status ?? 1;
	if (status !== 0) {
		throw new Error(
			`Command failed with exit status ${status}: cargo metadata --format-version 1 --no-deps`,
		);
	}

	const metadata = JSON.parse(result.stdout) as CargoMetadata;
	if (
		typeof metadata.target_directory !== "string" ||
		metadata.target_directory.length === 0
	) {
		throw new Error("cargo metadata did not report a target_directory");
	}

	return resolve(cwd, metadata.target_directory);
}
