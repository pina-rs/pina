#!/usr/bin/env node

// Regenerate the `pina profile trace` fixtures in
// `crates/pina_profile/tests/fixtures/trace/` from `examples/counter_program`.
//
//   node scripts/regenerate-trace-fixtures.ts
//
// The script runs the real workflow (two `cargo build-sbf` builds and the
// counter's traced Mollusk tests), then copies the traced build and one
// recording of each counter instruction into the fixture directory. Run it
// inside `devenv shell` after changing the counter program, the pina crate,
// or the platform tools, then refresh the snapshots that depend on it:
//
//   INSTA_UPDATE=always cargo test -p pina_profile --test trace
//   INSTA_UPDATE=always cargo test -p pina_cli --test profile_trace_command

import { spawnSync } from "node:child_process";
import {
	chmodSync,
	copyFileSync,
	mkdirSync,
	readdirSync,
	rmSync,
	statSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const PROGRAM = "counter_program";
const INSTRUCTIONS = ["increment", "initialize"];
const TRACE_EXTENSIONS = [".regs", ".insns", ".program_id", ".exec.sha256"];
const WORKSPACE = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const FIXTURES = join(WORKSPACE, "crates/pina_profile/tests/fixtures/trace");

interface TraceDocument {
	instructions: { instruction: string | null; traceIds: string[] }[];
}

interface CargoMetadata {
	target_directory: string;
}

function run(program: string, args: string[]): string {
	const result = spawnSync(program, args, {
		cwd: WORKSPACE,
		encoding: "utf8",
		stdio: ["ignore", "pipe", "inherit"],
		maxBuffer: 64 * 1024 * 1024,
	});

	if (result.error !== undefined) {
		throw result.error;
	}

	if (result.status !== 0) {
		throw new Error(
			`${program} ${args.join(" ")} exited with ${result.status}`,
		);
	}

	return result.stdout;
}

function main(): void {
	const report = JSON.parse(
		run("cargo", [
			"run",
			"--quiet",
			"--locked",
			"-p",
			"pina_cli",
			"--",
			"profile",
			"trace",
			"--project",
			join("examples", PROGRAM),
			"--json",
		]),
	) as TraceDocument;
	const metadata = JSON.parse(
		run("cargo", ["metadata", "--format-version", "1", "--no-deps"]),
	) as CargoMetadata;
	const trace = join(metadata.target_directory, "pina", "trace");
	const traceIds = INSTRUCTIONS.map((name) => {
		const profile = report.instructions.find((entry) =>
			entry.instruction === name
		);
		const id = profile?.traceIds[0];

		if (id === undefined) {
			throw new Error(`no recording of ${PROGRAM}::${name} was traced`);
		}

		return id;
	});

	rmSync(join(FIXTURES, "traces"), { recursive: true, force: true });
	mkdirSync(join(FIXTURES, "traces"), { recursive: true });
	copyFileSync(
		join(trace, "build", `${PROGRAM}.so`),
		join(FIXTURES, `${PROGRAM}.so`),
	);
	copyFileSync(
		join(trace, `${PROGRAM}.so.debug`),
		join(FIXTURES, `${PROGRAM}.so.debug`),
	);

	for (const id of traceIds) {
		for (const extension of TRACE_EXTENSIONS) {
			copyFileSync(
				join(trace, "traces", `${id}${extension}`),
				join(FIXTURES, "traces", `${id}${extension}`),
			);
		}
	}

	for (const directory of [FIXTURES, join(FIXTURES, "traces")]) {
		for (const name of readdirSync(directory).sort()) {
			const path = join(directory, name);
			const stats = statSync(path);

			if (stats.isFile() && name !== "README.md") {
				// Fixtures are data; keep the executable bit cargo-build-sbf sets out of git.
				chmodSync(path, 0o644);
				process.stdout.write(
					`${stats.size}\t${path.slice(WORKSPACE.length + 1)}\n`,
				);
			}
		}
	}
}

main();
