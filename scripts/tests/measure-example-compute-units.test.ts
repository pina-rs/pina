import assert from "node:assert/strict";
import test from "node:test";

import { commandAsync } from "../measure-example-compute-units.ts";

test("terminates a batch that exceeds its timeout", async () => {
	const startedAt = Date.now();

	const status = await commandAsync("sleep", ["60"], {
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
	const status = await commandAsync("sleep", ["0.2"], {
		cwd: process.cwd(),
		env: process.env,
		timeoutMinutes: 1,
	});

	assert.strictEqual(status, 0);
});

test("escalates to SIGKILL when a batch ignores SIGTERM", async () => {
	const startedAt = Date.now();

	const status = await commandAsync("node", [
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

	const status = await commandAsync("false", [], {
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
