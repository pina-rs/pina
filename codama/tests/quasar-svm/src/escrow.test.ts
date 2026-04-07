import {
	createKeyedMintAccount,
	QuasarSvm,
	SPL_TOKEN_PROGRAM_ID,
} from "@blueshift-gg/quasar-svm/kit";
import { getTokenDecoder } from "@solana-program/token";
import {
	type Address,
	generateKeyPairSigner,
	getAddressEncoder,
	getProgramDerivedAddress,
	getUtf8Encoder,
} from "@solana/kit";
import { describe, expect, test } from "vitest";
import { decodeEscrowState } from "../../../clients/js/escrow_program/src/generated/accounts";
import {
	getMakeInstruction,
	getTakeInstruction,
} from "../../../clients/js/escrow_program/src/generated/instructions";
import { ESCROW_PROGRAM_PROGRAM_ADDRESS } from "../../../clients/js/escrow_program/src/generated/programs";
import {
	createAta,
	createFundedSignerAccount,
	deriveAtaAddress,
	expectSome,
	getU64LeBytes,
	loadProgram,
} from "./helpers";

const PROGRAM_NAME = "escrow_program";

async function deriveEscrowPda(maker: Address, seed: bigint) {
	return await getProgramDerivedAddress({
		programAddress: ESCROW_PROGRAM_PROGRAM_ADDRESS,
		seeds: [
			getUtf8Encoder().encode("escrow"),
			getAddressEncoder().encode(maker),
			getU64LeBytes(seed),
		],
	});
}

describe("escrow_program quasar e2e", () => {
	test("make and take complete the escrow lifecycle", async () => {
		using svm = new QuasarSvm();
		if (!loadProgram(svm, ESCROW_PROGRAM_PROGRAM_ADDRESS, PROGRAM_NAME)) {
			console.log(
				`[SKIP] ${PROGRAM_NAME}.so not found. Build SBF binaries first.`,
			);
			return;
		}

		const maker = await generateKeyPairSigner();
		const taker = await generateKeyPairSigner();
		const mintA = createKeyedMintAccount(
			(await generateKeyPairSigner()).address,
			{
				decimals: 6,
				supply: 1_000n,
			},
		);
		const mintB = createKeyedMintAccount(
			(await generateKeyPairSigner()).address,
			{
				decimals: 6,
				supply: 1_000n,
			},
		);
		const makerAccount = createFundedSignerAccount(maker);
		const takerAccount = createFundedSignerAccount(taker);
		const makerAtaA = await createAta(maker.address, mintA.address, 500n);
		const takerAtaA = await createAta(taker.address, mintA.address, 0n);
		const takerAtaB = await createAta(taker.address, mintB.address, 250n);
		const makerAtaB = await createAta(maker.address, mintB.address, 0n);

		const seed = 7n;
		const amountA = 120n;
		const amountB = 90n;
		const [escrowPda, bump] = await deriveEscrowPda(maker.address, seed);
		const vaultAta = await deriveAtaAddress(
			escrowPda as Address,
			mintA.address,
			SPL_TOKEN_PROGRAM_ID as Address,
		);

		const makeResult = svm.processInstruction(
			getMakeInstruction({
				maker,
				mintA: mintA.address,
				mintB: mintB.address,
				makerAtaA: makerAtaA.address,
				escrow: escrowPda,
				vault: vaultAta,
				tokenProgram: SPL_TOKEN_PROGRAM_ID as Address,
				seed,
				amountA,
				amountB,
				bump,
			}),
			[makerAccount, mintA, mintB, makerAtaA],
		);
		makeResult.assertSuccess();

		const escrowState = decodeEscrowState(
			expectSome(
				makeResult.account(escrowPda),
				"escrow PDA should exist after make",
			),
		);
		expect(escrowState.data.maker).toBe(maker.address);
		expect(escrowState.data.amountA).toBe(amountA);
		expect(escrowState.data.amountB).toBe(amountB);
		expect(escrowState.data.seed).toBe(seed);

		const vaultAfterMake = expectSome(
			makeResult.account(vaultAta, getTokenDecoder()),
			"vault ATA should exist after make",
		);
		expect(vaultAfterMake.amount).toBe(amountA);

		const makerAtaAAfterMake = expectSome(
			makeResult.account(makerAtaA.address, getTokenDecoder()),
			"maker ATA A should exist after make",
		);
		expect(makerAtaAAfterMake.amount).toBe(380n);

		const takeResult = svm.processInstruction(
			getTakeInstruction({
				taker,
				mintA: mintA.address,
				mintB: mintB.address,
				takerAtaA: takerAtaA.address,
				takerAtaB: takerAtaB.address,
				maker: maker.address,
				makerAtaB: makerAtaB.address,
				escrow: escrowPda,
				vault: vaultAta,
				tokenProgram: SPL_TOKEN_PROGRAM_ID as Address,
			}),
			[
				takerAccount,
				mintA,
				mintB,
				takerAtaA,
				takerAtaB,
				makerAtaB,
				expectSome(
					makeResult.account(escrowPda),
					"escrow PDA should exist before take",
				),
				expectSome(
					makeResult.account(vaultAta),
					"vault ATA should exist before take",
				),
			],
		);
		takeResult.assertSuccess();

		expect(takeResult.account(escrowPda)).toBeNull();
		expect(takeResult.account(vaultAta)).toBeNull();

		const takerAtaAAfterTake = expectSome(
			takeResult.account(takerAtaA.address, getTokenDecoder()),
			"taker ATA A should exist after take",
		);
		const takerAtaBAfterTake = expectSome(
			takeResult.account(takerAtaB.address, getTokenDecoder()),
			"taker ATA B should exist after take",
		);
		const makerAtaBAfterTake = expectSome(
			takeResult.account(makerAtaB.address, getTokenDecoder()),
			"maker ATA B should exist after take",
		);
		expect(takerAtaAAfterTake.amount).toBe(amountA);
		expect(takerAtaBAfterTake.amount).toBe(160n);
		expect(makerAtaBAfterTake.amount).toBe(amountB);
	});
});
