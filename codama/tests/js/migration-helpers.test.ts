import assert from "node:assert/strict";
import { test } from "node:test";

import { type AccountRole, address, generateKeyPairSigner } from "@solana/kit";
import {
	STATE_MIGRATION_VERSION,
	stateNeedsMigration,
} from "../../clients/js/migrations_program/src/generated/accounts/state.js";
import {
	decodeValueChangedEventEvent,
	decodeValueChangedEventV0Event,
	parseMigrationsProgramEventsFromLogs,
	parseValueChangedEventEventFromLog,
	parseValueChangedEventV0EventFromLog,
} from "../../clients/js/migrations_program/src/generated/events/logs.js";
import { getValueChangedEventEventDecoder } from "../../clients/js/migrations_program/src/generated/events/valueChangedEvent.js";
import {
	getMigrateDiscriminatorBytes,
	getMigrateInstruction,
	MIGRATE_DISCRIMINATOR,
} from "../../clients/js/migrations_program/src/generated/instructions/migrate.js";
import { MIGRATIONS_PROGRAM_PROGRAM_ADDRESS } from "../../clients/js/migrations_program/src/generated/programs/migrationsProgram.js";

// AccountRole is a bitfield: readonly = 0, writable = 1, writable signer = 3.
const READONLY = 0 satisfies AccountRole;
const WRITABLE = 1 satisfies AccountRole;
const WRITABLE_SIGNER = 3 satisfies AccountRole;

// Well-known 32-byte addresses used as stand-ins for the fixture slots.
const PROGRAM = MIGRATIONS_PROGRAM_PROGRAM_ADDRESS;
const PAYER = await generateKeyPairSigner();
const SYSTEM = address("11111111111111111111111111111111");
const STATE = address("So11111111111111111111111111111111111111112");
const MANUAL = address("MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr");
const COMPACT = address("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");
const FORK = address("9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin");

function staleStateBytes(version: number): Uint8Array {
	// [discriminator = 1, version, value(u64), ...]
	const data = new Uint8Array(10);
	data[0] = 1;
	data[1] = version;
	return data;
}

test("the reserved discriminator is the all-ones byte", () => {
	assert.equal(MIGRATE_DISCRIMINATOR, 255);
	assert.deepEqual([...getMigrateDiscriminatorBytes()], [255]);
});

test("needsMigration is true only for stale envelopes of this account type", () => {
	assert.equal(STATE_MIGRATION_VERSION, 2);
	assert.equal(stateNeedsMigration(staleStateBytes(0)), true);
	assert.equal(stateNeedsMigration(staleStateBytes(1)), true);
	// Current and future versions need no migration; the decoder explains
	// future versions to the caller.
	assert.equal(stateNeedsMigration(staleStateBytes(2)), false);
	assert.equal(stateNeedsMigration(staleStateBytes(3)), false);
	// A different discriminator is not this account type at all.
	const foreign = staleStateBytes(0);
	foreign[0] = 9;
	assert.equal(stateNeedsMigration(foreign), false);
	// Truncated envelopes fail closed.
	assert.equal(stateNeedsMigration(new Uint8Array(1)), false);
	assert.equal(stateNeedsMigration(new Uint8Array()), false);
});

test("migrate carries only the reserved discriminator as its payload", () => {
	const instruction = getMigrateInstruction({ state: STATE });
	assert.equal(instruction.programAddress, PROGRAM);
	assert.deepEqual([...instruction.data], [255]);
});

test("migrate fills omitted slots with the program address and truncates the tail", () => {
	const instruction = getMigrateInstruction({
		payer: PAYER,
		systemProgram: SYSTEM,
		state: STATE,
	});
	assert.deepEqual(
		instruction.accounts.map((meta) => ({
			address: meta.address,
			role: meta.role,
		})),
		[
			{ address: PAYER.address, role: WRITABLE_SIGNER },
			{ address: SYSTEM, role: READONLY },
			{ address: STATE, role: WRITABLE },
		],
	);

	// Slots between provided accounts stay as program-address placeholders;
	// the kit meta factory marks those readonly, which the program treats the
	// same as any other omitted slot.
	const gapped = getMigrateInstruction({ compactState: COMPACT });
	assert.deepEqual(
		gapped.accounts.map((meta) => meta.address),
		[PROGRAM, SYSTEM, PROGRAM, PROGRAM, COMPACT],
	);
	assert.deepEqual(
		gapped.accounts.map((meta) => meta.role),
		[READONLY, READONLY, READONLY, READONLY, WRITABLE],
	);

	// With everything provided, no slot is truncated.
	const full = getMigrateInstruction({
		payer: PAYER,
		systemProgram: SYSTEM,
		state: STATE,
		manualState: MANUAL,
		compactState: COMPACT,
	});
	assert.equal(full.accounts.length, 5);
});

