import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
	chmodSync,
	mkdirSync,
	mkdtempSync,
	readdirSync,
	readFileSync,
	rmSync,
	writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { delimiter, dirname, join, resolve } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const REPO_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const SCRIPT_PATH = join(REPO_ROOT, "scripts/generate-pina-clients.sh");

test("generate-pina-clients.sh does not use readFileSync on stdin", () => {
	const executableLines = readFileSync(SCRIPT_PATH, "utf8")
		.split(/\r?\n/u)
		.filter((line) => !line.trimStart().startsWith("#"))
		.join("\n");
	assert.doesNotMatch(
		executableLines,
		/readFileSync\s*\(\s*(?:0|["']\/dev\/stdin["'])/u,
		"stdin must be streamed instead of read via fs.readFileSync(0, 'utf8') (nodejs/node#66341)",
	);
});

test("generate-pina-clients.sh parses cargo JSON over a pipe with >64 KiB of short reads", () => {
	const root = mkdtempSync(join(tmpdir(), "pina-generate-clients-test-"));
	try {
		const binDir = join(root, "bin");
		mkdirSync(binDir);

		const invocationsLog = join(root, "pina-invocations.log");
		const fakePina = join(root, "fake-pina");
		writeFileSync(
			fakePina,
			`#!/usr/bin/env bash
set -euo pipefail
printf '%s\\n' "$*" >> ${JSON.stringify(invocationsLog)}
`,
		);
		chmodSync(fakePina, 0o755);

		const fakePnpm = join(binDir, "pnpm");
		writeFileSync(fakePnpm, "#!/usr/bin/env bash\nexit 0\n");
		chmodSync(fakePnpm, 0o755);

		// Emit 24 short (<8 KiB) JSON chunks totaling >96 KiB followed by a
		// 400 KiB burst containing the `pina` compiler-artifact line. On Node
		// 24.21.0..24.22.3 / 26.8.0..26.13.0 this reproduces the ReadFileUtf8
		// heap buffer overflow (exit 134) if stdin is read with
		// `fs.readFileSync(0, "utf8")` (nodejs/node#66341).
		const fakeCargo = join(binDir, "cargo");
		writeFileSync(
			fakeCargo,
			`#!/usr/bin/env node
const padding = "x".repeat(3900);
const shortLine = JSON.stringify({
	reason: "compiler-artifact",
	target: { name: "dep" },
	executable: null,
	padding,
}) + "\\n";
const largeBurst =
	JSON.stringify({
		reason: "compiler-artifact",
		target: { name: "large_dep" },
		executable: null,
		padding: "y".repeat(400000),
	}) +
	"\\n" +
	JSON.stringify({
		reason: "compiler-artifact",
		target: { name: "pina" },
		executable: ${JSON.stringify(fakePina)},
	}) +
	"\\n";

let count = 0;
const timer = setInterval(() => {
	process.stdout.write(shortLine);
	count += 1;
	if (count === 24) {
		clearInterval(timer);
		process.stdout.write(largeBurst);
	}
}, 5);
`,
		);
		chmodSync(fakeCargo, 0o755);

		const result = spawnSync("bash", [SCRIPT_PATH], {
			cwd: REPO_ROOT,
			encoding: "utf8",
			env: {
				...process.env,
				PATH: `${binDir}${delimiter}${process.env.PATH ?? ""}`,
			},
		});

		assert.equal(
			result.status,
			0,
			`expected exit 0, got status=${String(result.status)} signal=${
				String(result.signal)
			}\nstderr:\n${result.stderr}`,
		);

		const expectedExamples = readdirSync(join(REPO_ROOT, "examples"), {
			withFileTypes: true,
		})
			.filter((entry) => entry.isDirectory())
			.map((entry) => entry.name)
			.sort();
		const invocations = readFileSync(invocationsLog, "utf8")
			.trim()
			.split("\n");
		assert.equal(invocations.length, expectedExamples.length);
		assert.equal(
			invocations[0],
			`generate --project ${
				join(REPO_ROOT, "examples", expectedExamples[0])
			} --npx node`,
		);
	} finally {
		rmSync(root, { force: true, recursive: true });
	}
});
