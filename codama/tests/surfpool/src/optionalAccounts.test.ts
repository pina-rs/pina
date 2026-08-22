/**
 * Surfpool end-to-end tests for optional account slots.
 *
 * These tests deploy the compiled SBF program into Surfpool and lock in the
 * on-chain half of the optional-account contract using raw instructions built
 * exactly the way the generated Codama clients emit them:
 *
 * 1. Omitted optional accounts keep the account count fixed by filling the
 *    slot with a readonly meta pointing at the executing program's address.
 * 2. On-chain, that filler parses as `None` and the instruction succeeds.
 * 3. Optional mutable accounts mutate state only when provided.
 * 4. Optional signers must sign when provided, and may be omitted otherwise.
 * 5. Optional readonly accounts accept arbitrary addresses when provided.
 *
 * The client-side emission contract (optional inputs, readonly filler metas,
 * parse-side `undefined` mapping) is covered in
 * `codama/tests/litesvm/src/optionalAccounts.test.ts`, which imports the
 * generated JavaScript client directly.
 */

import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

import {
	AccountRole,
	address,
	addSignersToInstruction,
	appendTransactionMessageInstruction,
	createKeyPairSignerFromBytes,
	createSolanaRpc,
	createTransactionMessage,
	generateKeyPairSigner,
	getAddressEncoder,
	getProgramDerivedAddress,
	type Instruction,
	sendTransactionWithoutConfirmingFactory,
	setTransactionMessageFeePayerSigner,
	setTransactionMessageLifetimeUsingBlockhash,
	signTransactionMessageWithSigners,
} from "@solana/kit";
import { Surfnet } from "@solana/surfpool";

const ROOT = resolve(import.meta.dirname, "../../../..");
const SBF_OUT_DIR = process.env.SBF_OUT_DIR ??
	resolve(ROOT, "target/surfpool/examples");
// Mirrors examples/optional_accounts_program/src/lib.rs declare_id!.
const PROGRAM_ID = "ccdMMVpwebk8NxwJdY4CndxkLKUTM6fkaFUteAfFeci";

type TestSigner = Awaited<ReturnType<typeof createKeyPairSignerFromBytes>>;

function artifactPath(name: string): string {
	const direct = resolve(SBF_OUT_DIR, `${name}.so`);
	const library = resolve(SBF_OUT_DIR, `lib${name}.so`);
	const artifact = existsSync(direct) ? direct : library;
	assert.ok(existsSync(artifact), `missing ${name} Surfpool artifact`);

	return artifact;
}

async function createSubmitter(surfnet: Surfnet): Promise<{
	payer: TestSigner;
	submit(instruction: Instruction): Promise<void>;
}> {
	const payer = await createKeyPairSignerFromBytes(surfnet.payerSecretKey);
	const rpc = createSolanaRpc(surfnet.rpcUrl);
	const sendTransaction = sendTransactionWithoutConfirmingFactory({ rpc });

	return {
		payer,
		async submit(instruction: Instruction): Promise<void> {
			const { value: latestBlockhash } = await rpc.getLatestBlockhash().send();
			const message = setTransactionMessageLifetimeUsingBlockhash(
				latestBlockhash,
				appendTransactionMessageInstruction(
					instruction,
					setTransactionMessageFeePayerSigner(
						payer,
						createTransactionMessage({ version: 0 }),
					),
				),
			);
			const signed = await signTransactionMessageWithSigners(message);
			await sendTransaction(signed, { commitment: "confirmed" });
		},
	};
}

async function withSurfnet(
	operation: (surfnet: Surfnet) => Promise<void>,
): Promise<void> {
	const surfnet = Surfnet.start();

	try {
		surfnet.deploy({
			programId: address(PROGRAM_ID),
			soPath: artifactPath("optional_accounts_program"),
		});
		await operation(surfnet);
	} finally {
		surfnet.stop();
	}
}

async function deriveStore(authority: string): Promise<string> {
	const [store] = await getProgramDerivedAddress({
		programAddress: address(PROGRAM_ID),
		seeds: [
			new TextEncoder().encode("store"),
			getAddressEncoder().encode(address(authority)),
		],
	});
	return store;
}

/** Instruction data is discriminator-only except `init` (discriminator +
 * bump). */
function instructionData(discriminator: number, bump?: number): Uint8Array {
	if (bump === undefined) return Uint8Array.of(discriminator);
	return Uint8Array.of(discriminator, bump);
}

function touchInstruction(
	authority: string,
	store: string | null,
): Instruction {
	return {
		programAddress: address(PROGRAM_ID),
		data: instructionData(1),
		accounts: [
			{ address: address(authority), role: AccountRole.READONLY_SIGNER },
			store === null
				? { address: address(PROGRAM_ID), role: AccountRole.READONLY }
				: { address: address(store), role: AccountRole.WRITABLE },
		],
	};
}

