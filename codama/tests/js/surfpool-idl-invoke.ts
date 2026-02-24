import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import {
	AccountRole,
	address,
	appendTransactionMessageInstruction,
	createKeyPairSignerFromBytes,
	createSolanaRpc,
	createSolanaRpcSubscriptions,
	createTransactionMessage,
	sendAndConfirmTransactionFactory,
	setTransactionMessageFeePayerSigner,
	setTransactionMessageLifetimeUsingBlockhash,
	signTransactionMessageWithSigners,
} from "@solana/kit";

type ConstantDiscriminatorNode = {
	kind: "constantDiscriminatorNode";
	offset: number;
	constant: {
		kind: "constantValueNode";
		type:
			| { kind: "numberTypeNode"; format: string; endian: "le" | "be" }
			| { kind: "bytesTypeNode" };
		value:
			| { kind: "numberValueNode"; number: number }
			| { kind: "bytesValueNode"; data: string; encoding: string };
	};
};

type InstructionAccountNode = {
	kind: "instructionAccountNode";
	name: string;
	isWritable: boolean;
	isSigner: boolean;
};

function requiredEnv(name: string): string {
	const value = process.env[name];
	if (!value) {
		throw new Error(`missing required environment variable: ${name}`);
	}
	return value;
}

function decodeHex(data: string): Uint8Array {
	const normalized = data.length % 2 === 0 ? data : `0${data}`;
	return Uint8Array.from(
		normalized.match(/.{1,2}/g)?.map((pair) => Number.parseInt(pair, 16)) ?? [],
	);
}

function encodeNumber(
	format: string,
	endian: "le" | "be",
	value: number,
): Uint8Array {
	const littleEndian = endian === "le";
	switch (format) {
		case "u8": {
			return Uint8Array.of(value);
		}
		case "u16": {
			const bytes = new Uint8Array(2);
			new DataView(bytes.buffer).setUint16(0, value, littleEndian);
			return bytes;
		}
		case "u32": {
			const bytes = new Uint8Array(4);
			new DataView(bytes.buffer).setUint32(0, value, littleEndian);
			return bytes;
		}
		case "u64": {
			const bytes = new Uint8Array(8);
			new DataView(bytes.buffer).setBigUint64(0, BigInt(value), littleEndian);
			return bytes;
		}
		default: {
			throw new Error(`unsupported discriminator number format: ${format}`);
		}
	}
}

function encodeDiscriminators(discriminators: unknown[]): Uint8Array {
	const constantDiscriminators = discriminators.filter(
		(node): node is ConstantDiscriminatorNode =>
			typeof node === "object" && node !== null &&
			(node as { kind?: string }).kind === "constantDiscriminatorNode",
	);
	assert.ok(
		constantDiscriminators.length > 0,
		"instruction has no constant discriminator",
	);

	const encodedParts = constantDiscriminators.map((node) => {
		const { type, value } = node.constant;
		if (type.kind === "numberTypeNode" && value.kind === "numberValueNode") {
			return {
				offset: node.offset,
				bytes: encodeNumber(type.format, type.endian, value.number),
			};
		}
		if (type.kind === "bytesTypeNode" && value.kind === "bytesValueNode") {
			if (value.encoding !== "base16") {
				throw new Error(
					`unsupported discriminator bytes encoding: ${value.encoding}. expected base16`,
				);
			}
			return {
				offset: node.offset,
				bytes: decodeHex(value.data),
			};
		}
		throw new Error("unsupported discriminator node value");
	});

	const size = encodedParts.reduce(
		(max, part) => Math.max(max, part.offset + part.bytes.length),
		0,
	);
	const out = new Uint8Array(size);
	for (const part of encodedParts) {
		out.set(part.bytes, part.offset);
	}
	return out;
}

