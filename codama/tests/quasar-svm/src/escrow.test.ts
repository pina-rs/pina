import {
	createKeyedMintAccount,
	createKeyedSystemAccount,
	QuasarSvm,
	SPL_TOKEN_2022_PROGRAM_ID,
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
	test.each([
		["SPL Token", SPL_TOKEN_PROGRAM_ID as Address],
		["Token-2022", SPL_TOKEN_2022_PROGRAM_ID as Address],
	])(
		"make and take complete the escrow lifecycle with %s",
		async (_, tokenProgram) => {
			using svm = new QuasarSvm();
			if (!loadProgram(svm, ESCROW_PROGRAM_PROGRAM_ADDRESS, PROGRAM_NAME)) {
				throw new Error(
					`${PROGRAM_NAME}.so not found. Build SBF binaries before running e2e tests.`,
				);
			}

			const maker = await generateKeyPairSigner();
			const taker = await generateKeyPairSigner();
			const mintA = createKeyedMintAccount(
				(await generateKeyPairSigner()).address,
				{
					decimals: 6,
					supply: 1_000n,
				},
				tokenProgram,
			);
			const mintB = createKeyedMintAccount(
				(await generateKeyPairSigner()).address,
				{
					decimals: 6,
					supply: 1_000n,
				},
				tokenProgram,
			);
			const makerAccount = createFundedSignerAccount(maker);
			const takerAccount = createFundedSignerAccount(taker);
			const makerAtaA = await createAta(
				maker.address,
				mintA.address,
				500n,
				tokenProgram,
			);
			const takerAtaA = await createAta(
				taker.address,
				mintA.address,
				0n,
				tokenProgram,
			);
			const takerAtaB = await createAta(
				taker.address,
				mintB.address,
				250n,
				tokenProgram,
			);
			const makerAtaB = await createAta(
				maker.address,
				mintB.address,
				0n,
				tokenProgram,
			);

			const seed = 7n;
			const amountA = 120n;
			const amountB = 90n;
			const [escrowPda, bump] = await deriveEscrowPda(maker.address, seed);
			const vaultAta = await deriveAtaAddress(
				escrowPda as Address,
				mintA.address,
				tokenProgram,
			);

			const makeResult = svm.processInstruction(
				getMakeInstruction({
					maker,
					mintA: mintA.address,
					mintB: mintB.address,
					makerAtaA: makerAtaA.address,
					escrow: escrowPda,
					vault: vaultAta,
					tokenProgram,
					seed,
					amountA,
					amountB,
					bump,
				}),
				[
					makerAccount,
					mintA,
					mintB,
					makerAtaA,
					createKeyedSystemAccount(escrowPda as Address, 0n),
					createKeyedSystemAccount(vaultAta, 0n),
				],
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
					tokenProgram,
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

			const closedEscrow = expectSome(
				takeResult.account(escrowPda),
				"closed escrow PDA should remain observable",
			);
			const closedVault = expectSome(
				takeResult.account(vaultAta),
				"closed vault ATA should remain observable",
			);
			expect(closedEscrow.lamports).toBe(0n);
			expect(closedEscrow.data).toHaveLength(0);
			expect(closedVault.lamports).toBe(0n);
			expect(closedVault.data).toHaveLength(0);

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
		},
	);
});
