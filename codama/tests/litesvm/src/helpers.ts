import {
	type Address,
	appendTransactionMessageInstructions,
	createTransactionMessage,
	type Instruction,
	lamports,
	pipe,
	type SendableTransaction,
	setTransactionMessageFeePayerSigner,
	signTransactionMessageWithSigners,
	type Transaction,
	type TransactionSigner,
	type TransactionWithLifetime,
} from "@solana/kit";
import { LiteSVM } from "litesvm";
import { existsSync } from "node:fs";
import { resolve } from "node:path";

export const LAMPORTS_PER_SOL = 1_000_000_000n;

/**
 * Resolve the path to a compiled SBF program binary.
 * Returns null if the binary doesn't exist (allows graceful skip).
 */
export function findProgramBinary(programName: string): string | null {
	const deployDir = process.env.SBF_OUT_DIR ??
		resolve(process.cwd(), "../../target/deploy");
	const soPath = resolve(deployDir, `${programName}.so`);
	return existsSync(soPath) ? soPath : null;
}

/**
 * Build and sign a v0 transaction from instructions.
 */
export async function buildAndSignTransaction(
	svm: LiteSVM,
	payer: TransactionSigner,
	instructions: Instruction[],
): Promise<SendableTransaction & Transaction & TransactionWithLifetime> {
	return await pipe(
		createTransactionMessage({ version: 0 }),
		(tx) => setTransactionMessageFeePayerSigner(payer, tx),
		(tx) => appendTransactionMessageInstructions(instructions, tx),
		(tx) => svm.setTransactionMessageLifetimeUsingLatestBlockhash(tx),
		(tx) => signTransactionMessageWithSigners(tx),
	);
}

/**
 * Fund an account with SOL.
 */
export function airdrop(
	svm: LiteSVM,
	address: Address,
	sol: bigint = 10n * LAMPORTS_PER_SOL,
) {
	svm.airdrop(address, lamports(sol));
}
