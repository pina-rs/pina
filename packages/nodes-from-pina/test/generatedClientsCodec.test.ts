/**
 * Codec roundtrip and instruction parse/rebuild tests for all three generated
 * JS clients.  These go beyond shape-checking: they encode data, decode it
 * back, and verify every field survives the trip.
 */

import { describe, expect, test } from "vitest";

// ---------------------------------------------------------------------------
// Role-registry client
// ---------------------------------------------------------------------------
import {
	decodeRegistryConfig,
	getRegistryConfigCodec,
	getRegistryConfigEncoder,
	REGISTRY_CONFIG_DISCRIMINATOR,
} from "../../../codama/clients/js/role_registry_program/src/generated/accounts/registryConfig";
import {
	decodeRoleEntry,
	getRoleEntryCodec,
	getRoleEntryEncoder,
	ROLE_ENTRY_DISCRIMINATOR,
} from "../../../codama/clients/js/role_registry_program/src/generated/accounts/roleEntry";
import {
	getAddRoleInstruction,
	getAddRoleInstructionDataCodec,
	getInitializeInstruction as getRoleInitializeInstruction,
	getInitializeInstructionDataCodec as getRoleInitializeCodec,
	getUpdateRoleInstruction,
	getUpdateRoleInstructionDataCodec,
	parseAddRoleInstruction,
	parseInitializeInstruction as parseRoleInitializeInstruction,
	parseUpdateRoleInstruction,
} from "../../../codama/clients/js/role_registry_program/src/generated/instructions";

// ---------------------------------------------------------------------------
// Vesting client
// ---------------------------------------------------------------------------
import {
	decodeVestingState,
	getVestingStateCodec,
	getVestingStateEncoder,
	VESTING_STATE_DISCRIMINATOR,
} from "../../../codama/clients/js/vesting_program/src/generated/accounts/vestingState";
import {
	getCancelInstruction,
	getClaimInstruction,
	getClaimInstructionDataCodec,
	getInitializeInstruction as getVestingInitializeInstruction,
	getInitializeInstructionDataCodec as getVestingInitializeCodec,
	parseCancelInstruction,
	parseClaimInstruction,
	parseInitializeInstruction as parseVestingInitialize,
} from "../../../codama/clients/js/vesting_program/src/generated/instructions";

// ---------------------------------------------------------------------------
// Staking client
// ---------------------------------------------------------------------------
import {
	decodePoolState,
	getPoolStateCodec,
	getPoolStateEncoder,
	POOL_STATE_DISCRIMINATOR,
} from "../../../codama/clients/js/staking_rewards_program/src/generated/accounts/poolState";
import {
	decodePositionState,
	getPositionStateCodec,
	getPositionStateEncoder,
	POSITION_STATE_DISCRIMINATOR,
} from "../../../codama/clients/js/staking_rewards_program/src/generated/accounts/positionState";
import {
	getDepositInstruction,
	getDepositInstructionDataCodec,
	getInitializePoolInstruction,
	getInitializePoolInstructionDataCodec,
	getOpenPositionInstruction,
	getOpenPositionInstructionDataCodec,
	getWithdrawInstruction,
	getWithdrawInstructionDataCodec,
	parseDepositInstruction,
	parseInitializePoolInstruction,
	parseOpenPositionInstruction,
	parseWithdrawInstruction,
} from "../../../codama/clients/js/staking_rewards_program/src/generated/instructions";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const ADDR_A = "3B7roNNQLnW43Par9AfTuVzEqZx7yPtXRA9K3Ev7RHyX";
const ADDR_B = "9MBwKBjzTLtLe8PkHVhi5CfGxKo8gCYbMEg5NMt1tcvr";
const ADDR_C = "FEa5fqN6NACrhWUZSBdGKybJKNxkdw8cdLvRvTARsFHh";
const SYSTEM = "11111111111111111111111111111111";
const TOKEN = "So11111111111111111111111111111111111111112";

/** Build a fake EncodedAccount for decoding tests.
 *  The generated `decodeXxx` functions use decoders that read struct fields
 *  directly (no discriminator prefix), so we pass encoder output as-is. */
