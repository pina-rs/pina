import {
	createKeyedAssociatedTokenAccount,
	createKeyedMintAccount,
	createKeyedSystemAccount,
	QuasarSvm,
	SPL_ASSOCIATED_TOKEN_PROGRAM_ID,
	SPL_TOKEN_PROGRAM_ID,
} from "@blueshift-gg/quasar-svm/kit";
import { getTokenDecoder } from "@solana-program/token";
import {
	type Account,
	type Address,
	address,
	generateKeyPairSigner,
	getAddressEncoder,
	getProgramDerivedAddress,
	getUtf8Encoder,
	lamports,
} from "@solana/kit";

/// The system program, the clock sysvar, and the associated-token program, as
/// the addresses the program validates against.
const SYSTEM_PROGRAM_ADDRESS = address("11111111111111111111111111111111");
const SYSVAR_CLOCK_ADDRESS = address(
	"SysvarC1ock11111111111111111111111111111111",
);
const SYSVAR_OWNER = address("Sysvar1111111111111111111111111111111111111");

/// A clock sysvar account carrying `unixTimestamp`.
///
/// The account is owned by the sysvar program, not the system program, because
/// the program asserts that ownership before reading the schedule comparison.
/// The layout is five little-endian 64-bit fields; the timestamp is last.
function createClockAccount(unixTimestamp: bigint): Account<Uint8Array> {
	const data = new Uint8Array(40);
	const view = new DataView(data.buffer);
	for (const [index, value] of [0n, 0n, 0n, 0n, unixTimestamp].entries()) {
		view.setBigUint64(index * 8, value, true);
	}

	return {
		address: SYSVAR_CLOCK_ADDRESS,
		programAddress: SYSVAR_OWNER,
		lamports: lamports(1_000_000n),
		data,
		executable: false,
		space: 40n,
	};
}
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
		// The schedule ends at 100; move the clock past it so the whole
		// allocation has vested by the time Claim runs.
		svm.setClock({
			slot: 100n,
			epochStartTimestamp: 0n,
			epoch: 0n,
			leaderScheduleEpoch: 0n,
			unixTimestamp: 1_000n,
		});
		if (!loadProgram(svm, VESTING_PROGRAM_PROGRAM_ADDRESS, PROGRAM_NAME)) {
			console.log(
				`[SKIP] ${PROGRAM_NAME}.so not found. Build SBF binaries first.`,
			);
			return;
		}
		// The harness provides the SPL token and associated-token programs the
		// release and refund paths CPI into, so only the program under test is
		// loaded here.

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
		// The admin holds the allocation: Initialize moves it into the vault in
		// the same instruction, so a schedule can never exist unfunded.
		const adminAta = await createAta(admin.address, mint.address, 1_000n);
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
				adminAta: adminAta.address,
				tokenProgram: SPL_TOKEN_PROGRAM_ID as Address,
				totalAmount: 1_000n,
				startTs: 0n,
				cliffTs: 0n,
				endTs: 100n,
				bump,
			}),
			[
				adminAccount,
				mint,
				createKeyedSystemAccount(vestingPda as Address, 0n),
				createKeyedSystemAccount(vaultAta, 0n),
				adminAta,
			],
		);
		initializeResult.assertSuccess();

		// The atomic funding moved the allocation out of the admin's account.
		const fundedVault = expectSome(
			initializeResult.account(vaultAta),
			"the vault should exist after initialize",
		);
		expect(
			expectSome(
				initializeResult.account(vaultAta, getTokenDecoder()),
				"the vault should be funded by initialize",
			).amount,
		).toBe(1_000n);

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
				clock: SYSVAR_CLOCK_ADDRESS,
				amount: 250n,
			}),
			[
				beneficiaryAccount,
				mint,
				expectSome(
					initializeResult.account(vestingPda),
					"vesting state should exist before claim",
				),
				fundedVault,
				beneficiaryAta,
				createClockAccount(1_000n),
			],
		);
		claimResult.assertSuccess();

		const claimedState = decodeVestingState(
			expectSome(
				claimResult.account(vestingPda),
				"vesting state should exist after claim",
			),
		);
		const beneficiaryAtaAfterClaim = expectSome(
			claimResult.account(beneficiaryAta.address, getTokenDecoder()),
			"beneficiary ATA should exist after claim",
		);
		// Claim releases the vested amount from the vault, so the beneficiary
		// balance grows by exactly the claimed amount.
		expect(claimedState.data.claimedAmount).toBe(250n);
		expect(beneficiaryAtaAfterClaim.amount).toBe(250n);

		const cancelResult = svm.processInstruction(
			getCancelInstruction({
				admin,
				mint: mint.address,
				vestingState: vestingPda,
				adminAta: adminAta.address,
				vault: vaultAta,
				beneficiaryAta: beneficiaryAta.address,
				tokenProgram: SPL_TOKEN_PROGRAM_ID as Address,
				clock: SYSVAR_CLOCK_ADDRESS,
			}),
			[
				adminAccount,
				mint,
				expectSome(
					claimResult.account(vestingPda),
					"vesting state should exist before cancel",
				),
				expectSome(
					initializeResult.account(adminAta.address),
					"the admin ATA should survive initialize",
				),
				expectSome(
					claimResult.account(vaultAta),
					"vault ATA should exist before cancel",
				),
				expectSome(
					claimResult.account(beneficiaryAta.address),
					"the beneficiary ATA should survive the claim",
				),
				createClockAccount(1_000n),
			],
		);
		cancelResult.assertSuccess();

		// Cancel settles the beneficiary's earned entitlement first, then
		// refunds only the genuinely unvested remainder. The clock sits past the
		// schedule's end, so every token still in the vault is vested and owed
		// to the beneficiary: the 250 already claimed stays with them and the
		// remaining 750 follows, leaving the administrator a zero refund.
		const beneficiaryAtaAfterCancel = expectSome(
			cancelResult.account(beneficiaryAta.address, getTokenDecoder()),
			"beneficiary ATA should exist after cancel",
		);
		expect(beneficiaryAtaAfterCancel.amount).toBe(1_000n);
		const adminAtaAfterCancel = expectSome(
			cancelResult.account(adminAta.address, getTokenDecoder()),
			"admin ATA should exist after cancel",
		);
		expect(adminAtaAfterCancel.amount).toBe(0n);

		const cancelledState = decodeVestingState(
			expectSome(
				cancelResult.account(vestingPda),
				"vesting state should exist after cancel",
			),
		);
		expect(cancelledState.data.cancelled).toBe(true);
	});
});
