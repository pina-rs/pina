/**
 * LiteSVM end-to-end tests for optional account slots.
 *
 * These tests load the compiled SBF binary into LiteSVM and drive the
 * generated JS client builders through every optional-account combination:
 *
 * - optional mutable account (provided / omitted)
 * - optional immutable account (provided / omitted)
 * - optional signer (signed / unsigned / omitted)
 * - optional non-signer (arbitrary address / omitted)
 *
 * Omitted slots must keep the account count fixed by emitting a readonly
 * program-address filler, which the program parses as `None`.
 */

import {
	AccountRole,
	type Address,
	address,
	assertAccountExists,
	generateKeyPairSigner,
	getAddressEncoder,
	getProgramDerivedAddress,
	getUtf8Encoder,
	type KeyPairSigner,
} from "@solana/kit";
import { FailedTransactionMetadata, LiteSVM } from "litesvm";
import { describe, expect, test } from "vitest";
import { getStoreStateCodec } from "../../../clients/js/optional_accounts_program/src/generated/accounts";

import {
	getInitInstructionAsync,
	getInspectInstruction,
	getNoteInstruction,
	getTouchInstruction,
	parseInspectInstruction,
} from "../../../clients/js/optional_accounts_program/src/generated/instructions";
import { OPTIONAL_ACCOUNTS_PROGRAM_PROGRAM_ADDRESS } from "../../../clients/js/optional_accounts_program/src/generated/programs";
import { airdrop, buildAndSignTransaction, findProgramBinary } from "./helpers";

const PROGRAM_NAME = "optional_accounts_program";
const PROGRAM_ADDRESS = OPTIONAL_ACCOUNTS_PROGRAM_PROGRAM_ADDRESS;

function loadProgram(): { svm: LiteSVM } {
	const soPath = findProgramBinary(PROGRAM_NAME);
	if (!soPath) {
		throw new Error(
			`${PROGRAM_NAME}.so not found; build SBF binaries before running this suite`,
		);
	}

	const svm = new LiteSVM();
	svm.addProgramFromFile(PROGRAM_ADDRESS, soPath);
	return { svm };
}

async function deriveStorePda(authority: Address) {
	return await getProgramDerivedAddress({
		programAddress: PROGRAM_ADDRESS,
		seeds: [
			getUtf8Encoder().encode("store"),
			getAddressEncoder().encode(authority),
		],
	});
}

function initInstruction(
	authority: KeyPairSigner,
	storePda: Address,
	storeBump: number,
) {
	return getInitInstructionAsync({
		authority,
		store: storePda,
		bump: storeBump,
	});
}

describe("optional_accounts_program e2e", () => {
	test("parses omitted slots against a custom program address", async () => {
		const authority = await generateKeyPairSigner();
		const customProgramAddress = address("11111111111111111111111111111111");
		const instruction = getInspectInstruction(
			{ authority },
			{ programAddress: customProgramAddress },
		);
		const parsed = parseInspectInstruction(instruction);

		expect(parsed.programAddress).toBe(customProgramAddress);
		expect(parsed.accounts.store).toBeUndefined();
		expect(parsed.accounts.witness).toBeUndefined();
	});

	test("init creates the store PDA with the required baseline layout", async () => {
		const { svm } = loadProgram();

		const authority = await generateKeyPairSigner();
		airdrop(svm, authority.address);
		const [storePda, storeBump] = await deriveStorePda(authority.address);

		const initIx = await initInstruction(authority, storePda, storeBump);
		expect(initIx.accounts).toHaveLength(3);

		const tx = await buildAndSignTransaction(svm, authority, [initIx]);
		svm.sendTransaction(tx);

		const account = svm.getAccount(storePda);
		assertAccountExists(account);
		const state = getStoreStateCodec().decode(account.data);
		expect(state.count).toBe(0n);
	});

	test("touch: omitted optional mutable slot parses as None; provided slot increments", async () => {
		const { svm } = loadProgram();

		const authority = await generateKeyPairSigner();
		airdrop(svm, authority.address);
		const [storePda, storeBump] = await deriveStorePda(authority.address);

		svm.sendTransaction(
			await buildAndSignTransaction(svm, authority, [
				await initInstruction(authority, storePda, storeBump),
			]),
		);

		// Omitted: fixed two-account layout with a readonly program filler.
		const absentIx = getTouchInstruction({ authority });
		expect(absentIx.accounts).toHaveLength(2);
		expect(absentIx.accounts[1].address).toBe(PROGRAM_ADDRESS);
		svm.sendTransaction(
			await buildAndSignTransaction(svm, authority, [absentIx]),
		);
		let account = svm.getAccount(storePda);
		assertAccountExists(account);
		expect(getStoreStateCodec().decode(account.data).count).toBe(0n);

		// Provided: writable meta and an on-chain increment.
		const providedIx = getTouchInstruction({
			authority,
			store: storePda,
		});
		expect(providedIx.accounts[1].role).toBe(AccountRole.WRITABLE);
		svm.sendTransaction(
			await buildAndSignTransaction(svm, authority, [providedIx]),
		);
		account = svm.getAccount(storePda);
		assertAccountExists(account);
		expect(getStoreStateCodec().decode(account.data).count).toBe(1n);
	});

	test("inspect: optional signer enforced when present, skippable when omitted", async () => {
		const { svm } = loadProgram();

		const authority = await generateKeyPairSigner();
		const witness = await generateKeyPairSigner();
		airdrop(svm, authority.address);
		airdrop(svm, witness.address);
		const [storePda, storeBump] = await deriveStorePda(authority.address);

		svm.sendTransaction(
			await buildAndSignTransaction(svm, authority, [
				await initInstruction(authority, storePda, storeBump),
			]),
		);

		// Both optionals omitted.
		svm.sendTransaction(
			await buildAndSignTransaction(svm, authority, [
				getInspectInstruction({ authority }),
			]),
		);

		// Optional immutable store + signed witness.
		svm.sendTransaction(
			await buildAndSignTransaction(svm, authority, [
				getInspectInstruction({
					authority,
					store: storePda,
					witness,
				}),
			]),
		);

		// An unsigned witness present in the meta list must be rejected.
		// LiteSVM reports program failures as metadata instead of throwing.
		const unsignedWitness = await generateKeyPairSigner();
		const baseIx = getInspectInstruction({ authority, store: storePda });
		const withUnsignedWitness = {
			...baseIx,
			accounts: [
				baseIx.accounts[0],
				baseIx.accounts[1],
				{
					address: unsignedWitness.address,
					role: AccountRole.READONLY,
				},
			],
		};
		const result = svm.sendTransaction(
			await buildAndSignTransaction(svm, authority, [withUnsignedWitness]),
		);
		expect(result).toBeInstanceOf(FailedTransactionMetadata);
		if (!(result instanceof FailedTransactionMetadata)) {
			throw new Error("unsigned witness transaction unexpectedly succeeded");
		}
		expect(result.err().toString()).toContain("MissingRequiredSignature");
	});

	test("note: arbitrary readonly account accepted when provided", async () => {
		const { svm } = loadProgram();

		const authority = await generateKeyPairSigner();
		const noteHolder = await generateKeyPairSigner();
		airdrop(svm, authority.address);

		svm.sendTransaction(
			await buildAndSignTransaction(svm, authority, [
				getNoteInstruction({
					authority,
					note: noteHolder.address,
				}),
			]),
		);

		svm.sendTransaction(
			await buildAndSignTransaction(svm, authority, [
				getNoteInstruction({ authority }),
			]),
		);
	});
});
