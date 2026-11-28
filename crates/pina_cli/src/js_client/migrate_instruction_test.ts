import assert from "node:assert/strict";

import {
	type AccountRole,
	type AccountSignerMeta,
	address,
	createNoopSigner,
	type ReadonlyAccount,
	type WritableAccount,
	type WritableSignerAccount,
} from "@solana/kit";
import {
	getMigrateInstruction,
	type MigrateInput,
} from "./instructions/migrate.js";
import { MIGRATIONS_PROGRAM_PROGRAM_ADDRESS as PROGRAM } from "./programs/index.js";

const PAYER_ADDRESS = "9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin";
const SYSTEM_ADDRESS = "11111111111111111111111111111111";
const STATE_ADDRESS = "So11111111111111111111111111111111111111112";
const MANUAL = address("MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr");
const COMPACT = address("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");

const payer = createNoopSigner(address(PAYER_ADDRESS));
const system = address(SYSTEM_ADDRESS);
const state = address(STATE_ADDRESS);
const readonly = 0 satisfies AccountRole;
const writable = 1 satisfies AccountRole;
const writableSigner = 3 satisfies AccountRole;

const funded = getMigrateInstruction({
	payer,
	systemProgram: system,
	state,
	manualState: MANUAL,
	compactState: COMPACT,
});
const [payerMeta, systemMeta, stateMeta] = funded.accounts satisfies readonly [
	& WritableSignerAccount<typeof PAYER_ADDRESS>
	& AccountSignerMeta<typeof PAYER_ADDRESS>,
	ReadonlyAccount<typeof SYSTEM_ADDRESS>,
	WritableAccount<typeof STATE_ADDRESS>,
	...readonly WritableAccount<string>[],
];
assert.equal(payerMeta.signer, payer);
assert.equal(systemMeta.address, system);
assert.equal(stateMeta.address, state);
assert.equal(funded.programAddress, PROGRAM);
assert.deepEqual(funded.data, Uint8Array.of(255));
assert.deepEqual(funded.accounts.map(({ address, role }) => [address, role]), [
	[payer.address, writableSigner],
	[system, readonly],
	[state, writable],
	[MANUAL, writable],
	[COMPACT, writable],
]);

const gapped = getMigrateInstruction({ manualState: MANUAL }, {
	programAddress: payer.address,
});
assert.equal(gapped.programAddress, payer.address);
assert.deepEqual(gapped.accounts.map(({ address, role }) => [address, role]), [
	[payer.address, readonly],
	[system, readonly],
	[payer.address, readonly],
	[MANUAL, writable],
]);

const omitted = getMigrateInstruction({} satisfies MigrateInput);
assert.deepEqual(omitted.accounts.map(({ address, role }) => [address, role]), [
	[PROGRAM, readonly],
	[system, readonly],
]);