function encodedAccount(
	address: string,
	encoderOutput: Uint8Array | {
		readonly length: number;
		[index: number]: number;
	},
) {
	const buf = encoderOutput instanceof Uint8Array
		? encoderOutput
		: new Uint8Array(encoderOutput as any);
	return {
		address,
		data: buf,
		executable: false,
		exists: true as const,
		lamports: 1_000_000n,
		programAddress: address,
		space: BigInt(buf.length),
	} as any;
}

// ===================================================================
// Role-registry codec roundtrips
// ===================================================================

describe("role-registry codec roundtrips", () => {
	test("RegistryConfig encode → decode preserves all fields", () => {
		const codec = getRegistryConfigCodec();
		const input = { admin: ADDR_A as any, roleCount: 42n, bump: 7 };
		const bytes = codec.encode(input);
		const decoded = codec.decode(bytes as Uint8Array);
		expect(decoded.admin).toBe(ADDR_A);
		expect(decoded.roleCount).toBe(42n);
		expect(decoded.bump).toBe(7);
	});

	test("RoleEntry encode → decode preserves all fields", () => {
		const codec = getRoleEntryCodec();
		const input = {
			registry: ADDR_A as any,
			roleId: 11n,
			grantee: ADDR_B as any,
			permissions: 255n,
			active: true,
			bump: 3,
		};
		const bytes = codec.encode(input);
		const decoded = codec.decode(bytes as Uint8Array);
		expect(decoded.registry).toBe(ADDR_A);
		expect(decoded.roleId).toBe(11n);
		expect(decoded.grantee).toBe(ADDR_B);
		expect(decoded.permissions).toBe(255n);
		expect(decoded.active).toBe(true);
		expect(decoded.bump).toBe(3);
	});

	test("RegistryConfig decodeAccount from encoded bytes", () => {
		const encoder = getRegistryConfigEncoder();
		const data = encoder.encode({
			admin: ADDR_A as any,
			roleCount: 0n,
			bump: 5,
		});
		const account = encodedAccount(ADDR_A, data);
		const decoded = decodeRegistryConfig(account);
		expect(decoded.data.admin).toBe(ADDR_A);
		expect(decoded.data.roleCount).toBe(0n);
		expect(decoded.data.bump).toBe(5);
	});

	test("RoleEntry decodeAccount from encoded bytes", () => {
		const encoder = getRoleEntryEncoder();
		const data = encoder.encode({
			registry: ADDR_A as any,
			roleId: 99n,
			grantee: ADDR_C as any,
			permissions: 7n,
			active: false,
			bump: 1,
		});
		const account = encodedAccount(ADDR_B, data);
		const decoded = decodeRoleEntry(account);
		expect(decoded.data.roleId).toBe(99n);
		expect(decoded.data.grantee).toBe(ADDR_C);
		expect(decoded.data.active).toBe(false);
	});

	test("Initialize instruction data codec roundtrip", () => {
		const codec = getRoleInitializeCodec();
		const encoded = codec.encode({ bump: 12 });
		const decoded = codec.decode(encoded as Uint8Array);
		expect(decoded.bump).toBe(12);
	});

	test("AddRole instruction data codec roundtrip", () => {
		const codec = getAddRoleInstructionDataCodec();
		const encoded = codec.encode({ roleId: 42n, permissions: 77n, bump: 3 });
		const decoded = codec.decode(encoded as Uint8Array);
		expect(decoded.roleId).toBe(42n);
		expect(decoded.permissions).toBe(77n);
		expect(decoded.bump).toBe(3);
	});

	test("UpdateRole instruction data codec roundtrip", () => {
		const codec = getUpdateRoleInstructionDataCodec();
		const encoded = codec.encode({ permissions: 128n });
		const decoded = codec.decode(encoded as Uint8Array);
		expect(decoded.permissions).toBe(128n);
	});

	test("Initialize instruction build → parse roundtrip", () => {
		const ix = getRoleInitializeInstruction({
			admin: ADDR_A,
			registryConfig: ADDR_B,
			bump: 8,
		} as any);
		const parsed = parseRoleInitializeInstruction(ix);
		expect(parsed.data.bump).toBe(8);
		expect(parsed.accounts.admin.address).toBe(ADDR_A);
		expect(parsed.accounts.registryConfig.address).toBe(ADDR_B);
		expect(parsed.accounts.systemProgram.address).toBe(SYSTEM);
	});

	test("AddRole instruction build → parse roundtrip", () => {
		const ix = getAddRoleInstruction({
			admin: ADDR_A,
			grantee: ADDR_C,
			registryConfig: ADDR_B,
			roleEntry: ADDR_A,
			roleId: 5n,
			permissions: 15n,
			bump: 2,
		} as any);
		const parsed = parseAddRoleInstruction(ix);
		expect(parsed.data.roleId).toBe(5n);
		expect(parsed.data.permissions).toBe(15n);
		expect(parsed.data.bump).toBe(2);
		expect(parsed.accounts.grantee.address).toBe(ADDR_C);
	});

	test("UpdateRole instruction build → parse roundtrip", () => {
		const ix = getUpdateRoleInstruction({
			admin: ADDR_A,
			registryConfig: ADDR_B,
			roleEntry: ADDR_C,
			permissions: 63n,
		} as any);
		const parsed = parseUpdateRoleInstruction(ix);
		expect(parsed.data.permissions).toBe(63n);
		expect(parsed.accounts.roleEntry.address).toBe(ADDR_C);
	});
});

