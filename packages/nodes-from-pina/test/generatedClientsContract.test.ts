import { describe, expect, test } from "vitest";

import {
	CANCEL_DISCRIMINATOR as VESTING_CANCEL_DISCRIMINATOR,
	CLAIM_DISCRIMINATOR as VESTING_CLAIM_DISCRIMINATOR,
	getCancelInstruction,
	getClaimInstruction,
	getClaimInstructionDataDecoder,
	getInitializeInstruction as getVestingInitializeInstruction,
	getInitializeInstructionDataDecoder,
	INITIALIZE_DISCRIMINATOR as VESTING_INITIALIZE_DISCRIMINATOR,
	parseCancelInstruction,
	parseClaimInstruction,
	parseInitializeInstruction,
} from "../../../codama/clients/js/vesting_program/src/generated/instructions";
import {
	identifyVestingProgramInstruction,
	VESTING_PROGRAM_PROGRAM_ADDRESS,
	VestingProgramInstruction,
} from "../../../codama/clients/js/vesting_program/src/generated/programs";

import {
	ADD_ROLE_DISCRIMINATOR as ROLE_ADD_ROLE_DISCRIMINATOR,
	DEACTIVATE_ROLE_DISCRIMINATOR as ROLE_DEACTIVATE_ROLE_DISCRIMINATOR,
	getAddRoleInstruction,
	getAddRoleInstructionDataDecoder,
	getDeactivateRoleInstruction,
	getInitializeInstruction as getRoleInitializeInstruction,
	getInitializeInstructionDataDecoder
		as getRoleInitializeInstructionDataDecoder,
	getRotateAdminInstruction,
	getUpdateRoleInstruction,
	getUpdateRoleInstructionDataDecoder,
	INITIALIZE_DISCRIMINATOR as ROLE_INITIALIZE_DISCRIMINATOR,
	parseAddRoleInstruction,
	parseDeactivateRoleInstruction,
	parseInitializeInstruction as parseRoleInitializeInstruction,
	parseRotateAdminInstruction,
	parseUpdateRoleInstruction,
	ROTATE_ADMIN_DISCRIMINATOR as ROLE_ROTATE_ADMIN_DISCRIMINATOR,
	UPDATE_ROLE_DISCRIMINATOR as ROLE_UPDATE_ROLE_DISCRIMINATOR,
} from "../../../codama/clients/js/role_registry_program/src/generated/instructions";
import {
	identifyRoleRegistryProgramInstruction,
	ROLE_REGISTRY_PROGRAM_PROGRAM_ADDRESS,
	RoleRegistryProgramInstruction,
} from "../../../codama/clients/js/role_registry_program/src/generated/programs";

import {
	CLAIM_DISCRIMINATOR as STAKING_CLAIM_DISCRIMINATOR,
	DEPOSIT_DISCRIMINATOR as STAKING_DEPOSIT_DISCRIMINATOR,
	getClaimInstruction as getStakingClaimInstruction,
	getDepositInstruction,
	getDepositInstructionDataDecoder,
	getInitializePoolInstruction,
	getInitializePoolInstructionDataDecoder,
	getOpenPositionInstruction,
	getOpenPositionInstructionDataDecoder,
	getWithdrawInstruction,
	getWithdrawInstructionDataDecoder,
	INITIALIZE_POOL_DISCRIMINATOR as STAKING_INITIALIZE_POOL_DISCRIMINATOR,
	OPEN_POSITION_DISCRIMINATOR as STAKING_OPEN_POSITION_DISCRIMINATOR,
	parseClaimInstruction as parseStakingClaimInstruction,
	parseDepositInstruction,
	parseInitializePoolInstruction,
	parseOpenPositionInstruction,
	parseWithdrawInstruction,
	WITHDRAW_DISCRIMINATOR as STAKING_WITHDRAW_DISCRIMINATOR,
} from "../../../codama/clients/js/staking_rewards_program/src/generated/instructions";
import {
	identifyStakingRewardsProgramInstruction,
	STAKING_REWARDS_PROGRAM_PROGRAM_ADDRESS,
	StakingRewardsProgramInstruction,
} from "../../../codama/clients/js/staking_rewards_program/src/generated/programs";

