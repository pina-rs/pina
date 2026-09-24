import assert from "node:assert/strict";
import test from "node:test";

import {
	type BatchEvidence,
	classifyBatchExit,
	commandAsync,
} from "../measure-example-compute-units.ts";

test("terminates a batch that exceeds its timeout", async () => {
	const startedAt = Date.now();

	const { status } = await commandAsync("sleep", ["60"], {
		cwd: process.cwd(),
		env: process.env,
		timeoutMinutes: 0.05,
	});

	const elapsedSeconds = (Date.now() - startedAt) / 1000;

	assert.notStrictEqual(status, 0);
	assert.ok(
		elapsedSeconds < 30,
		`the batch ran for ${
			elapsedSeconds.toFixed(1)
		}s instead of being terminated`,
	);
});

test("reports the exit status of a batch that finishes in time", async () => {
	const { status } = await commandAsync("sleep", ["0.2"], {
		cwd: process.cwd(),
		env: process.env,
		timeoutMinutes: 1,
	});

	assert.strictEqual(status, 0);
});

test("escalates to SIGKILL when a batch ignores SIGTERM", async () => {
	const startedAt = Date.now();

	const { status } = await commandAsync("node", [
		"-e",
		"process.on('SIGTERM', () => {}); setInterval(() => {}, 1000);",
	], {
		cwd: process.cwd(),
		env: process.env,
		timeoutMinutes: 0.05,
	});

	const elapsedSeconds = (Date.now() - startedAt) / 1000;

	assert.notStrictEqual(status, 0);
	assert.ok(
		elapsedSeconds < 20,
		`the batch ran for ${
			elapsedSeconds.toFixed(1)
		}s instead of being killed after the grace period`,
	);
});

test("propagates a nonzero exit status without waiting for the timeout", async () => {
	const startedAt = Date.now();

	const { status } = await commandAsync("false", [], {
		cwd: process.cwd(),
		env: process.env,
		timeoutMinutes: 1,
	});

	const elapsedSeconds = (Date.now() - startedAt) / 1000;

	assert.strictEqual(status, 1);
	assert.ok(
		elapsedSeconds < 30,
		`the batch ran for ${elapsedSeconds.toFixed(1)}s`,
	);
});

test("captures a batch's stdout alongside a crash exit status", async () => {
	const { status, stdout } = await commandAsync("node", [
		"-e",
		"console.log('test result: ok. 1 passed; 0 failed'); process.exit(101);",
	], {
		cwd: process.cwd(),
		env: process.env,
		timeoutMinutes: 1,
	});

	assert.strictEqual(status, 101);
	assert.strictEqual(stdout, "test result: ok. 1 passed; 0 failed\n");
});

const passingOutput = [
	"running 2 tests",
	"test initialize ... ok",
	"test increment ... ok",
	"",
	"test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.00s",
	"",
	"running 1 test",
	"test transfer ... ok",
	"",
	"test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.00s",
	"",
].join("\n");

function evidence(overrides: Partial<BatchEvidence> = {}): BatchEvidence {
	return {
		status: 101,
		stdout: passingOutput,
		testBinaries: 2,
		ownedCases: ["counter/initialize", "counter/increment", "sol/transfer"],
		sampleCounts: new Map([
			["counter/initialize", 3],
			["counter/increment", 1],
			["sol/transfer", 2],
		]),
		...overrides,
	};
}

test("a zero exit passes regardless of the recorded evidence", () => {
	assert.equal(
		classifyBatchExit(
			evidence({ status: 0, stdout: "", sampleCounts: new Map() }),
		),
		"passed",
	);
});

test("tolerates a teardown exit once every binary passed and every case is measured", () => {
	assert.equal(classifyBatchExit(evidence()), "incompleteTeardown");
	assert.equal(
		classifyBatchExit(evidence({ status: 137 })),
		"incompleteTeardown",
	);
});

test("reads summaries that a forced color flag wrapped in escapes", () => {
	const colored = passingOutput.replaceAll(
		"test result: ok.",
		"test result: \u001b[32mok\u001b[0m.",
	);

	assert.equal(
		classifyBatchExit(evidence({ stdout: colored })),
		"incompleteTeardown",
	);
});

test("fails when an owned case has no recorded samples", () => {
	const sampleCounts = new Map([
		["counter/initialize", 3],
		["counter/increment", 1],
	]);

	assert.equal(classifyBatchExit(evidence({ sampleCounts })), "failed");
	assert.equal(
		classifyBatchExit(
			evidence({
				sampleCounts: new Map([...sampleCounts, ["sol/transfer", 0]]),
			}),
		),
		"failed",
	);
});

test("fails when any test binary reports a failed result", () => {
	const stdout = passingOutput.replace(
		"test result: ok. 1 passed; 0 failed",
		"test result: FAILED. 0 passed; 1 failed",
	);

	assert.equal(classifyBatchExit(evidence({ stdout })), "failed");
});

test("fails when a binary panicked before printing its summary", () => {
	const stdout = [
		"test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.00s",
		"",
		"running 1 test",
		"thread 'surfnet-sdk' panicked at tokio/src/runtime/blocking/shutdown.rs:51:21:",
	].join("\n");

	assert.equal(classifyBatchExit(evidence({ stdout })), "failed");
});

test("fails when the batch never ran a test binary", () => {
	assert.equal(
		classifyBatchExit(evidence({ stdout: "error: could not compile" })),
		"failed",
	);
	assert.equal(
		classifyBatchExit(evidence({ stdout: "", testBinaries: 0 })),
		"failed",
	);
});

test("fails when the batch owns no cases to prove measured", () => {
	assert.equal(
		classifyBatchExit(evidence({ ownedCases: [], sampleCounts: new Map() })),
		"failed",
	);
});

test("ignores summary text that is not at the start of a line", () => {
	const stdout = passingOutput.replace(
		"test result: ok. 1 passed",
		"log: test result: ok. 1 passed",
	);

	assert.equal(classifyBatchExit(evidence({ stdout })), "failed");
});
