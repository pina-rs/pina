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
import { decodeVestingState } from "../../../clients/js/vesting_program/src/generated/accounts";
import {
	getCancelInstruction,
	getClaimInstruction,
	getInitializeInstruction,
} from "../../../clients/js/vesting_program/src/generated/instructions";
import { VESTING_PROGRAM_PROGRAM_ADDRESS } from "../../../clients/js/vesting_program/src/generated/programs";
import {
	createAta,
	createFundedSignerAccount,
	deriveAtaAddress,
	expectSome,
	loadProgram,
} from "./helpers";

const PROGRAM_NAME = "vesting_program";

async function deriveVestingPda(
	admin: Address,
	beneficiary: Address,
	mint: Address,
) {
	return await getProgramDerivedAddress({
		programAddress: VESTING_PROGRAM_PROGRAM_ADDRESS,
		seeds: [
			getUtf8Encoder().encode("vesting"),
			getAddressEncoder().encode(admin),
			getAddressEncoder().encode(beneficiary),
			getAddressEncoder().encode(mint),
		],
	});
}

describe("vesting_program quasar e2e", () => {
	test("initialize, claim, and cancel a vesting schedule", async () => {
		using svm = new QuasarSvm();
		if (!loadProgram(svm, VESTING_PROGRAM_PROGRAM_ADDRESS, PROGRAM_NAME)) {
			console.log(
				`[SKIP] ${PROGRAM_NAME}.so not found. Build SBF binaries first.`,
			);
			return;
		}

		const admin = await generateKeyPairSigner();
		const beneficiary = await generateKeyPairSigner();
		const adminAccount = createFundedSignerAccount(admin);
		const beneficiaryAccount = createFundedSignerAccount(beneficiary);
		const mint = createKeyedMintAccount(
			(await generateKeyPairSigner()).address,
			{
				decimals: 6,
				supply: 10_000n,
			},
		);
		const beneficiaryAta = await createAta(
			beneficiary.address,
			mint.address,
			0n,
		);
		const [vestingPda, bump] = await deriveVestingPda(
			admin.address,
			beneficiary.address,
			mint.address,
		);
		const vaultAta = await deriveAtaAddress(
			vestingPda as Address,
			mint.address,
			SPL_TOKEN_PROGRAM_ID as Address,
		);

		const initializeResult = svm.processInstruction(
			getInitializeInstruction({
				admin,
				beneficiary: beneficiary.address,
				mint: mint.address,
				vestingState: vestingPda,
				vault: vaultAta,
				tokenProgram: SPL_TOKEN_PROGRAM_ID as Address,
				totalAmount: 1_000n,
				startTs: 0n,
				cliffTs: 0n,
				endTs: 100n,
				bump,
			}),
			[adminAccount, mint],
		);
		initializeResult.assertSuccess();

		const initializedState = decodeVestingState(
			expectSome(
				initializeResult.account(vestingPda),
				"vesting state should exist after initialize",
			),
		);
		expect(initializedState.data.admin).toBe(admin.address);
		expect(initializedState.data.beneficiary).toBe(beneficiary.address);
		expect(initializedState.data.claimedAmount).toBe(0n);
		expect(initializedState.data.cancelled).toBe(false);

		const claimResult = svm.processInstruction(
			getClaimInstruction({
				beneficiary,
				mint: mint.address,
				vestingState: vestingPda,
				beneficiaryAta: beneficiaryAta.address,
				vault: vaultAta,
				tokenProgram: SPL_TOKEN_PROGRAM_ID as Address,
				amount: 250n,
			}),
			[
				beneficiaryAccount,
				mint,
				expectSome(
					initializeResult.account(vestingPda),
					"vesting state should exist before claim",
				),
				expectSome(
					initializeResult.account(vaultAta),
					"vault ATA should exist before claim",
				),
				beneficiaryAta,
			],
		);
		claimResult.assertSuccess();

		const claimedState = decodeVestingState(
			expectSome(
				claimResult.account(vestingPda),
				"vesting state should exist after claim",
			),
		);
		expect(claimedState.data.claimedAmount).toBe(250n);

		const beneficiaryAtaAfterClaim = expectSome(
			claimResult.account(beneficiaryAta.address, getTokenDecoder()),
			"beneficiary ATA should exist after claim",
		);
		expect(beneficiaryAtaAfterClaim.amount).toBe(250n);

		const cancelResult = svm.processInstruction(
			getCancelInstruction({
				admin,
				mint: mint.address,
				vestingState: vestingPda,
				vault: vaultAta,
				tokenProgram: SPL_TOKEN_PROGRAM_ID as Address,
			}),
			[
				adminAccount,
				mint,
				expectSome(
					claimResult.account(vestingPda),
					"vesting state should exist before cancel",
				),
				expectSome(
					claimResult.account(vaultAta),
					"vault ATA should exist before cancel",
				),
			],
		);
		cancelResult.assertSuccess();

		const cancelledState = decodeVestingState(
			expectSome(
				cancelResult.account(vestingPda),
				"vesting state should exist after cancel",
			),
		);
		expect(cancelledState.data.cancelled).toBe(true);
	});
});