test("migrate without a payer marks the payer slot as the program placeholder", () => {
	const instruction = getMigrateInstruction({ state: STATE });
	// Slot 1 must be the system program: the program rejects anything else
	// there, so it defaults to that address rather than the placeholder.
	assert.deepEqual(
		instruction.accounts.map((meta) => meta.address),
		[PROGRAM, SYSTEM, STATE],
	);

	// The payer and system program are always sent, even with nothing to
	// migrate, because the program requires both slots.
	const bare = getMigrateInstruction({});
	assert.deepEqual(
		bare.accounts.map((meta) => meta.address),
		[PROGRAM, SYSTEM],
	);
});

test("migrate honors a program address override", () => {
	const fork = FORK;
	const instruction = getMigrateInstruction(
		{ state: STATE },
		{ programAddress: fork },
	);
	assert.equal(instruction.programAddress, fork);
});

/** `[discriminator = 4][version][value(u64)]`, plus `[memo(u16)]` from v1. */
function valueChangedEventBytes(
	version: number,
	value: bigint,
	memo: number,
): Uint8Array {
	// Version zero carried only the u64 payload; later versions appended memo.
	const data = new Uint8Array(version === 0 ? 10 : 12);
	data[0] = 4;
	data[1] = version;
	const view = new DataView(data.buffer);
	view.setBigUint64(2, value, true);
	if (data.length === 12) {
		view.setUint16(10, memo, true);
	}
	return data;
}

function programDataLog(bytes: Uint8Array): string {
	return `Program data: ${Buffer.from(bytes).toString("base64")}`;
}

test("the generated event decoder enforces the current envelope", () => {
	const current = valueChangedEventBytes(1, 42n, 7);
	const decoded = getValueChangedEventEventDecoder().decode(current);
	assert.equal(decoded.discriminator, 4);
	assert.equal(decoded.migrationVersion, 1);
	assert.equal(decoded.value, 42n);
	assert.equal(decoded.memo, 7);

	const future = valueChangedEventBytes(2, 42n, 7);
	assert.throws(
		() => getValueChangedEventEventDecoder().decode(future),
		/upgrade this client/,
	);
	const stale = valueChangedEventBytes(0, 42n, 7);
	assert.throws(
		() => getValueChangedEventEventDecoder().decode(stale),
		/migration version mismatch/,
	);
});

test("a log written at an older version decodes with that version's event", () => {
	// Version zero carried only `value`; it is its own event, not a projection.
	const historical = decodeValueChangedEventV0Event(
		valueChangedEventBytes(0, 42n, 0),
	);
	assert.equal(historical.name, "valueChangedEventV0");
	assert.equal(historical.data.value, 42n);
	assert.equal(historical.data.migrationVersion, 0);
	assert.equal(historical.data.discriminator, 4);
	assert.equal("memo" in historical.data, false);

	const current = decodeValueChangedEventEvent(
		valueChangedEventBytes(1, 42n, 7),
	);
	assert.equal(current.name, "valueChangedEvent");
	assert.equal(current.data.migrationVersion, 1);
	assert.equal(current.data.memo, 7);
});

test("each event decodes only its own version and fails closed otherwise", () => {
	assert.throws(
		() => decodeValueChangedEventEvent(valueChangedEventBytes(0, 42n, 0)),
		/event migration version mismatch: expected 1, received 0/,
	);
	assert.throws(
		() => decodeValueChangedEventV0Event(valueChangedEventBytes(1, 42n, 7)),
		/event migration version mismatch: expected 0, received 1/,
	);
	assert.throws(
		() => decodeValueChangedEventEvent(valueChangedEventBytes(2, 42n, 7)),
		/regenerate this client/,
	);
	const truncated = new Uint8Array([4, 0, 1, 2, 3]);
	assert.throws(() => decodeValueChangedEventV0Event(truncated));
	const foreign = valueChangedEventBytes(0, 42n, 0);
	foreign[0] = 9;
	assert.throws(
		() => decodeValueChangedEventV0Event(foreign),
		/does not match the "ValueChangedEventV0Event" event discriminator/,
	);
	assert.throws(
		() => decodeValueChangedEventEvent(new Uint8Array([4])),
		/too short for the "ValueChangedEventEvent" event envelope/,
	);
});