// ===================================================================
// Vesting codec roundtrips
// ===================================================================

describe("vesting codec roundtrips", () => {
	test("VestingState encode → decode preserves all fields", () => {
		const codec = getVestingStateCodec();
		const input = {
			admin: ADDR_A as any,
			beneficiary: ADDR_B as any,
			mint: ADDR_C as any,
			totalAmount: 1_000_000n,
			claimedAmount: 250_000n,
			startTs: 100n,
			cliffTs: 200n,
			endTs: 300n,
			cancelled: false,
			bump: 9,
		};
		const bytes = codec.encode(input);
		const decoded = codec.decode(bytes as Uint8Array);
		expect(decoded.admin).toBe(ADDR_A);
		expect(decoded.beneficiary).toBe(ADDR_B);
		expect(decoded.mint).toBe(ADDR_C);
		expect(decoded.totalAmount).toBe(1_000_000n);
		expect(decoded.claimedAmount).toBe(250_000n);
		expect(decoded.startTs).toBe(100n);
		expect(decoded.cliffTs).toBe(200n);
		expect(decoded.endTs).toBe(300n);
		expect(decoded.cancelled).toBe(false);
		expect(decoded.bump).toBe(9);
	});

	test("VestingState decodeAccount from encoded bytes", () => {
		const encoder = getVestingStateEncoder();
		const data = encoder.encode({
			admin: ADDR_A as any,
			beneficiary: ADDR_B as any,
			mint: ADDR_C as any,
			totalAmount: 500n,
			claimedAmount: 0n,
			startTs: 10n,
			cliffTs: 20n,
			endTs: 30n,
			cancelled: true,
			bump: 4,
		});
		const account = encodedAccount(ADDR_A, data);
		const decoded = decodeVestingState(account);
		expect(decoded.data.totalAmount).toBe(500n);
		expect(decoded.data.cancelled).toBe(true);
		expect(decoded.data.bump).toBe(4);
	});

	test("Initialize instruction data codec roundtrip", () => {
		const codec = getVestingInitializeCodec();
		const encoded = codec.encode({
			totalAmount: 999n,
			startTs: 1n,
			cliffTs: 2n,
			endTs: 3n,
			bump: 7,
		});
		const decoded = codec.decode(encoded as Uint8Array);
		expect(decoded.totalAmount).toBe(999n);
		expect(decoded.startTs).toBe(1n);
		expect(decoded.cliffTs).toBe(2n);
		expect(decoded.endTs).toBe(3n);
		expect(decoded.bump).toBe(7);
	});

	test("Claim instruction data codec roundtrip", () => {
		const codec = getClaimInstructionDataCodec();
		const encoded = codec.encode({ amount: 42n });
		const decoded = codec.decode(encoded as Uint8Array);
		expect(decoded.amount).toBe(42n);
	});

	test("Initialize instruction build → parse roundtrip", () => {
		const ix = getVestingInitializeInstruction({
			admin: ADDR_A,
			beneficiary: ADDR_B,
			mint: ADDR_C,
			vestingState: ADDR_A,
			vault: ADDR_B,
			tokenProgram: TOKEN,
			totalAmount: 500n,
			startTs: 111n,
			cliffTs: 222n,
			endTs: 333n,
			bump: 9,
		} as any);
		const parsed = parseVestingInitialize(ix);
		expect(parsed.data.totalAmount).toBe(500n);
		expect(parsed.data.startTs).toBe(111n);
		expect(parsed.data.bump).toBe(9);
		expect(parsed.accounts.admin.address).toBe(ADDR_A);
		expect(parsed.accounts.beneficiary.address).toBe(ADDR_B);
	});

	test("Claim instruction build → parse roundtrip", () => {
		const ix = getClaimInstruction({
			beneficiary: ADDR_B,
			mint: ADDR_C,
			vestingState: ADDR_A,
			beneficiaryAta: ADDR_B,
			vault: ADDR_C,
			tokenProgram: TOKEN,
			amount: 25n,
		} as any);
		const parsed = parseClaimInstruction(ix);
		expect(parsed.data.amount).toBe(25n);
		expect(parsed.accounts.beneficiary.address).toBe(ADDR_B);
	});

	test("Cancel instruction build → parse (no data)", () => {
		const ix = getCancelInstruction({
			admin: ADDR_A,
			mint: ADDR_C,
			vestingState: ADDR_B,
			vault: ADDR_A,
			tokenProgram: TOKEN,
		} as any);
		const parsed = parseCancelInstruction(ix);
		expect(parsed.accounts.admin.address).toBe(ADDR_A);
		expect(parsed.accounts.vestingState.address).toBe(ADDR_B);
	});
});