function parseInstructionAccountAssignments(
	raw: string | undefined,
): Record<string, string> {
	if (raw === undefined || raw.trim() === "") {
		return {};
	}

	const parsed = JSON.parse(raw);
	assert.ok(
		typeof parsed === "object" && parsed !== null && !Array.isArray(parsed),
		"INSTRUCTION_ACCOUNTS_JSON must be a JSON object mapping account names to addresses",
	);

	const entries = Object.entries(parsed).map(([key, value]) => {
		assert.equal(
			typeof value,
			"string",
			`INSTRUCTION_ACCOUNTS_JSON.${key} must be a base58 address string`,
		);
		assert.notEqual(
			value.length,
			0,
			`INSTRUCTION_ACCOUNTS_JSON.${key} must not be empty`,
		);
		return [key, value] as const;
	});

	return Object.fromEntries(entries);
}

const idlPath = requiredEnv("IDL_PATH");
const payerKeypairPath = requiredEnv("PAYER_KEYPAIR_PATH");
const rpcUrl = requiredEnv("RPC_URL");
const wsUrl = requiredEnv("WS_URL");
const programId = requiredEnv("PROGRAM_ID");
const instructionName = process.env.INSTRUCTION_NAME ?? "initialize";
const instructionAccountAssignments = parseInstructionAccountAssignments(
	process.env.INSTRUCTION_ACCOUNTS_JSON,
);

const idl = JSON.parse(readFileSync(idlPath, "utf8")) as {
	program?: {
		instructions?: Array<
			{ name: string; accounts: unknown[]; discriminators: unknown[] }
		>;
	};
};

const instructionNode = idl.program?.instructions?.find(
	(instruction) => instruction.name === instructionName,
);
assert.ok(
	instructionNode,
	`instruction '${instructionName}' not found in generated IDL`,
);

const data = encodeDiscriminators(instructionNode.discriminators ?? []);

const payerSecret = Uint8Array.from(
	JSON.parse(readFileSync(payerKeypairPath, "utf8")) as number[],
);
const payer = await createKeyPairSignerFromBytes(payerSecret);
const payerAddress = String(payer.address);

const instructionAccounts = instructionNode.accounts.map((account, index) => {
	assert.ok(
		typeof account === "object" && account !== null &&
			(account as { kind?: string }).kind === "instructionAccountNode",
		`instruction '${instructionName}' account at index ${index} is not an instructionAccountNode`,
	);

	const accountNode = account as InstructionAccountNode;
	const assignedAddress = instructionAccountAssignments[accountNode.name] ??
		(accountNode.isSigner ? payerAddress : undefined);
	assert.ok(
		assignedAddress,
		`instruction '${instructionName}' account '${accountNode.name}' has no address assignment. ` +
			"Provide INSTRUCTION_ACCOUNTS_JSON to map account names to addresses.",
	);

	if (accountNode.isSigner && assignedAddress !== payerAddress) {
		throw new Error(
			`instruction '${instructionName}' account '${accountNode.name}' is a signer. ` +
				"This smoke test currently only supports signer accounts assigned to the payer.",
		);
	}

	const role = accountNode.isSigner
		? (accountNode.isWritable
			? AccountRole.WRITABLE_SIGNER
			: AccountRole.READONLY_SIGNER)
		: (accountNode.isWritable ? AccountRole.WRITABLE : AccountRole.READONLY);

	return { address: address(assignedAddress), role };
});

const rpc = createSolanaRpc(rpcUrl);
const rpcSubscriptions = createSolanaRpcSubscriptions(wsUrl);
const sendAndConfirmTransaction = sendAndConfirmTransactionFactory({
	rpc,
	rpcSubscriptions,
});

const { value: latestBlockhash } = await rpc.getLatestBlockhash().send();

const message = setTransactionMessageLifetimeUsingBlockhash(
	latestBlockhash,
	appendTransactionMessageInstruction(
		{ accounts: instructionAccounts, data, programAddress: address(programId) },
		setTransactionMessageFeePayerSigner(
			payer,
			createTransactionMessage({ version: 0 }),
		),
	),
);

const signedTransaction = await signTransactionMessageWithSigners(message);
await sendAndConfirmTransaction(signedTransaction, { commitment: "confirmed" });

console.log(
	`IDL invocation succeeded for instruction '${instructionName}' on program ${programId}.`,
);