const SYSTEM_PROGRAM_ADDRESS = "11111111111111111111111111111111";

const READONLY = 0;
const WRITABLE = 1;

type AccountExpectation = { address: string; role: number };

function accountRole(
	account: { role?: number; isSigner?: boolean; isWritable?: boolean },
): number {
	if (typeof account.role === "number") return account.role;
	if (account.isSigner === true && account.isWritable === true) return 3;
	if (account.isSigner === true) return 2;
	if (account.isWritable === true) return 1;
	return 0;
}

function expectAccountsMatch(
	accounts: readonly {
		address: string;
		role?: number;
		isSigner?: boolean;
		isWritable?: boolean;
	}[],
	expectations: readonly AccountExpectation[],
) {
	expect(accounts).toHaveLength(expectations.length);
	expectations.forEach((expected, index) => {
		expect(accounts[index]!.address).toBe(expected.address);
		expect(accountRole(accounts[index]!)).toBe(expected.role);
	});
}

describe("vesting JS client contracts", () => {
	test("initializes claim and cancel map to expected contract shape", () => {
		const admin = "3B7roNNQLnW43Par9AfTuVzEqZx7yPtXRA9K3Ev7RHyX";
		const beneficiary = "9MBwKBjzTLtLe8PkHVhi5CfGxKo8gCYbMEg5NMt1tcvr";
		const mint = "FEa5fqN6NACrhWUZSBdGKybJKNxkdw8cdLvRvTARsFHh";
		const vestingState = "BeneficiaryState111111111111111111111111111111";
		const vault = "11111111111111111111111111111112";
		const beneficiaryAta = "BeneficiaryAta111111111111111111111111111111";
		const tokenProgram = "So11111111111111111111111111111111111111112";

		const initialize = getVestingInitializeInstruction({
			admin,
			beneficiary,
			mint,
			vestingState,
			vault,
			totalAmount: 500n,
			startTs: 111n,
			cliffTs: 222n,
			endTs: 333n,
			bump: 9,
			tokenProgram,
		} as any);
		expect(initialize.programAddress).toBe(VESTING_PROGRAM_PROGRAM_ADDRESS);
		expectAccountsMatch(initialize.accounts, [
			{ address: admin, role: READONLY },
			{ address: beneficiary, role: READONLY },
			{ address: mint, role: READONLY },
			{ address: vestingState, role: WRITABLE },
			{ address: vault, role: WRITABLE },
			{ address: SYSTEM_PROGRAM_ADDRESS, role: READONLY },
			{ address: tokenProgram, role: READONLY },
		]);
		const initializeData = getInitializeInstructionDataDecoder().decode(
			initialize.data,
		);
		expect(initializeData.totalAmount).toBe(500n);
		expect(initializeData.startTs).toBe(111n);
		expect(initializeData.bump).toBe(9);
		expect(parseInitializeInstruction(initialize).data.totalAmount).toBe(500n);

		const claim = getClaimInstruction({
			beneficiary,
			mint,
			vestingState,
			beneficiaryAta,
			vault,
			tokenProgram,
			amount: 25n,
		} as any);
		expectAccountsMatch(claim.accounts, [
			{ address: beneficiary, role: READONLY },
			{ address: mint, role: READONLY },
			{ address: vestingState, role: WRITABLE },
			{ address: beneficiaryAta, role: WRITABLE },
			{ address: vault, role: WRITABLE },
			{ address: SYSTEM_PROGRAM_ADDRESS, role: READONLY },
			{ address: tokenProgram, role: READONLY },
		]);
		expect(getClaimInstructionDataDecoder().decode(claim.data).amount).toBe(
			25n,
		);
		expect(parseClaimInstruction(claim).data.amount).toBe(25n);

		const cancel = getCancelInstruction({
			admin,
			mint,
			vestingState,
			vault,
			tokenProgram,
		} as any);
		expectAccountsMatch(cancel.accounts, [
			{ address: admin, role: READONLY },
			{ address: mint, role: READONLY },
			{ address: vestingState, role: WRITABLE },
			{ address: vault, role: WRITABLE },
			{ address: tokenProgram, role: READONLY },
		]);
		expect(cancel).not.toHaveProperty("data");
		expect(parseCancelInstruction(cancel).accounts.admin.address).toBe(admin);
	});

	test("vesting discriminators are identified by instruction helpers", () => {
		expect(
			identifyVestingProgramInstruction({
				data: new Uint8Array([VESTING_INITIALIZE_DISCRIMINATOR]),
			}),
		).toBe(
			VestingProgramInstruction.Initialize,
		);
		expect(
			identifyVestingProgramInstruction({
				data: new Uint8Array([VESTING_CLAIM_DISCRIMINATOR]),
			}),
		).toBe(
			VestingProgramInstruction.Claim,
		);
		expect(
			identifyVestingProgramInstruction({
				data: new Uint8Array([VESTING_CANCEL_DISCRIMINATOR]),
			}),
		).toBe(
			VestingProgramInstruction.Cancel,
		);
	});
});

