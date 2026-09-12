import assert from "node:assert/strict";
import { test } from "node:test";

import { type AccountRole, address, generateKeyPairSigner } from "@solana/kit";
import {
	STATE_MIGRATION_VERSION,
	stateNeedsMigration,
} from "../../clients/js/migrations_program/src/generated/accounts/state.js";
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
