import assert from "node:assert/strict";
import { subtle } from "node:crypto";
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
	createSolanaRpcSubscriptions,
	createTransactionMessage,
	getAddressCodec,
	getAddressDecoder,
	getAddressEncoder,
	getBase64EncodedWireTransaction,
	getProgramDerivedAddress,
	type Instruction,
	isOffCurveAddress,
	sendAndConfirmTransactionFactory,
	setTransactionMessageFeePayerSigner,
	setTransactionMessageLifetimeUsingBlockhash,
	signTransactionMessageWithSigners,
} from "@solana/kit";
import { Surfnet } from "@solana/surfpool";

const ROOT = resolve(import.meta.dirname, "../../../..");
const SBF_OUT_DIR = process.env.SBF_OUT_DIR ??
	resolve(ROOT, "target/surfpool/examples");
const PINA_BPF_PROGRAM_ID = "2nYtoevJCC8AFjdsfmkf8y1jN2nN9k4jVtD7G3f5n1Qe";
const PROP_AMM_PROGRAM_ID = "55555555555555555555555555555555555555555555";
const SYSTEM_PROGRAM_ID = "11111111111111111111111111111111";

/// The byte string 'ProgramDerivedAddress', appended to every PDA preimage.
const PDA_MARKER = new TextEncoder().encode("ProgramDerivedAddress");

type TestSigner = Awaited<ReturnType<typeof createKeyPairSignerFromBytes>>;

function artifactPath(name: string): string {
	const direct = resolve(SBF_OUT_DIR, `${name}.so`);
	const library = resolve(SBF_OUT_DIR, `lib${name}.so`);
	const artifact = existsSync(direct) ? direct : library;
	assert.ok(existsSync(artifact), `missing ${name} Surfpool artifact`);

	return artifact;
}

function rawInstruction(
	programId: string,
	data: Uint8Array,
	accounts: Instruction["accounts"],
): Instruction {
	return {
		programAddress: address(programId),
		accounts,
		data,
	};
}

function instructionData(
	discriminator: number,
	newAuthority?: string,
	bump?: number,
): Uint8Array {
	const addressBytes = newAuthority
		? getAddressEncoder().encode(address(newAuthority))
		: new Uint8Array();
	const bumpBytes = bump === undefined ? new Uint8Array() : Uint8Array.of(bump);
	// The envelope inserts the migration version byte after the discriminator.
	const data = new Uint8Array(2 + bumpBytes.length + addressBytes.length);
	data[0] = discriminator;
	data[1] = 0;
	data.set(bumpBytes, 2);
	data.set(addressBytes, 2 + bumpBytes.length);

	return data;
}

async function createSubmitter(surfnet: Surfnet): Promise<{
	payer: TestSigner;
	submit(instruction: Instruction): Promise<void>;
}> {
	const payer = await createKeyPairSignerFromBytes(surfnet.payerSecretKey);
	const rpc = createSolanaRpc(surfnet.rpcUrl);
	const rpcSubscriptions = createSolanaRpcSubscriptions(surfnet.wsUrl);
	const sendAndConfirmTransaction = sendAndConfirmTransactionFactory({
		rpc,
		rpcSubscriptions,
	});

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
			const confirmable = signed as Parameters<
				typeof sendAndConfirmTransaction
			>[0];
			const simulated = await rpc.simulateTransaction(
				getBase64EncodedWireTransaction(signed),
				{ encoding: "base64", sigVerify: true },
			).send();

			if (simulated.value.err !== null) {
				throw Object.assign(new Error("program execution failed"), {
					programError: simulated.value.err,
					logs: simulated.value.logs ?? [],
				});
			}

			await sendAndConfirmTransaction(confirmable, { commitment: "confirmed" });
		},
	};
}

function instructionError(value: unknown): unknown {
	if (typeof value !== "object" || value === null) return undefined;
	const instructionError = (value as Record<string, unknown>).InstructionError;
	if (!Array.isArray(instructionError) || instructionError.length !== 2) {
		return undefined;
	}

	return instructionError[1];
}

async function assertProgramError(
	operation: () => Promise<void>,
	expected: string,
): Promise<void> {
	let caught: unknown;

	try {
		await operation();
	} catch (error) {
		caught = error;
	}

	assert.ok(
		caught instanceof Error,
		`expected ${expected}, but invocation succeeded`,
	);
	const programError =
		(caught as Error & { programError?: unknown }).programError;
	assert.deepEqual(instructionError(programError), expected);
}

async function fetchAccountData(
	surfnet: Surfnet,
	accountAddress: string,
): Promise<Uint8Array> {
	const rpc = createSolanaRpc(surfnet.rpcUrl);
	const { value: account } = await rpc.getAccountInfo(
		address(accountAddress),
		{ encoding: "base64" },
	).send();
	assert.ok(account, `account ${accountAddress} was not created`);
	const [encoded] = account.data;

	return new Uint8Array(Buffer.from(encoded, "base64"));
}