describe("role registry JS client contracts", () => {
	test("initialize/add/update/deactivate/rotate map to expected contract shape", () => {
		const admin = "3B7roNNQLnW43Par9AfTuVzEqZx7yPtXRA9K3Ev7RHyX";
		const newAdmin = "9MBwKBjzTLtLe8PkHVhi5CfGxKo8gCYbMEg5NMt1tcvr";
		const grantee = "FEa5fqN6NACrhWUZSBdGKybJKNxkdw8cdLvRvTARsFHh";
		const registryConfig = "11111111111111111111111111111112";
		const roleEntry = "RoleEntry111111111111111111111111111111111";

		expect(ROLE_REGISTRY_PROGRAM_PROGRAM_ADDRESS).toBe(
			"3B7roNNQLnW43Par9AfTuVzEqZx7yPtXRA9K3Ev7RHyX",
		);

		const init = getRoleInitializeInstruction({
			admin,
			registryConfig,
			bump: 8,
		} as any);
		expect(init.programAddress).toBe(ROLE_REGISTRY_PROGRAM_PROGRAM_ADDRESS);
		expectAccountsMatch(init.accounts, [
			{ address: admin, role: WRITABLE },
			{ address: registryConfig, role: WRITABLE },
			{ address: SYSTEM_PROGRAM_ADDRESS, role: READONLY },
		]);
		expect(getRoleInitializeInstructionDataDecoder().decode(init.data).bump)
			.toBe(8);
		expect(parseRoleInitializeInstruction(init).data.bump).toBe(8);

		const addRole = getAddRoleInstruction({
			admin,
			grantee,
			registryConfig,
			roleEntry,
			roleId: 11n,
			permissions: 77n,
			bump: 7,
		} as any);
		expectAccountsMatch(addRole.accounts, [
			{ address: admin, role: WRITABLE },
			{ address: grantee, role: READONLY },
			{ address: registryConfig, role: WRITABLE },
			{ address: roleEntry, role: WRITABLE },
			{ address: SYSTEM_PROGRAM_ADDRESS, role: READONLY },
		]);
		const addRoleData = getAddRoleInstructionDataDecoder().decode(addRole.data);
		expect(addRoleData.roleId).toBe(11n);
		expect(addRoleData.permissions).toBe(77n);
		expect(addRoleData.bump).toBe(7);
		expect(parseAddRoleInstruction(addRole).data.roleId).toBe(11n);

		const updateRole = getUpdateRoleInstruction({
			admin,
			registryConfig,
			roleEntry,
			permissions: 42n,
		} as any);
		expectAccountsMatch(updateRole.accounts, [
			{ address: admin, role: READONLY },
			{ address: registryConfig, role: READONLY },
			{ address: roleEntry, role: WRITABLE },
		]);
		expect(
			getUpdateRoleInstructionDataDecoder().decode(updateRole.data).permissions,
		).toBe(
			42n,
		);
		expect(parseUpdateRoleInstruction(updateRole).data.permissions).toBe(42n);

		const deactivateRole = getDeactivateRoleInstruction({
			admin,
			registryConfig,
			roleEntry,
		} as any);
		expectAccountsMatch(deactivateRole.accounts, [
			{ address: admin, role: READONLY },
			{ address: registryConfig, role: READONLY },
			{ address: roleEntry, role: WRITABLE },
		]);
		expect(
			parseDeactivateRoleInstruction(deactivateRole).accounts.roleEntry.address,
		).toBe(roleEntry);
		expect(deactivateRole).not.toHaveProperty("data");

		const rotateAdmin = getRotateAdminInstruction({
			admin,
			newAdmin,
			registryConfig,
		} as any);
		expectAccountsMatch(rotateAdmin.accounts, [
			{ address: admin, role: READONLY },
			{ address: newAdmin, role: READONLY },
			{ address: registryConfig, role: WRITABLE },
		]);
		expect(parseRotateAdminInstruction(rotateAdmin).accounts.newAdmin.address)
			.toBe(newAdmin);
		expect(rotateAdmin).not.toHaveProperty("data");
	});

	test("role discriminators are identified by instruction helpers", () => {
		expect(
			identifyRoleRegistryProgramInstruction({
				data: new Uint8Array([ROLE_INITIALIZE_DISCRIMINATOR]),
			}),
		).toBe(
			RoleRegistryProgramInstruction.Initialize,
		);
		expect(
			identifyRoleRegistryProgramInstruction({
				data: new Uint8Array([ROLE_ADD_ROLE_DISCRIMINATOR]),
			}),
		).toBe(
			RoleRegistryProgramInstruction.AddRole,
		);
		expect(
			identifyRoleRegistryProgramInstruction({
				data: new Uint8Array([ROLE_UPDATE_ROLE_DISCRIMINATOR]),
			}),
		).toBe(
			RoleRegistryProgramInstruction.UpdateRole,
		);
		expect(
			identifyRoleRegistryProgramInstruction({
				data: new Uint8Array([ROLE_DEACTIVATE_ROLE_DISCRIMINATOR]),
			}),
		).toBe(RoleRegistryProgramInstruction.DeactivateRole);
		expect(
			identifyRoleRegistryProgramInstruction({
				data: new Uint8Array([ROLE_ROTATE_ADMIN_DISCRIMINATOR]),
			}),
		).toBe(
			RoleRegistryProgramInstruction.RotateAdmin,
		);
	});
});