test("omitted optional slots keep the account count fixed and parse as None", async () => {
	await withSurfnet(async (surfnet) => {
		const rpc = createSolanaRpc(surfnet.rpcUrl);
		const { payer, submit } = await createSubmitter(surfnet);
		const authority = String(payer.address);
		const store = await deriveStore(authority);

		// Init: authority (writable signer), store (writable), system program.
		const [, bump] = await getProgramDerivedAddress({
			programAddress: address(PROGRAM_ID),
			seeds: [
				new TextEncoder().encode("store"),
				getAddressEncoder().encode(address(authority)),
			],
		});
		await submit({
			programAddress: address(PROGRAM_ID),
			data: Uint8Array.of(0, bump),
			accounts: [
				{ address: address(authority), role: AccountRole.WRITABLE_SIGNER },
				{ address: address(store), role: AccountRole.WRITABLE },
				{
					address: address("11111111111111111111111111111111"),
					role: AccountRole.READONLY,
				},
			],
		});

		// Touch with the store omitted: two metas, filler readonly.
		await submit(touchInstruction(authority, null));

		const absent = await rpc.getAccountInfo(address(store), {
			encoding: "base64",
		}).send();
		assert.ok(absent.value, "store account must exist");
		const [encoded] = absent.value.data;
		const bytes = new Uint8Array(Buffer.from(encoded, "base64"));
		// Layout: 1 discriminator + 1 bump + u64 count. Count stays zero when
		// the optional slot was filled with the program address.
		const view = new DataView(bytes.buffer, bytes.byteOffset);
		assert.equal(view.getBigUint64(2, true), 0n);

		// Touch with the store present increments on-chain.
		await submit(touchInstruction(authority, store));
		const touched = await rpc.getAccountInfo(address(store), {
			encoding: "base64",
		}).send();
		assert.ok(touched.value);
		const [touchedEncoded] = touched.value.data;
		const touchedBytes = new Uint8Array(
			Buffer.from(touchedEncoded, "base64"),
		);
		const touchedView = new DataView(
			touchedBytes.buffer,
			touchedBytes.byteOffset,
		);
		assert.equal(touchedView.getBigUint64(2, true), 1n);
	});
});

test("optional witness must sign when provided and may be omitted", async () => {
	await withSurfnet(async (surfnet) => {
		const { payer, submit } = await createSubmitter(surfnet);
		const authority = String(payer.address);

		// Inspect with both optionals omitted succeeds.
		await submit({
			programAddress: address(PROGRAM_ID),
			data: instructionData(2),
			accounts: [
				{ address: address(authority), role: AccountRole.READONLY_SIGNER },
				{ address: address(PROGRAM_ID), role: AccountRole.READONLY },
				{ address: address(PROGRAM_ID), role: AccountRole.READONLY },
			],
		});

		// A signed witness passes validation.
		const witness = await generateKeyPairSigner();
		const signedIx: Instruction = addSignersToInstruction(
			[witness],
			{
				programAddress: address(PROGRAM_ID),
				data: instructionData(2),
				accounts: [
					{ address: address(authority), role: AccountRole.READONLY_SIGNER },
					{ address: address(PROGRAM_ID), role: AccountRole.READONLY },
					{ address: witness.address, role: AccountRole.READONLY_SIGNER },
				],
			},
		);
		await submit(signedIx);

		// An unsigned witness fails with MissingRequiredSignature.
		const unsignedWitness = address(Surfnet.newKeypair().publicKey);
		const unsignedIx: Instruction = {
			programAddress: address(PROGRAM_ID),
			data: instructionData(2),
			accounts: [
				{ address: address(authority), role: AccountRole.READONLY_SIGNER },
				{ address: address(PROGRAM_ID), role: AccountRole.READONLY },
				{ address: address(unsignedWitness), role: AccountRole.READONLY },
			],
		};
		await assert.rejects(submit(unsignedIx), (error: unknown) => {
			assert.ok(error instanceof Error);

			// Surfpool wraps runtime failures: the instruction error code
			// (4615008 == MissingRequiredSignature) lives somewhere in the
			// cause chain rather than on the top-level message.
			let cause: unknown = (error as { cause?: unknown }).cause;
			let rendered = "";
			while (cause !== undefined && cause !== null) {
				rendered += String(cause);
				rendered += JSON.stringify(cause) ?? "";
				cause = (cause as { cause?: unknown }).cause;
			}
			assert.match(rendered, /4615008/);
			return true;
		});
	});
});

test("optional note accepts arbitrary readonly accounts or nothing", async () => {
	await withSurfnet(async (surfnet) => {
		const { payer, submit } = await createSubmitter(surfnet);
		const authority = String(payer.address);
		const note = address(Surfnet.newKeypair().publicKey);

		await submit({
			programAddress: address(PROGRAM_ID),
			data: instructionData(3),
			accounts: [
				{ address: address(authority), role: AccountRole.READONLY_SIGNER },
				{ address: note, role: AccountRole.READONLY },
			],
		});

		await submit({
			programAddress: address(PROGRAM_ID),
			data: instructionData(3),
			accounts: [
				{ address: address(authority), role: AccountRole.READONLY_SIGNER },
				{ address: address(PROGRAM_ID), role: AccountRole.READONLY },
			],
		});
	});
});
