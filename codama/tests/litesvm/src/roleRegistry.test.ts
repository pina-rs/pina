/**
 * LiteSVM end-to-end tests for the role_registry_program.
 *
 * These tests load the compiled SBF binary into LiteSVM, send real
 * transactions using the generated JS client instruction builders,
 * and verify the resulting on-chain account state using the generated
 * account decoders. This validates that pina's discriminator model
 * produces correct instructions from both Rust and TypeScript.
 */

import {
	type Address,
	assertAccountExists,
	generateKeyPairSigner,
	getAddressEncoder,
	getProgramDerivedAddress,
	getUtf8Encoder,
	lamports,
} from "@solana/kit";
import { LiteSVM } from "litesvm";
import { describe, expect, test } from "vitest";
import {
	decodeRegistryConfig,
	decodeRoleEntry,
} from "../../../clients/js/role_registry_program/src/generated/accounts";
import {
	getAddRoleInstruction,
	getDeactivateRoleInstruction,
	getInitializeInstruction,
	getRotateAdminInstruction,
	getUpdateRoleInstruction,
} from "../../../clients/js/role_registry_program/src/generated/instructions";
import {
	ROLE_REGISTRY_PROGRAM_PROGRAM_ADDRESS,
} from "../../../clients/js/role_registry_program/src/generated/programs";
import { airdrop, buildAndSignTransaction, findProgramBinary } from "./helpers";

const PROGRAM_NAME = "role_registry_program";

function loadProgram(): { svm: LiteSVM; programAddress: Address } | null {
	const soPath = findProgramBinary(PROGRAM_NAME);
	if (!soPath) return null;

	const svm = new LiteSVM();
	svm.addProgramFromFile(
		ROLE_REGISTRY_PROGRAM_PROGRAM_ADDRESS,
		soPath,
	);
	return { svm, programAddress: ROLE_REGISTRY_PROGRAM_PROGRAM_ADDRESS };
}

async function deriveRegistryPda(admin: Address) {
	return await getProgramDerivedAddress({
		programAddress: ROLE_REGISTRY_PROGRAM_PROGRAM_ADDRESS,
		seeds: [
			getUtf8Encoder().encode("registry"),
			getAddressEncoder().encode(admin),
		],
	});
}

async function deriveRoleEntryPda(registry: Address, roleId: bigint) {
	const roleIdBytes = new Uint8Array(8);
	new DataView(roleIdBytes.buffer).setBigUint64(0, roleId, true);
	return await getProgramDerivedAddress({
		programAddress: ROLE_REGISTRY_PROGRAM_PROGRAM_ADDRESS,
		seeds: [
			getUtf8Encoder().encode("role-entry"),
			getAddressEncoder().encode(registry),
			roleIdBytes,
		],
	});
}