// ===================================================================
// Staking codec roundtrips
// ===================================================================

describe("staking codec roundtrips", () => {
	test("PoolState encode → decode preserves all fields", () => {
		const codec = getPoolStateCodec();
		const input = {
			admin: ADDR_A as any,
			stakeMint: ADDR_B as any,
			rewardMint: ADDR_C as any,
			totalStaked: 10_000n,
			rewardIndex: 500n,
			paused: false,
			bump: 5,
		};
		const bytes = codec.encode(input);
		const decoded = codec.decode(bytes as Uint8Array);
		expect(decoded.admin).toBe(ADDR_A);
		expect(decoded.stakeMint).toBe(ADDR_B);
		expect(decoded.rewardMint).toBe(ADDR_C);
		expect(decoded.totalStaked).toBe(10_000n);
		expect(decoded.rewardIndex).toBe(500n);
		expect(decoded.paused).toBe(false);
		expect(decoded.bump).toBe(5);
	});

	test("PositionState encode → decode preserves all fields", () => {
		const codec = getPositionStateCodec();
		const input = {
			pool: ADDR_A as any,
			owner: ADDR_B as any,
			stakedAmount: 1_000n,
			rewardDebt: 50n,
			pendingRewards: 25n,
			bump: 3,
		};
		const bytes = codec.encode(input);
		const decoded = codec.decode(bytes as Uint8Array);
		expect(decoded.pool).toBe(ADDR_A);
		expect(decoded.owner).toBe(ADDR_B);
		expect(decoded.stakedAmount).toBe(1_000n);
		expect(decoded.rewardDebt).toBe(50n);
		expect(decoded.pendingRewards).toBe(25n);
		expect(decoded.bump).toBe(3);
	});

	test("PoolState decodeAccount from encoded bytes", () => {
		const encoder = getPoolStateEncoder();
		const data = encoder.encode({
			admin: ADDR_A as any,
			stakeMint: ADDR_B as any,
			rewardMint: ADDR_C as any,
			totalStaked: 0n,
			rewardIndex: 0n,
			paused: true,
			bump: 1,
		});
		const account = encodedAccount(ADDR_A, data);
		const decoded = decodePoolState(account);
		expect(decoded.data.paused).toBe(true);
		expect(decoded.data.bump).toBe(1);
	});

	test("PositionState decodeAccount from encoded bytes", () => {
		const encoder = getPositionStateEncoder();
		const data = encoder.encode({
			pool: ADDR_A as any,
			owner: ADDR_B as any,
			stakedAmount: 777n,
			rewardDebt: 10n,
			pendingRewards: 5n,
			bump: 2,
		});
		const account = encodedAccount(ADDR_B, data);
		const decoded = decodePositionState(account);
		expect(decoded.data.stakedAmount).toBe(777n);
		expect(decoded.data.pendingRewards).toBe(5n);
	});

	test("InitializePool instruction data codec roundtrip", () => {
		const codec = getInitializePoolInstructionDataCodec();
		const encoded = codec.encode({ bump: 8 });
		expect(codec.decode(encoded as Uint8Array).bump).toBe(8);
	});

	test("OpenPosition instruction data codec roundtrip", () => {
		const codec = getOpenPositionInstructionDataCodec();
		const encoded = codec.encode({ bump: 4 });
		expect(codec.decode(encoded as Uint8Array).bump).toBe(4);
	});

	test("Deposit instruction data codec roundtrip", () => {
		const codec = getDepositInstructionDataCodec();
		const encoded = codec.encode({ amount: 42n });
		expect(codec.decode(encoded as Uint8Array).amount).toBe(42n);
	});

	test("Withdraw instruction data codec roundtrip", () => {
		const codec = getWithdrawInstructionDataCodec();
		const encoded = codec.encode({ amount: 7n });
		expect(codec.decode(encoded as Uint8Array).amount).toBe(7n);
	});

	test("InitializePool instruction build → parse roundtrip", () => {
		const ix = getInitializePoolInstruction({
			admin: ADDR_A,
			stakeMint: ADDR_B,
			rewardMint: ADDR_C,
			poolState: ADDR_A,
			stakeVault: ADDR_B,
			rewardVault: ADDR_C,
			tokenProgram: TOKEN,
			bump: 5,
		} as any);
		const parsed = parseInitializePoolInstruction(ix);
		expect(parsed.data.bump).toBe(5);
		expect(parsed.accounts.admin.address).toBe(ADDR_A);
		expect(parsed.accounts.stakeMint.address).toBe(ADDR_B);
	});

	test("OpenPosition instruction build → parse roundtrip", () => {
		const ix = getOpenPositionInstruction({
			user: ADDR_A,
			poolState: ADDR_B,
			positionState: ADDR_C,
			bump: 4,
		} as any);
		const parsed = parseOpenPositionInstruction(ix);
		expect(parsed.data.bump).toBe(4);
		expect(parsed.accounts.user.address).toBe(ADDR_A);
		expect(parsed.accounts.poolState.address).toBe(ADDR_B);
	});

	test("Deposit instruction build → parse roundtrip", () => {
		const ix = getDepositInstruction({
			user: ADDR_A,
			stakeMint: ADDR_B,
			poolState: ADDR_C,
			positionState: ADDR_A,
			userStakeAta: ADDR_B,
			tokenProgram: TOKEN,
			amount: 42n,
		} as any);
		const parsed = parseDepositInstruction(ix);
		expect(parsed.data.amount).toBe(42n);
		expect(parsed.accounts.user.address).toBe(ADDR_A);
	});

	test("Withdraw instruction build → parse roundtrip", () => {
		const ix = getWithdrawInstruction({
			user: ADDR_A,
			stakeMint: ADDR_B,
			poolState: ADDR_C,
			positionState: ADDR_A,
			userStakeAta: ADDR_B,
			tokenProgram: TOKEN,
			amount: 7n,
		} as any);
		const parsed = parseWithdrawInstruction(ix);
		expect(parsed.data.amount).toBe(7n);
	});
});
