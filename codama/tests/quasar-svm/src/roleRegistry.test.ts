import { QuasarSvm } from "@blueshift-gg/quasar-svm/kit";
import {
	type Address,
	generateKeyPairSigner,
	getAddressEncoder,
	getProgramDerivedAddress,
	getUtf8Encoder,
} from "@solana/kit";
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
import { createFundedSignerAccount, loadProgram } from "./helpers";

const PROGRAM_NAME = "role_registry_program";

function expectSome<T>(value: T | null, message: string): T {
	if (value === null) {
		throw new Error(message);
	}
	return value;
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

describe("role_registry_program quasar e2e", () => {
	test("full lifecycle: initialize -> add role -> update -> deactivate -> rotate admin", async () => {
		using svm = new QuasarSvm();
		if (
			!loadProgram(svm, ROLE_REGISTRY_PROGRAM_PROGRAM_ADDRESS, PROGRAM_NAME)
		) {
			console.log(
				`[SKIP] ${PROGRAM_NAME}.so not found. Build SBF binaries first.`,
			);
			return;
		}

		const admin = await generateKeyPairSigner();
		const grantee = await generateKeyPairSigner();
		const newAdmin = await generateKeyPairSigner();
		const adminAccount = createFundedSignerAccount(admin);

		const [registryPda, registryBump] = await deriveRegistryPda(admin.address);
		const initIx = getInitializeInstruction({
			admin,
			registryConfig: registryPda,
			bump: registryBump,
		});
		const initResult = svm.processInstruction(initIx, [adminAccount]);
		initResult.assertSuccess();

		const registryAccount = expectSome(
			initResult.account(registryPda),
			"RegistryConfig account should exist after initialize",
		);
		const registry = decodeRegistryConfig(registryAccount);
		expect(registry.data.admin).toBe(admin.address);
		expect(registry.data.roleCount).toBe(0n);
		expect(registry.data.bump).toBe(registryBump);

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
		const addRoleResult = svm.processInstruction(addRoleIx, [
			adminAccount,
			registryAccount,
		]);
		addRoleResult.assertSuccess();

		const roleEntryAccount = expectSome(
			addRoleResult.account(roleEntryPda),
			"RoleEntry account should exist after add role",
		);
		const roleEntry = decodeRoleEntry(roleEntryAccount);
		expect(roleEntry.data.registry).toBe(registryPda);
		expect(roleEntry.data.roleId).toBe(roleId);
		expect(roleEntry.data.grantee).toBe(grantee.address);
		expect(roleEntry.data.permissions).toBe(permissions);
		expect(roleEntry.data.active).toBe(true);
		expect(roleEntry.data.bump).toBe(roleEntryBump);

		const registryAfterAdd = expectSome(
			addRoleResult.account(registryPda),
			"RegistryConfig should still exist after add role",
		);
		expect(decodeRegistryConfig(registryAfterAdd).data.roleCount).toBe(1n);

		const newPermissions = 42n;
		const updateIx = getUpdateRoleInstruction({
			admin,
			registryConfig: registryPda,
			roleEntry: roleEntryPda,
			permissions: newPermissions,
		});
		const updateResult = svm.processInstruction(updateIx, [
			adminAccount,
			registryAfterAdd,
			roleEntryAccount,
		]);
		updateResult.assertSuccess();

		const roleAfterUpdate = expectSome(
			updateResult.account(roleEntryPda),
			"RoleEntry should exist after update",
		);
		expect(decodeRoleEntry(roleAfterUpdate).data.permissions).toBe(
			newPermissions,
		);
		expect(decodeRoleEntry(roleAfterUpdate).data.active).toBe(true);

		const deactivateIx = getDeactivateRoleInstruction({
			admin,
			registryConfig: registryPda,
			roleEntry: roleEntryPda,
		});
		const deactivateResult = svm.processInstruction(deactivateIx, [
			adminAccount,
			registryAfterAdd,
			roleAfterUpdate,
		]);
		deactivateResult.assertSuccess();

		const roleAfterDeactivate = expectSome(
			deactivateResult.account(roleEntryPda),
			"RoleEntry should exist after deactivate",
		);
		expect(decodeRoleEntry(roleAfterDeactivate).data.active).toBe(false);

		const rotateIx = getRotateAdminInstruction({
			admin,
			newAdmin: newAdmin.address,
			registryConfig: registryPda,
		});
		const rotateResult = svm.processInstruction(rotateIx, [
			adminAccount,
			registryAfterAdd,
		]);
		rotateResult.assertSuccess();

		const registryAfterRotate = expectSome(
			rotateResult.account(registryPda),
			"RegistryConfig should exist after rotate admin",
		);
		expect(decodeRegistryConfig(registryAfterRotate).data.admin).toBe(
			newAdmin.address,
		);
	});

	test("wrong signer cannot add role", async () => {
		using svm = new QuasarSvm();
		if (
			!loadProgram(svm, ROLE_REGISTRY_PROGRAM_PROGRAM_ADDRESS, PROGRAM_NAME)
		) {
			console.log(`[SKIP] ${PROGRAM_NAME}.so not found.`);
			return;
		}

		const admin = await generateKeyPairSigner();
		const wrongAdmin = await generateKeyPairSigner();
		const grantee = await generateKeyPairSigner();
		const adminAccount = createFundedSignerAccount(admin);
		const wrongAdminAccount = createFundedSignerAccount(wrongAdmin);

		const [registryPda, registryBump] = await deriveRegistryPda(admin.address);
		const initIx = getInitializeInstruction({
			admin,
			registryConfig: registryPda,
			bump: registryBump,
		});
		const initResult = svm.processInstruction(initIx, [adminAccount]);
		initResult.assertSuccess();
		const registryAccount = expectSome(
			initResult.account(registryPda),
			"RegistryConfig account should exist after initialize",
		);

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
		const result = svm.processInstruction(addRoleIx, [
			wrongAdminAccount,
			registryAccount,
		]);
		expect(result.isError()).toBe(true);
	});
});
