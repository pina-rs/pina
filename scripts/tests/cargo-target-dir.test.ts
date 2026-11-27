import assert from "node:assert/strict";
import {
	mkdirSync,
	mkdtempSync,
	realpathSync,
	rmSync,
	writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { resolveCargoTargetDir } from "../cargo-target-dir.ts";

function cleanCargoEnv(): NodeJS.ProcessEnv {
	const env: NodeJS.ProcessEnv = { ...process.env };
	delete env.CARGO_TARGET_DIR;
	delete env.CARGO_BUILD_TARGET_DIR;
	return env;
}

test("resolveCargoTargetDir honors a parent .cargo/config.toml with an un-normalized relative target-dir", () => {
	const root = realpathSync(
		mkdtempSync(join(tmpdir(), "pina-cargo-target-dir-")),
	);
	try {
		const worktrees = join(root, "worktrees");
		const project = join(worktrees, "repo", "pkg");
		mkdirSync(join(worktrees, ".cargo"), { recursive: true });
		mkdirSync(join(project, "src"), { recursive: true });

		writeFileSync(
			join(worktrees, ".cargo", "config.toml"),
			'[build]\ntarget-dir = "../shared-target"\n',
		);
		writeFileSync(
			join(project, "Cargo.toml"),
			'[package]\nname = "demo"\nversion = "0.1.0"\nedition = "2021"\n',
		);
		writeFileSync(join(project, "src", "lib.rs"), "");

		const resolved = resolveCargoTargetDir(project, cleanCargoEnv());
		assert.equal(resolved, join(root, "shared-target"));
		assert.ok(
			!resolved.includes(".."),
			`expected normalized path, got ${resolved}`,
		);
	} finally {
		rmSync(root, { force: true, recursive: true });
	}
});

test("resolveCargoTargetDir falls back to the crate target directory when no external config applies", () => {
	const root = realpathSync(
		mkdtempSync(join(tmpdir(), "pina-cargo-target-dir-local-")),
	);
	try {
		const project = join(root, "pkg");
		const isolatedCargoHome = join(root, "cargo-home");
		mkdirSync(join(project, "src"), { recursive: true });
		mkdirSync(isolatedCargoHome, { recursive: true });

		writeFileSync(
			join(project, "Cargo.toml"),
			'[package]\nname = "demo"\nversion = "0.1.0"\nedition = "2021"\n',
		);
		writeFileSync(join(project, "src", "lib.rs"), "");

		const env = { ...cleanCargoEnv(), CARGO_HOME: isolatedCargoHome };
		assert.equal(resolveCargoTargetDir(project, env), join(project, "target"));
	} finally {
		rmSync(root, { force: true, recursive: true });
	}
});
