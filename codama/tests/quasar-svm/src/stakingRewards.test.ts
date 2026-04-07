import {
	createKeyedMintAccount,
	QuasarSvm,
	SPL_TOKEN_PROGRAM_ID,
} from "@blueshift-gg/quasar-svm/kit";
import {
	type Address,
	generateKeyPairSigner,
	getAddressEncoder,
	getProgramDerivedAddress,
	getUtf8Encoder,
} from "@solana/kit";
import { describe, expect, test } from "vitest";
import {
	decodePoolState,
	decodePositionState,
} from "../../../clients/js/staking_rewards_program/src/generated/accounts";
import {
	getClaimInstruction,
	getDepositInstruction,
	getInitializePoolInstruction,
	getOpenPositionInstruction,
} from "../../../clients/js/staking_rewards_program/src/generated/instructions";
import { STAKING_REWARDS_PROGRAM_PROGRAM_ADDRESS } from "../../../clients/js/staking_rewards_program/src/generated/programs";
import {
	createAta,
	createFundedSignerAccount,
	deriveAtaAddress,
	expectSome,
	loadProgram,
} from "./helpers";

const PROGRAM_NAME = "staking_rewards_program";

async function derivePoolPda(stakeMint: Address, rewardMint: Address) {
	return await getProgramDerivedAddress({
		programAddress: STAKING_REWARDS_PROGRAM_PROGRAM_ADDRESS,
		seeds: [
			getUtf8Encoder().encode("pool"),
			getAddressEncoder().encode(stakeMint),
			getAddressEncoder().encode(rewardMint),
		],
	});
}

async function derivePositionPda(pool: Address, owner: Address) {
	return await getProgramDerivedAddress({
		programAddress: STAKING_REWARDS_PROGRAM_PROGRAM_ADDRESS,
		seeds: [
			getUtf8Encoder().encode("position"),
			getAddressEncoder().encode(pool),
			getAddressEncoder().encode(owner),
		],
	});
}

describe("staking_rewards_program quasar e2e", () => {
	test("initialize pool, open position, deposit, and claim", async () => {
		using svm = new QuasarSvm();
		if (
			!loadProgram(svm, STAKING_REWARDS_PROGRAM_PROGRAM_ADDRESS, PROGRAM_NAME)
		) {
			console.log(
				`[SKIP] ${PROGRAM_NAME}.so not found. Build SBF binaries first.`,
			);
			return;
		}

		const admin = await generateKeyPairSigner();
		const user = await generateKeyPairSigner();
		const adminAccount = createFundedSignerAccount(admin);
		const userAccount = createFundedSignerAccount(user);
		const stakeMint = createKeyedMintAccount(
			(await generateKeyPairSigner()).address,
			{
				decimals: 6,
				supply: 10_000n,
			},
		);
		const rewardMint = createKeyedMintAccount(
			(await generateKeyPairSigner()).address,
			{
				decimals: 6,
				supply: 10_000n,
			},
		);
		const userStakeAta = await createAta(user.address, stakeMint.address, 500n);
		const userRewardAta = await createAta(user.address, rewardMint.address, 0n);

		const [poolPda, poolBump] = await derivePoolPda(
			stakeMint.address,
			rewardMint.address,
		);
		const stakeVault = await deriveAtaAddress(
			poolPda as Address,
			stakeMint.address,
			SPL_TOKEN_PROGRAM_ID as Address,
		);
		const rewardVault = await deriveAtaAddress(
			poolPda as Address,
			rewardMint.address,
			SPL_TOKEN_PROGRAM_ID as Address,
		);

		const initializePoolResult = svm.processInstruction(
			getInitializePoolInstruction({
				admin,
				stakeMint: stakeMint.address,
				rewardMint: rewardMint.address,
				poolState: poolPda,
				stakeVault,
				rewardVault,
				tokenProgram: SPL_TOKEN_PROGRAM_ID as Address,
				bump: poolBump,
			}),
			[adminAccount, stakeMint, rewardMint],
		);
		initializePoolResult.assertSuccess();

		const poolState = decodePoolState(
			expectSome(
				initializePoolResult.account(poolPda),
				"pool state should exist after initializePool",
			),
		);
		expect(poolState.data.admin).toBe(admin.address);
		expect(poolState.data.totalStaked).toBe(0n);
		expect(poolState.data.bump).toBe(poolBump);

		const [positionPda, positionBump] = await derivePositionPda(
			poolPda,
			user.address,
		);
		const openPositionResult = svm.processInstruction(
			getOpenPositionInstruction({
				user,
				poolState: poolPda,
				positionState: positionPda,
				bump: positionBump,
			}),
			[
				userAccount,
				expectSome(
					initializePoolResult.account(poolPda),
					"pool state should exist before openPosition",
				),
			],
		);
		openPositionResult.assertSuccess();

		const openPositionState = decodePositionState(
			expectSome(
				openPositionResult.account(positionPda),
				"position state should exist after openPosition",
			),
		);
		expect(openPositionState.data.owner).toBe(user.address);
		expect(openPositionState.data.stakedAmount).toBe(0n);

		const depositResult = svm.processInstruction(
			getDepositInstruction({
				user,
				stakeMint: stakeMint.address,
				poolState: poolPda,
				positionState: positionPda,
				userStakeAta: userStakeAta.address,
				tokenProgram: SPL_TOKEN_PROGRAM_ID as Address,
				amount: 200n,
			}),
			[
				userAccount,
				stakeMint,
				expectSome(
					openPositionResult.account(poolPda),
					"pool state should exist before deposit",
				),
				expectSome(
					openPositionResult.account(positionPda),
					"position state should exist before deposit",
				),
				userStakeAta,
			],
		);
		depositResult.assertSuccess();

		const poolAfterDeposit = decodePoolState(
			expectSome(
				depositResult.account(poolPda),
				"pool state should exist after deposit",
			),
		);
		const positionAfterDeposit = decodePositionState(
			expectSome(
				depositResult.account(positionPda),
				"position state should exist after deposit",
			),
		);
		expect(poolAfterDeposit.data.totalStaked).toBe(200n);
		expect(positionAfterDeposit.data.stakedAmount).toBe(200n);

		const claimResult = svm.processInstruction(
			getClaimInstruction({
				user,
				rewardMint: rewardMint.address,
				poolState: poolPda,
				positionState: positionPda,
				userRewardAta: userRewardAta.address,
				tokenProgram: SPL_TOKEN_PROGRAM_ID as Address,
			}),
			[
				userAccount,
				rewardMint,
				expectSome(
					depositResult.account(poolPda),
					"pool state should exist before claim",
				),
				expectSome(
					depositResult.account(positionPda),
					"position state should exist before claim",
				),
				userRewardAta,
			],
		);
		claimResult.assertSuccess();

		const positionAfterClaim = decodePositionState(
			expectSome(
				claimResult.account(positionPda),
				"position state should exist after claim",
			),
		);
		expect(positionAfterClaim.data.pendingRewards).toBeGreaterThanOrEqual(0n);
	});
});