describe("staking rewards JS client contracts", () => {
	test("initialize-pool/open-position/deposit/withdraw/claim map to expected contract shape", () => {
		const admin = "3B7roNNQLnW43Par9AfTuVzEqZx7yPtXRA9K3Ev7RHyX";
		const stakeMint = "StakeMint111111111111111111111111111111111";
		const rewardMint = "RewardMint11111111111111111111111111111111";
		const poolState = "9MBwKBjzTLtLe8PkHVhi5CfGxKo8gCYbMEg5NMt1tcvr";
		const positionState = "Position111111111111111111111111111111111";
		const stakeVault = "StakeVault11111111111111111111111111111111";
		const rewardVault = "RewardVault1111111111111111111111111111111";
		const tokenProgram = "So11111111111111111111111111111111111111112";
		const userStakeAta = "UserStakeAta11111111111111111111111111111";
		const userRewardAta = "UserRewardAta1111111111111111111111111111";

		expect(STAKING_REWARDS_PROGRAM_PROGRAM_ADDRESS).toBe(
			"9MBwKBjzTLtLe8PkHVhi5CfGxKo8gCYbMEg5NMt1tcvr",
		);

		const init = getInitializePoolInstruction({
			admin,
			stakeMint,
			rewardMint,
			poolState,
			stakeVault,
			rewardVault,
			tokenProgram,
			bump: 5,
		} as any);
		expect(init.programAddress).toBe(STAKING_REWARDS_PROGRAM_PROGRAM_ADDRESS);
		expectAccountsMatch(init.accounts, [
			{ address: admin, role: READONLY },
			{ address: stakeMint, role: READONLY },
			{ address: rewardMint, role: READONLY },
			{ address: poolState, role: WRITABLE },
			{ address: stakeVault, role: WRITABLE },
			{ address: rewardVault, role: WRITABLE },
			{ address: SYSTEM_PROGRAM_ADDRESS, role: READONLY },
			{ address: tokenProgram, role: READONLY },
		]);
		expect(getInitializePoolInstructionDataDecoder().decode(init.data).bump)
			.toBe(5);
		expect(parseInitializePoolInstruction(init).data.bump).toBe(5);

		const openPosition = getOpenPositionInstruction({
			user: admin,
			poolState,
			positionState,
			bump: 4,
		} as any);
		expectAccountsMatch(openPosition.accounts, [
			{ address: admin, role: READONLY },
			{ address: poolState, role: READONLY },
			{ address: positionState, role: WRITABLE },
			{ address: SYSTEM_PROGRAM_ADDRESS, role: READONLY },
		]);
		expect(
			getOpenPositionInstructionDataDecoder().decode(openPosition.data).bump,
		).toBe(4);
		expect(parseOpenPositionInstruction(openPosition).data.bump).toBe(4);

		const deposit = getDepositInstruction({
			user: admin,
			stakeMint,
			poolState,
			positionState,
			userStakeAta,
			tokenProgram,
			amount: 42n,
		} as any);
		expectAccountsMatch(deposit.accounts, [
			{ address: admin, role: READONLY },
			{ address: stakeMint, role: READONLY },
			{ address: poolState, role: WRITABLE },
			{ address: positionState, role: WRITABLE },
			{ address: userStakeAta, role: WRITABLE },
			{ address: tokenProgram, role: READONLY },
			{ address: SYSTEM_PROGRAM_ADDRESS, role: READONLY },
		]);
		expect(getDepositInstructionDataDecoder().decode(deposit.data).amount).toBe(
			42n,
		);
		expect(parseDepositInstruction(deposit).data.amount).toBe(42n);

		const withdraw = getWithdrawInstruction({
			user: admin,
			stakeMint,
			poolState,
			positionState,
			userStakeAta,
			tokenProgram,
			amount: 7n,
		} as any);
		expectAccountsMatch(withdraw.accounts, [
			{ address: admin, role: READONLY },
			{ address: stakeMint, role: READONLY },
			{ address: poolState, role: WRITABLE },
			{ address: positionState, role: WRITABLE },
			{ address: userStakeAta, role: WRITABLE },
			{ address: tokenProgram, role: READONLY },
			{ address: SYSTEM_PROGRAM_ADDRESS, role: READONLY },
		]);
		expect(getWithdrawInstructionDataDecoder().decode(withdraw.data).amount)
			.toBe(7n);
		expect(parseWithdrawInstruction(withdraw).data.amount).toBe(7n);

		const claim = getStakingClaimInstruction({
			user: admin,
			rewardMint,
			poolState,
			positionState,
			userRewardAta,
			tokenProgram,
		} as any);
		expectAccountsMatch(claim.accounts, [
			{ address: admin, role: READONLY },
			{ address: rewardMint, role: READONLY },
			{ address: poolState, role: READONLY },
			{ address: positionState, role: WRITABLE },
			{ address: userRewardAta, role: WRITABLE },
			{ address: tokenProgram, role: READONLY },
			{ address: SYSTEM_PROGRAM_ADDRESS, role: READONLY },
		]);
		expect(parseStakingClaimInstruction(claim).accounts.user.address).toBe(
			admin,
		);
		expect(claim).not.toHaveProperty("data");
	});

	test("staking discriminators are identified by instruction helpers", () => {
		expect(
			identifyStakingRewardsProgramInstruction({
				data: new Uint8Array([STAKING_INITIALIZE_POOL_DISCRIMINATOR]),
			}),
		).toBe(StakingRewardsProgramInstruction.InitializePool);
		expect(
			identifyStakingRewardsProgramInstruction({
				data: new Uint8Array([STAKING_OPEN_POSITION_DISCRIMINATOR]),
			}),
		).toBe(StakingRewardsProgramInstruction.OpenPosition);
		expect(
			identifyStakingRewardsProgramInstruction({
				data: new Uint8Array([STAKING_DEPOSIT_DISCRIMINATOR]),
			}),
		).toBe(StakingRewardsProgramInstruction.Deposit);
		expect(
			identifyStakingRewardsProgramInstruction({
				data: new Uint8Array([STAKING_WITHDRAW_DISCRIMINATOR]),
			}),
		).toBe(StakingRewardsProgramInstruction.Withdraw);
		expect(
			identifyStakingRewardsProgramInstruction({
				data: new Uint8Array([STAKING_CLAIM_DISCRIMINATOR]),
			}),
		).toBe(StakingRewardsProgramInstruction.Claim);
	});
});