function deployCpiPrograms(surfnet: Surfnet): void {
	surfnet.deploy({
		programId: PINA_BPF_PROGRAM_ID,
		soPath: artifactPath("pina_bpf_program"),
	});
	surfnet.deploy({
		programId: PROP_AMM_PROGRAM_ID,
		soPath: artifactPath("prop_amm_program"),
	});
}

function createOracle(surfnet: Surfnet, authority: string): string {
	const oracle = Surfnet.newKeypair().publicKey;
	const data = new Uint8Array(2 + 32 + 8);
	data[0] = 1;
	data[1] = 0;
	data.set(getAddressEncoder().encode(address(authority)), 2);
	surfnet.setAccount(
		oracle,
		1_000_000,
		data,
		PROP_AMM_PROGRAM_ID,
	);

	return oracle;
}

function proxyRotateInstruction(
	discriminator: number,
	oracle: string,
	authority: string,
	newAuthority: string,
	bump?: number,
): Instruction {
	return rawInstruction(
		PINA_BPF_PROGRAM_ID,
		instructionData(discriminator, newAuthority, bump),
		[
			{ address: address(oracle), role: AccountRole.WRITABLE },
			{
				address: address(authority),
				role: discriminator === 1
					? AccountRole.READONLY_SIGNER
					: AccountRole.READONLY,
			},
			{ address: address(PROP_AMM_PROGRAM_ID), role: AccountRole.READONLY },
		],
	);
}

async function withSurfnet(
	operation: (surfnet: Surfnet) => Promise<void>,
): Promise<void> {
	const surfnet = Surfnet.start();

	try {
		deployCpiPrograms(surfnet);
		await operation(surfnet);
	} finally {
		surfnet.stop();
	}
}

test("generated CPI preserves a transaction signer requirement", async () => {
	await withSurfnet(async (surfnet) => {
		const { payer, submit } = await createSubmitter(surfnet);
		const oracle = createOracle(surfnet, String(payer.address));
		const nextAuthority = Surfnet.newKeypair().publicKey;

		await submit(proxyRotateInstruction(
			1,
			oracle,
			String(payer.address),
			nextAuthority,
		));

		const data = await fetchAccountData(surfnet, oracle);
		assert.deepEqual(
			data.slice(2, 34),
			getAddressEncoder().encode(address(nextAuthority)),
		);
	});
});

test("generated CPI lets invoke_signed satisfy a PDA signer requirement", async () => {
	await withSurfnet(async (surfnet) => {
		const { payer, submit } = await createSubmitter(surfnet);
		const oracle = createOracle(surfnet, String(payer.address));
		const [proxyAuthority, bump] = await getProgramDerivedAddress({
			programAddress: address(PINA_BPF_PROGRAM_ID),
			seeds: ["cpi-authority"],
		});
		surfnet.setAccount(
			String(proxyAuthority),
			1_000_000,
			new Uint8Array(),
			PINA_BPF_PROGRAM_ID,
		);

		await submit(proxyRotateInstruction(
			1,
			oracle,
			String(payer.address),
			String(proxyAuthority),
		));

		const nextAuthority = Surfnet.newKeypair().publicKey;
		await submit(proxyRotateInstruction(
			2,
			oracle,
			String(proxyAuthority),
			nextAuthority,
			bump,
		));

		const data = await fetchAccountData(surfnet, oracle);
		assert.deepEqual(
			data.slice(2, 34),
			getAddressEncoder().encode(address(nextAuthority)),
		);
	});
});

async function deriveStatePda(): Promise<readonly [string, number]> {
	const [state, bump] = await getProgramDerivedAddress({
		programAddress: address(PINA_BPF_PROGRAM_ID),
		seeds: ["state"],
	});

	return [String(state), bump];
}

/**
 * Derive the address for exactly these seeds, without searching for a bump.
 *
 * This is `create_program_address` rather than `getProgramDerivedAddress`: the
 * caller supplies the bump, the derivation runs once, and the result is
 * rejected only when it lands on the Ed25519 curve. `getProgramDerivedAddress`
 * cannot express this, because it searches from 255 down and always returns the
 * canonical bump.
 */
async function createProgramAddress(
	seeds: readonly (string | Uint8Array)[],
	programAddress: string,
): Promise<string> {
	const addressCodec = getAddressCodec();
	const addressDecoder = getAddressDecoder();
	const textEncoder = new TextEncoder();
	const seedBytes: number[] = [];
	for (const seed of seeds) {
		// A string seed is literal seed bytes, not an address; only the program
		// address is base58-decoded.
		seedBytes.push(
			...(typeof seed === "string" ? textEncoder.encode(seed) : seed),
		);
	}

	const digest = await subtle.digest(
		"SHA-256",
		new Uint8Array([
			...seedBytes,
			...addressCodec.encode(address(programAddress)),
			...PDA_MARKER,
		]),
	);
	const candidate = addressDecoder.decode(new Uint8Array(digest));
	// A PDA must not fall on the Ed25519 curve. `getProgramDerivedAddress`
	// rejects an on-curve candidate and moves to the next bump down; supplying
	// the bump directly makes that rejection the caller's failure.
	assert.ok(
		isOffCurveAddress(candidate),
		"seed combination lands on the Ed25519 curve",
	);

	return String(candidate);
}

