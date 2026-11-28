import assert from "node:assert/strict";
import { Buffer } from "node:buffer";

import {
	decodeChangedEvent,
	MIGRATIONS_PROGRAM_EVENT_SOURCE_ADDRESS as PROGRAM,
	parseChangedEventFromLog,
	parseMigrationsProgramEventsFromLogs,
} from "./events/logs.js";

function programDataLog(bytes: Uint8Array) {
	return `Program data: ${Buffer.from(bytes).toString("base64")}`;
}

function parseTransaction(bytes: Uint8Array) {
	return parseMigrationsProgramEventsFromLogs([
		`Program ${PROGRAM} invoke [1]`,
		programDataLog(bytes),
		`Program ${PROGRAM} success`,
	]);
}

const discriminator = [0x34, 0x12];
const known = Uint8Array.of(...discriminator, 7, 42);
const expected = {
	name: "changed",
	data: { discriminator: 0x1234, migrationVersion: 7, value: 42 },
};
assert.deepEqual(decodeChangedEvent(known), expected);
assert.deepEqual(parseChangedEventFromLog(programDataLog(known)), expected);
assert.deepEqual(parseTransaction(known), [expected]);

const truncated = known.subarray(0, 2);
assert.throws(() => decodeChangedEvent(truncated), {
	name: "RangeError",
	message: /too short for the "ChangedEvent" event envelope/,
});
assert.equal(parseChangedEventFromLog(programDataLog(truncated)), null);
assert.throws(() => parseTransaction(truncated), {
	name: "RangeError",
	message: /log is too short for its version envelope/,
});

const unknown = Uint8Array.of(...discriminator, 8, 42);
assert.equal(parseChangedEventFromLog(programDataLog(unknown)), null);
assert.throws(() => parseTransaction(unknown), {
	name: "RangeError",
	message: /log carries migration version 8/,
});
