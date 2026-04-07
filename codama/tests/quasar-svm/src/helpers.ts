import {
	createKeyedAssociatedTokenAccount,
	createKeyedSystemAccount,
	QuasarSvm,
	SPL_ASSOCIATED_TOKEN_PROGRAM_ID,
	SPL_TOKEN_PROGRAM_ID,
} from "@blueshift-gg/quasar-svm/kit";
import type { Address, TransactionSigner } from "@solana/kit";
import { getAddressEncoder, getProgramDerivedAddress } from "@solana/kit";
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

export const LAMPORTS_PER_SOL = 1_000_000_000n;

export function findProgramBinary(programName: string): string | null {
	const deployDir = process.env.SBF_OUT_DIR ??
		resolve(process.cwd(), "../../target/deploy");
	const soPath = resolve(deployDir, `${programName}.so`);
	return existsSync(soPath) ? soPath : null;
}

export function loadProgram(
	svm: QuasarSvm,
	programAddress: Address,
	programName: string,
): boolean {
	const soPath = findProgramBinary(programName);
	if (!soPath) return false;

	svm.addProgram(programAddress, readFileSync(soPath));
	return true;
}

export function createFundedSignerAccount(
	signer: TransactionSigner,
	sol: bigint = 10n * LAMPORTS_PER_SOL,
) {
	return createKeyedSystemAccount(signer.address, sol);
}

export function expectSome<T>(value: T | null, message: string): T {
	if (value === null) throw new Error(message);
	return value;
}

export function getU64LeBytes(value: bigint): Uint8Array {
	const bytes = new Uint8Array(8);
	new DataView(bytes.buffer).setBigUint64(0, value, true);
	return bytes;
}

export async function deriveAtaAddress(
	wallet: Address,
	mint: Address,
	tokenProgram: Address = SPL_TOKEN_PROGRAM_ID as Address,
): Promise<Address> {
	const [ata] = await getProgramDerivedAddress({
		programAddress: SPL_ASSOCIATED_TOKEN_PROGRAM_ID as Address,
		seeds: [
			getAddressEncoder().encode(wallet),
			getAddressEncoder().encode(tokenProgram),
			getAddressEncoder().encode(mint),
		],
	});
	return ata as Address;
}

export async function createAta(owner: Address, mint: Address, amount: bigint) {
	return await createKeyedAssociatedTokenAccount(owner, mint, amount);
}
