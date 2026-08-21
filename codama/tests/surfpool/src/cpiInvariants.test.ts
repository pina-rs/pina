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
	getAddressEncoder,
	getBase64EncodedWireTransaction,
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
const PINA_BPF_PROGRAM_ID = "2nYtoevJCC8AFjdsfmkf8y1jN2nN9k4jVtD7G3f5n1Qe";
const PROP_AMM_PROGRAM_ID = "55555555555555555555555555555555555555555555";
const SYSTEM_PROGRAM_ID = "11111111111111111111111111111111";

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
	const data = new Uint8Array(1 + bumpBytes.length + addressBytes.length);
	data[0] = discriminator;
	data.set(bumpBytes, 1);
	data.set(addressBytes, 1 + bumpBytes.length);

	return data;
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

			await sendTransaction(signed, { commitment: "confirmed" });
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
		soPath: artifactPath("pina_bpf"),
	});
	surfnet.deploy({
		programId: PROP_AMM_PROGRAM_ID,
		soPath: artifactPath("prop_amm_program"),
	});
}

function createOracle(surfnet: Surfnet, authority: string): string {
	const oracle = Surfnet.newKeypair().publicKey;
	const data = new Uint8Array(1 + 32 + 8);
	data[0] = 1;
	data.set(getAddressEncoder().encode(address(authority)), 1);
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
			data.slice(1, 33),
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
			data.slice(1, 33),
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
		assert.equal(data[1], bump);
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
