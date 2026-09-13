import assert from "node:assert/strict";
import { test } from "node:test";

import { type AccountRole, address, generateKeyPairSigner } from "@solana/kit";
import {
	STATE_MIGRATION_VERSION,
	stateNeedsMigration,
} from "../../clients/js/migrations_program/src/generated/accounts/state.js";
import {
	normalizeValueChangedEventEvent,
	parseMigrationsProgramEventsFromLogs,
	parseValueChangedEventEventFromLog,
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
		[PROGRAM, PROGRAM, PROGRAM, PROGRAM, COMPACT],
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
	assert.deepEqual(
		instruction.accounts.map((meta) => meta.address),
		[PROGRAM, PROGRAM, STATE],
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

test("a log written at an older version projects into the current shape", () => {
	// Version zero carried only `value`; the projection zero-fills `memo`.
	const historical = valueChangedEventBytes(0, 42n, 0);
	const normalized = normalizeValueChangedEventEvent(historical);

	assert.equal(normalized.name, "valueChangedEvent");
	assert.equal(normalized.sourceVersion, 0);
	assert.equal(normalized.wasMigrated, true);
	assert.equal(normalized.data.value, 42n);
	assert.equal(normalized.data.memo, 0);
	assert.equal(normalized.data.migrationVersion, 1);
	assert.equal(normalized.data.discriminator, 4);

	const current = normalizeValueChangedEventEvent(
		valueChangedEventBytes(1, 42n, 7),
	);
	assert.equal(current.sourceVersion, 1);
	assert.equal(current.wasMigrated, false);
	assert.equal(current.data.memo, 7);
});

test("unknown, future, and malformed event logs fail closed", () => {
	assert.throws(
		() => normalizeValueChangedEventEvent(valueChangedEventBytes(2, 42n, 7)),
		/log was written by a newer program; upgrade this client/,
	);
	const truncated = new Uint8Array([4, 0, 1, 2, 3]);
	assert.throws(
		() => normalizeValueChangedEventEvent(truncated),
		/log length does not match the v0 schema/,
	);
	const foreign = valueChangedEventBytes(0, 42n, 0);
	foreign[0] = 9;
	assert.throws(
		() => normalizeValueChangedEventEvent(foreign),
		/does not match the "ValueChangedEventEvent" event discriminator/,
	);
	assert.throws(
		() => normalizeValueChangedEventEvent(new Uint8Array([4])),
		/too short for the "ValueChangedEventEvent" event envelope/,
	);
});

test("Program data log lines decode through the event entry point", () => {
	const log = programDataLog(valueChangedEventBytes(0, 7n, 0));
	const parsed = parseValueChangedEventEventFromLog(log);
	assert.notEqual(parsed, null);
	assert.equal(parsed?.sourceVersion, 0);
	assert.equal(parsed?.wasMigrated, true);
	assert.equal(parsed?.data.value, 7n);

	assert.equal(parseValueChangedEventEventFromLog("not a log line"), null);
	const otherProgram = programDataLog(new Uint8Array([9, 1, 0]));
	assert.equal(parseValueChangedEventEventFromLog(otherProgram), null);
});

test("the program log parser skips unrelated lines and keeps matching ones", () => {
	const events = parseMigrationsProgramEventsFromLogs([
		"Program log: Instruction: Update",
		programDataLog(valueChangedEventBytes(1, 5n, 3)),
		programDataLog(new Uint8Array([9, 1, 0])),
	]);
	assert.equal(events.length, 1);
	assert.equal(events[0]?.name, "valueChangedEvent");
	assert.equal(events[0]?.data.memo, 3);
	assert.deepEqual(parseMigrationsProgramEventsFromLogs([]), []);
});