test("Program data log lines decode through the event for their version", () => {
	const log = programDataLog(valueChangedEventBytes(0, 7n, 0));
	assert.equal(parseValueChangedEventEventFromLog(log), null);
	const parsed = parseValueChangedEventV0EventFromLog(log);
	assert.notEqual(parsed, null);
	assert.equal(parsed?.name, "valueChangedEventV0");
	assert.equal(parsed?.data.value, 7n);

	assert.equal(parseValueChangedEventEventFromLog("not a log line"), null);
	const otherProgram = programDataLog(new Uint8Array([9, 1, 0]));
	assert.equal(parseValueChangedEventEventFromLog(otherProgram), null);
	assert.equal(parseValueChangedEventV0EventFromLog(otherProgram), null);
});

test("the program log parser skips unrelated lines and keeps matching ones", () => {
	const events = parseMigrationsProgramEventsFromLogs([
		`Program ${PROGRAM} invoke [1]`,
		"Program log: Instruction: Update",
		programDataLog(valueChangedEventBytes(1, 5n, 3)),
		programDataLog(valueChangedEventBytes(0, 4n, 0)),
		programDataLog(new Uint8Array([9, 1, 0])),
		`Program ${PROGRAM} success`,
	]);
	assert.deepEqual(
		events.map((event) => [event.name, event.data.value]),
		[
			["valueChangedEvent", 5n],
			["valueChangedEventV0", 4n],
		],
	);
	const [current] = events;
	assert.ok(current?.name === "valueChangedEvent");
	assert.equal(current.data.memo, 3);
	assert.deepEqual(parseMigrationsProgramEventsFromLogs([]), []);
});

test("the program log parser rejects versions no generated event describes", () => {
	const frame = (line: string) => [
		`Program ${PROGRAM} invoke [1]`,
		line,
		`Program ${PROGRAM} success`,
	];
	assert.throws(
		() =>
			parseMigrationsProgramEventsFromLogs(
				frame(programDataLog(valueChangedEventBytes(2, 1n, 1))),
			),
		/log carries migration version 2, which this client cannot decode; regenerate it/,
	);
	assert.throws(
		() =>
			parseMigrationsProgramEventsFromLogs(
				frame(programDataLog(new Uint8Array([4]))),
			),
		/log is too short for its version envelope/,
	);
});

test("the program log parser only trusts lines this program emitted", () => {
	const spoofed = programDataLog(valueChangedEventBytes(1, 999n, 9));
	// A foreign program's future-version line would throw if it were decoded.
	const foreignFuture = programDataLog(valueChangedEventBytes(9, 1n, 1));
	const events = parseMigrationsProgramEventsFromLogs([
		// A data line outside any invocation frame is not attributable.
		spoofed,
		`Program ${PROGRAM} invoke [1]`,
		programDataLog(valueChangedEventBytes(1, 5n, 3)),
		// This program invokes another one, which forges this program's event.
		`Program ${FORK} invoke [2]`,
		spoofed,
		foreignFuture,
		`Program ${FORK} failed: custom program error: 0x1`,
		// Back in this program's frame after the inner call returns.
		programDataLog(valueChangedEventBytes(1, 6n, 4)),
		`Program ${PROGRAM} success`,
		// A later top-level instruction of another program.
		`Program ${FORK} invoke [1]`,
		spoofed,
		`Program ${FORK} success`,
	]);
	assert.deepEqual(
		events.map((event) => event.data.value),
		[5n, 6n],
	);

	// A caller that deployed the same program elsewhere names its address.
	const forked = parseMigrationsProgramEventsFromLogs(
		[`Program ${FORK} invoke [1]`, spoofed, `Program ${FORK} success`],
		FORK,
	);
	assert.deepEqual(
		forked.map((event) => event.data.value),
		[999n],
	);
});