/**
 * Find a bump that yields a valid PDA without being the canonical one.
 *
 * Every bump `b` where `create_program_address(seeds, b)` succeeds yields a
 * distinct address for the same seeds. Canonical derivation returns only the
 * highest such bump, so a lower one produces a shadow account that seeded
 * lookups never find.
 */
async function deriveShadowStatePda(): Promise<readonly [string, number]> {
	const [canonicalAddress, canonical] = await deriveStatePda();
	// Tie this helper to the canonical derivation so a drift in the preimage,
	// the marker, or the curve check cannot quietly turn the shadow test into a
	// test of a made-up address.
	assert.equal(
		await createProgramAddress(
			["state", Uint8Array.of(canonical)],
			PINA_BPF_PROGRAM_ID,
		),
		canonicalAddress,
		"create_program_address must reproduce the canonical address",
	);

	for (let bump = canonical - 1; bump >= 0; bump -= 1) {
		try {
			const state = await createProgramAddress(
				["state", Uint8Array.of(bump)],
				PINA_BPF_PROGRAM_ID,
			);

			return [state, bump];
		} catch {
			// Not a valid bump for these seeds; try the next one down.
		}
	}

	throw new Error("state seeds have no noncanonical valid bump");
}

function createPdaInstruction(
	payer: string,
	state: string,
	bump: number,
	signer: boolean,
): Instruction {
	return rawInstruction(
		PINA_BPF_PROGRAM_ID,
		instructionData(3, undefined, bump),
		[
			{ address: address(payer), role: AccountRole.WRITABLE_SIGNER },
			{
				address: address(state),
				role: signer ? AccountRole.WRITABLE_SIGNER : AccountRole.WRITABLE,
			},
			{ address: address(SYSTEM_PROGRAM_ID), role: AccountRole.READONLY },
		],
	);
}

test("PDA creation accepts the canonical target", async () => {
	await withSurfnet(async (surfnet) => {
		const { payer, submit } = await createSubmitter(surfnet);
		const [state, bump] = await deriveStatePda();

		await submit(
			createPdaInstruction(String(payer.address), state, bump, false),
		);
		const data = await fetchAccountData(surfnet, state);
		assert.equal(data[0], 1);
		assert.equal(data[1], 0);
		assert.equal(data[2], bump);
	});
});

test("PDA creation rejects a separately signing zero-balance target", async () => {
	await withSurfnet(async (surfnet) => {
		const { payer, submit } = await createSubmitter(surfnet);
		const target = await createKeyPairSignerFromBytes(
			new Uint8Array(Surfnet.newKeypair().secretKey),
		);
		const [, bump] = await deriveStatePda();
		const instruction = createPdaInstruction(
			String(payer.address),
			String(target.address),
			bump,
			true,
		);

		await assertProgramError(
			() => submit(addSignersToInstruction([target], instruction)),
			"InvalidSeeds",
		);
	});
});

test("PDA creation rejects a separately signing prefunded target", async () => {
	await withSurfnet(async (surfnet) => {
		const { payer, submit } = await createSubmitter(surfnet);
		const target = await createKeyPairSignerFromBytes(
			new Uint8Array(Surfnet.newKeypair().secretKey),
		);
		surfnet.fundSol(String(target.address), 1_000_000);
		const [, bump] = await deriveStatePda();
		const instruction = createPdaInstruction(
			String(payer.address),
			String(target.address),
			bump,
			true,
		);

		await assertProgramError(
			() => submit(addSignersToInstruction([target], instruction)),
			"InvalidSeeds",
		);
	});
});

test("PDA creation rejects the canonical target with a wrong bump", async () => {
	await withSurfnet(async (surfnet) => {
		const { payer, submit } = await createSubmitter(surfnet);
		const [state, bump] = await deriveStatePda();
		const wrongBump = bump === 0 ? 1 : bump - 1;

		await assertProgramError(
			() =>
				submit(createPdaInstruction(
					String(payer.address),
					state,
					wrongBump,
					false,
				)),
			"InvalidSeeds",
		);
	});
});

// The tests above pair the canonical address with a wrong bump, which a
// single-derivation check already rejects because the derived address does not
// match the account. They do not cover the actual shadow: a *valid* bump whose
// own address is passed alongside it. `assert_empty` only guards the address
// being created, so the unchecked builder accepts it, and `State` seeds are the
// global `[SEED_STATE_PREFIX]`, which makes the target a singleton that nothing
// on chain can distinguish from the real one.
test("PDA creation rejects a shadow state at a noncanonical bump", async () => {
	await withSurfnet(async (surfnet) => {
		const { payer, submit } = await createSubmitter(surfnet);
		const [shadowState, shadowBump] = await deriveShadowStatePda();

		await assertProgramError(
			() =>
				submit(createPdaInstruction(
					String(payer.address),
					shadowState,
					shadowBump,
					false,
				)),
			"InvalidSeeds",
		);

		const created = await createSolanaRpc(surfnet.rpcUrl)
			.getAccountInfo(address(shadowState), { encoding: "base64" })
			.send();
		assert.equal(created.value, null, "the shadow state must not exist");
	});
});