describe("role_registry_program e2e", () => {
	test("full lifecycle: initialize → add role → update → deactivate → rotate admin", async () => {
		const loaded = loadProgram();
		if (!loaded) {
			console.log(
				`[SKIP] ${PROGRAM_NAME}.so not found. Build SBF binaries first.`,
			);
			return;
		}
		const { svm } = loaded;

		const admin = await generateKeyPairSigner();
		const grantee = await generateKeyPairSigner();
		const newAdmin = await generateKeyPairSigner();
		airdrop(svm, admin.address);

		// --- Initialize ---
		const [registryPda, registryBump] = await deriveRegistryPda(admin.address);

		const initIx = getInitializeInstruction({
			admin,
			registryConfig: registryPda,
			bump: registryBump,
		});
		const initTx = await buildAndSignTransaction(svm, admin, [initIx]);
		svm.sendTransaction(initTx);

		// Verify RegistryConfig account
		const registryAccount = svm.getAccount(registryPda);
		assertAccountExists(registryAccount);
		const registry = decodeRegistryConfig(registryAccount);
		expect(registry.data.admin).toBe(admin.address);
		expect(registry.data.roleCount).toBe(0n);
		expect(registry.data.bump).toBe(registryBump);

		// --- Add Role ---
		const roleId = 1n;
		const permissions = 255n;
		const [roleEntryPda, roleEntryBump] = await deriveRoleEntryPda(
			registryPda,
			roleId,
		);

		const addRoleIx = getAddRoleInstruction({
			admin,
			grantee: grantee.address,
			registryConfig: registryPda,
			roleEntry: roleEntryPda,
			roleId,
			permissions,
			bump: roleEntryBump,
		});
		const addRoleTx = await buildAndSignTransaction(svm, admin, [addRoleIx]);
		svm.sendTransaction(addRoleTx);

		// Verify RoleEntry account
		const roleEntryAccount = svm.getAccount(roleEntryPda);
		assertAccountExists(roleEntryAccount);
		const roleEntry = decodeRoleEntry(roleEntryAccount);
		expect(roleEntry.data.registry).toBe(registryPda);
		expect(roleEntry.data.roleId).toBe(roleId);
		expect(roleEntry.data.grantee).toBe(grantee.address);
		expect(roleEntry.data.permissions).toBe(permissions);
		expect(roleEntry.data.active).toBe(true);
		expect(roleEntry.data.bump).toBe(roleEntryBump);

		// Verify role_count incremented
		const registryAfterAdd = decodeRegistryConfig(svm.getAccount(registryPda));
		assertAccountExists(registryAfterAdd);
		expect(registryAfterAdd.data.roleCount).toBe(1n);

		// --- Update Role ---
		const newPermissions = 42n;
		const updateIx = getUpdateRoleInstruction({
			admin,
			registryConfig: registryPda,
			roleEntry: roleEntryPda,
			permissions: newPermissions,
		});
		const updateTx = await buildAndSignTransaction(svm, admin, [updateIx]);
		svm.sendTransaction(updateTx);

		const roleAfterUpdate = decodeRoleEntry(svm.getAccount(roleEntryPda));
		assertAccountExists(roleAfterUpdate);
		expect(roleAfterUpdate.data.permissions).toBe(newPermissions);
		expect(roleAfterUpdate.data.active).toBe(true);

		// --- Deactivate Role ---
		const deactivateIx = getDeactivateRoleInstruction({
			admin,
			registryConfig: registryPda,
			roleEntry: roleEntryPda,
		});
		const deactivateTx = await buildAndSignTransaction(svm, admin, [
			deactivateIx,
		]);
		svm.sendTransaction(deactivateTx);

		const roleAfterDeactivate = decodeRoleEntry(svm.getAccount(roleEntryPda));
		assertAccountExists(roleAfterDeactivate);
		expect(roleAfterDeactivate.data.active).toBe(false);

		// --- Rotate Admin ---
		airdrop(svm, newAdmin.address);
		const rotateIx = getRotateAdminInstruction({
			admin,
			newAdmin: newAdmin.address,
			registryConfig: registryPda,
		});
		const rotateTx = await buildAndSignTransaction(svm, admin, [rotateIx]);
		svm.sendTransaction(rotateTx);

		const registryAfterRotate = decodeRegistryConfig(
			svm.getAccount(registryPda),
		);
		assertAccountExists(registryAfterRotate);
		expect(registryAfterRotate.data.admin).toBe(newAdmin.address);
	});

	test("wrong signer cannot add role", async () => {
		const loaded = loadProgram();
		if (!loaded) {
			console.log(`[SKIP] ${PROGRAM_NAME}.so not found.`);
			return;
		}
		const { svm } = loaded;

		const admin = await generateKeyPairSigner();
		const wrongAdmin = await generateKeyPairSigner();
		const grantee = await generateKeyPairSigner();
		airdrop(svm, admin.address);
		airdrop(svm, wrongAdmin.address);

		// Initialize with admin
		const [registryPda, registryBump] = await deriveRegistryPda(admin.address);
		const initIx = getInitializeInstruction({
			admin,
			registryConfig: registryPda,
			bump: registryBump,
		});
		svm.sendTransaction(await buildAndSignTransaction(svm, admin, [initIx]));

		// Try to add role with wrongAdmin — should fail
		const [roleEntryPda, roleEntryBump] = await deriveRoleEntryPda(
			registryPda,
			1n,
		);
		const addRoleIx = getAddRoleInstruction({
			admin: wrongAdmin,
			grantee: grantee.address,
			registryConfig: registryPda,
			roleEntry: roleEntryPda,
			roleId: 1n,
			permissions: 7n,
			bump: roleEntryBump,
		});

		// Build and attempt to send — expect it to throw
		const tx = await buildAndSignTransaction(svm, wrongAdmin, [addRoleIx]);
		expect(() => svm.sendTransaction(tx)).toThrow();
	});
});
