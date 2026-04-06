import "@solana/kit";
import "@solana/program-client-core";

declare module "@solana/kit" {
	/** Compatibility shim for Codama-generated clients. */
	export type ClientWithRpc<TRpcMethods = unknown> = {
		rpc: TRpcMethods;
	};

	/** Compatibility shim for Codama-generated clients. */
	export type ClientWithTransactionPlanning = {
		planTransaction: (...args: unknown[]) => unknown;
		planTransactions: (...args: unknown[]) => unknown;
	};

	/** Compatibility shim for Codama-generated clients. */
	export type ClientWithTransactionSending = {
		sendTransaction: (...args: unknown[]) => unknown;
		sendTransactions: (...args: unknown[]) => unknown;
	};

	/** Compatibility shim for Codama-generated clients. */
	export type GetAccountInfoApi = unknown;

	/** Compatibility shim for Codama-generated clients. */
	export type GetMultipleAccountsApi = unknown;

	/** Compatibility shim for Codama-generated clients. */
	export const SOLANA_ERROR__PROGRAM_CLIENTS__INSUFFICIENT_ACCOUNT_METAS:
		SolanaErrorCode;

	/** Compatibility shim for Codama-generated clients. */
	export const SOLANA_ERROR__PROGRAM_CLIENTS__FAILED_TO_IDENTIFY_ACCOUNT:
		SolanaErrorCode;

	/** Compatibility shim for Codama-generated clients. */
	export const SOLANA_ERROR__PROGRAM_CLIENTS__FAILED_TO_IDENTIFY_INSTRUCTION:
		SolanaErrorCode;

	/** Compatibility shim for Codama-generated clients. */
	export const SOLANA_ERROR__PROGRAM_CLIENTS__UNRECOGNIZED_INSTRUCTION_TYPE:
		SolanaErrorCode;
}

declare module "@solana/program-client-core" {
	/**
	 * Compatibility overload that accepts generated codecs from current Codama output.
	 */
	export function addSelfFetchFunctions<
		TInput = unknown,
		TOutput = unknown,
		TCodec = unknown,
	>(
		client: unknown,
		codec: TCodec,
	): SelfFetchFunctions<TInput, TOutput> & TCodec;

	/**
	 * Compatibility overload that relaxes client constraints for generated program plugins.
	 */
	export function addSelfPlanAndSendFunctions<TItem = unknown>(
		client: unknown,
		input: TItem,
	): SelfPlanAndSendFunctions & TItem;
}
