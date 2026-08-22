import assert from "node:assert/strict";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { basename, dirname, resolve } from "node:path";

import {
	AccountRole,
	address,
	addSignersToInstruction,
	appendTransactionMessageInstruction,
	createKeyPairSignerFromBytes,
	createSolanaRpc,
	createTransactionMessage,
	getAddressEncoder,
	getBase64EncodedWireTransaction,
	getProgramDerivedAddress,
	getSignatureFromTransaction,
	type Instruction,
	sendTransactionWithoutConfirmingFactory,
	setTransactionMessageFeePayerSigner,
	setTransactionMessageLifetimeUsingBlockhash,
	signTransactionMessageWithSigners,
} from "@solana/kit";
import { Surfnet } from "@solana/surfpool";

type TestSigner = Awaited<ReturnType<typeof createKeyPairSignerFromBytes>>;

const ROOT = resolve(import.meta.dirname, "../../../..");
const IDL_DIRECTORY = resolve(ROOT, "codama/idls");
const EXAMPLES_DIRECTORY = resolve(ROOT, "examples");
const SBF_OUT_DIR = process.env.SBF_OUT_DIR ??
	resolve(ROOT, "target/surfpool/examples");

// Keep this list intentional. The inventory assertion below turns adding an
// example into a required Surfpool test decision instead of a silent omission.
const EXAMPLE_PROGRAMS = [
	"anchor_declare_id",
	"anchor_declare_program",
	"anchor_duplicate_mutable_accounts",
	"anchor_errors",
	"anchor_events",
	"anchor_floats",
	"anchor_realloc",
	"anchor_system_accounts",
	"anchor_sysvars",
	"counter_program",
	"escrow_program",
	"hello_solana",
	"optional_accounts_program",
	"pina_bpf",
	"profile_program",
	"prop_amm_program",
	"role_registry_program",
	"staking_rewards_program",
	"todo_program",
	"transfer_sol",
	"vesting_program",
] as const;

type ExampleProgram = (typeof EXAMPLE_PROGRAMS)[number];

type NumberTypeNode = {
	kind: "numberTypeNode";
	format: "u8" | "u16" | "u32" | "u64" | "i8" | "i16" | "i32" | "i64";
	endian: "le" | "be";
};

type TypeNode = NumberTypeNode | {
	kind: "bytesTypeNode" | "stringTypeNode" | "publicKeyTypeNode";
} | {
	kind: "fixedSizeTypeNode";
	size: number;
	type: TypeNode;
} | {
	kind: "sizePrefixTypeNode";
	prefix: NumberTypeNode;
	type: TypeNode;
};

type DiscriminatorNode = {
	kind: "constantDiscriminatorNode";
	offset: number;
	constant: {
		kind: "constantValueNode";
		type: NumberTypeNode | { kind: "bytesTypeNode" };
		value:
			| { kind: "numberValueNode"; number: number }
			| { kind: "bytesValueNode"; data: string; encoding: "base16" };
	};
};

type InstructionNode = {
	kind: "instructionNode";
	name: string;
	accounts?: Array<{ name: string }>;
	arguments?: Array<{ name: string; type: TypeNode }>;
	discriminators?: DiscriminatorNode[];
};

type ProgramIdl = {
	program: {
		publicKey: string;
		instructions: InstructionNode[];
	};
};

type ExampleDescriptor = {
	name: ExampleProgram;
	programId: string;
	artifactPath: string;
	idl: ProgramIdl;
};

function stringifyValue(value: unknown): string {
	return JSON.stringify(
		value,
		(_key, nested: unknown) =>
			typeof nested === "bigint" ? nested.toString() : nested,
	);
}

class ProgramInvocationError extends Error {
	public readonly errorText: string;
	public readonly programError: unknown;
	public readonly logs: readonly string[];

	public constructor(programError: unknown, logs: readonly string[]) {
		const errorText = stringifyValue(programError);
		super(`program execution failed: ${errorText}\n${logs.join("\n")}`);
		this.errorText = errorText;
		this.programError = programError;
		this.logs = logs;
	}
}

function readIdl(name: ExampleProgram): ProgramIdl {
	const idlPath = resolve(IDL_DIRECTORY, `${name}.json`);
	const idl = JSON.parse(readFileSync(idlPath, "utf8")) as ProgramIdl;
	assert.ok(idl.program.publicKey, `${name} has no program public key`);
	assert.ok(idl.program.instructions.length > 0, `${name} has no instructions`);
	return idl;
}

function artifactPath(name: ExampleProgram): string {
	const direct = resolve(SBF_OUT_DIR, `${name}.so`);
	const library = resolve(SBF_OUT_DIR, `lib${name}.so`);
	const artifact = existsSync(direct) ? direct : library;
	assert.ok(
		existsSync(artifact),
		`missing ${name} SBF artifact; run scripts/build-surfpool-examples.sh first`,
	);
	return artifact;
}

function assertInventory(): void {
	const expected = [...EXAMPLE_PROGRAMS].sort();
	const idls = readdirSync(IDL_DIRECTORY)
		.filter((file) => file.endsWith(".json"))
		.map((file) => basename(file, ".json"))
		.sort();
	const exampleCrates = readdirSync(EXAMPLES_DIRECTORY)
		.filter((entry) =>
			existsSync(resolve(EXAMPLES_DIRECTORY, entry, "Cargo.toml"))
		)
		.sort();

	assert.deepEqual(
		idls,
		expected,
		"every generated example IDL needs a Surfpool case",
	);
	assert.deepEqual(
		exampleCrates,
		expected,
		"every example program crate needs a Surfpool case",
	);
}

function encodeNumber(type: NumberTypeNode, value = 0): Uint8Array {
	const size = Number.parseInt(type.format.slice(1), 10) / 8;
	const bytes = new Uint8Array(size);
	const view = new DataView(bytes.buffer);
	const littleEndian = type.endian === "le";

	switch (type.format) {
		case "u8":
		case "i8":
			bytes[0] = value;
			return bytes;
		case "u16":
			view.setUint16(0, value, littleEndian);
			return bytes;
		case "i16":
			view.setInt16(0, value, littleEndian);
			return bytes;
		case "u32":
			view.setUint32(0, value, littleEndian);
			return bytes;
		case "i32":
			view.setInt32(0, value, littleEndian);
			return bytes;
		case "u64":
			view.setBigUint64(0, BigInt(value), littleEndian);
			return bytes;
		case "i64":
			view.setBigInt64(0, BigInt(value), littleEndian);
			return bytes;
	}
}

function zeroValue(type: TypeNode): Uint8Array {
	switch (type.kind) {
		case "numberTypeNode":
			return encodeNumber(type);
		case "publicKeyTypeNode":
			return new Uint8Array(32);
		case "bytesTypeNode":
		case "stringTypeNode":
			return new Uint8Array();
		case "sizePrefixTypeNode":
			return encodeNumber(type.prefix);
		case "fixedSizeTypeNode": {
			const bytes = zeroValue(type.type);
			assert.ok(
				bytes.length <= type.size,
				`cannot zero-encode fixed ${type.size}-byte IDL value`,
			);
			const padded = new Uint8Array(type.size);
			padded.set(bytes);
			return padded;
		}
	}
}

function decodeHex(data: string): Uint8Array {
	assert.equal(data.length % 2, 0, "base16 discriminator must have full bytes");
	const bytes = new Uint8Array(data.length / 2);
	for (let index = 0; index < bytes.length; index += 1) {
		bytes[index] = Number.parseInt(data.slice(index * 2, index * 2 + 2), 16);
	}
	return bytes;
}

function encodeInstruction(instruction: InstructionNode): Uint8Array {
	const parts = (instruction.discriminators ?? []).map((node) => {
		assert.equal(node.kind, "constantDiscriminatorNode");
		const { type, value } = node.constant;
		if (type.kind === "numberTypeNode" && value.kind === "numberValueNode") {
			return { offset: node.offset, bytes: encodeNumber(type, value.number) };
		}
		if (type.kind === "bytesTypeNode" && value.kind === "bytesValueNode") {
			assert.equal(value.encoding, "base16");
			return { offset: node.offset, bytes: decodeHex(value.data) };
		}
		throw new Error(`unsupported discriminator for ${instruction.name}`);
	});
	assert.ok(
		parts.length > 0,
		`${instruction.name} has no constant discriminator`,
	);

	const discriminatorSize = parts.reduce(
		(size, part) => Math.max(size, part.offset + part.bytes.length),
		0,
	);
	const args = (instruction.arguments ?? [])
		.filter((argument) => argument.name !== "discriminator")
		.map((argument) => zeroValue(argument.type));
	const bytes = new Uint8Array(
		discriminatorSize +
			args.reduce((size, argument) => size + argument.length, 0),
	);
	for (const part of parts) bytes.set(part.bytes, part.offset);
	let offset = discriminatorSize;
	for (const argument of args) {
		bytes.set(argument, offset);
		offset += argument.length;
	}
	return bytes;
}

function instructionByName(
	descriptor: ExampleDescriptor,
	name: string,
): InstructionNode {
	const instruction = descriptor.idl.program.instructions.find((candidate) =>
		candidate.name === name
	);
	assert.ok(instruction, `${descriptor.name} has no ${name} instruction`);
	return instruction;
}

function rawInstruction(
	programId: string,
	data: Uint8Array,
	accounts: Instruction["accounts"] = [],
): Instruction {
	return {
		programAddress: address(programId),
		accounts,
		data,
	};
}

async function createSubmitter(surfnet: Surfnet): Promise<{
	payer: TestSigner;
	submit(instruction: Instruction): Promise<void>;
}> {
	const payer = await createKeyPairSignerFromBytes(surfnet.payerSecretKey);
	const rpc = createSolanaRpc(surfnet.rpcUrl);
	const sendTransaction = sendTransactionWithoutConfirmingFactory({ rpc });

	return {
		payer,
		async submit(instruction: Instruction): Promise<void> {
			const { value: latestBlockhash } = await rpc.getLatestBlockhash().send();
			const message = setTransactionMessageLifetimeUsingBlockhash(
				latestBlockhash,
				appendTransactionMessageInstruction(
					instruction,
					setTransactionMessageFeePayerSigner(
						payer,
						createTransactionMessage({ version: 0 }),
					),
				),
			);
			const signed = await signTransactionMessageWithSigners(message);
			const simulated = await rpc.simulateTransaction(
				getBase64EncodedWireTransaction(signed),
				{ encoding: "base64", sigVerify: true },
			).send();
			if (simulated.value.err !== null) {
				throw new ProgramInvocationError(
					simulated.value.err,
					simulated.value.logs ?? [],
				);
			}

			// Simulating first gives negative tests stable program logs. Submit the
			// successful transaction as well so each positive case reaches Surfpool's
			// deployed SBF runtime rather than being a simulation-only smoke test.
			const signature = getSignatureFromTransaction(signed);
			await sendTransaction(signed, {
				commitment: "confirmed",
			});
			const statuses = await rpc.getSignatureStatuses([signature], {
				searchTransactionHistory: true,
			}).send();
			const status = statuses.value[0];
			assert.ok(status, `submitted transaction ${signature} has no status`);
			assert.equal(
				status.err,
				null,
				`submitted transaction ${signature} failed: ${
					JSON.stringify(status.err)
				}`,
			);
		},
	};
}

async function fetchAccountData(
	surfnet: Surfnet,
	accountAddress: string,
): Promise<Uint8Array> {
	const rpc = createSolanaRpc(surfnet.rpcUrl);
	const { value: account } = await rpc.getAccountInfo(
		address(accountAddress),
		{ encoding: "base64" },
	).send();
	assert.ok(account, `account ${accountAddress} was not created`);
	const [encoded, encoding] = account.data;
	assert.equal(encoding, "base64", "account data must be base64 encoded");
	return new Uint8Array(Buffer.from(encoded, "base64"));
}

type ExpectedProgramError = string | { Custom: bigint };

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === "object" && value !== null;
}

function instructionError(value: unknown): unknown {
	if (!isRecord(value)) return undefined;
	const error = value.InstructionError;
	if (!Array.isArray(error) || error.length !== 2) return undefined;
	return error[1];
}

async function assertRejected(
	operation: () => Promise<void>,
	message: string,
	expectedProgramError: ExpectedProgramError,
): Promise<void> {
	let error: unknown;
	try {
		await operation();
	} catch (caught) {
		error = caught;
	}
	assert.ok(
		error instanceof ProgramInvocationError,
		`${message}: ${String(error)}`,
	);
	assert.ok(
		error.logs.length > 0,
		`${message}: runtime returned no program logs`,
	);
	assert.deepEqual(
		instructionError(error.programError),
		expectedProgramError,
		`${message}: expected ${
			stringifyValue(expectedProgramError)
		}, got ${error.errorText}`,
	);
}

type ExpectedEntrypointCase = {
	instruction: string;
	accounts: "none" | "payerSigner";
	programError?: ExpectedProgramError;
};

// This is the second half of the inventory check: every program must either
// complete a concrete entrypoint call or return the exact runtime error that
// proves the dispatcher entered the selected handler. It prevents a generic
// malformed-input rejection from becoming a substitute for execution coverage.
const EXPECTED_ENTRYPOINT_CASES: Record<
	ExampleProgram,
	ExpectedEntrypointCase
> = {
	anchor_declare_id: { instruction: "initialize", accounts: "none" },
	anchor_declare_program: {
		instruction: "validateExternalProgram",
		accounts: "none",
		programError: "NotEnoughAccountKeys",
	},
	anchor_duplicate_mutable_accounts: {
		instruction: "failsDuplicateMutable",
		accounts: "none",
		programError: "NotEnoughAccountKeys",
	},
	anchor_errors: {
		instruction: "hello",
		accounts: "none",
		programError: { Custom: 6000n },
	},
	anchor_events: { instruction: "initialize", accounts: "none" },
	anchor_floats: {
		instruction: "create",
		accounts: "none",
		programError: "NotEnoughAccountKeys",
	},
	anchor_realloc: {
		instruction: "realloc",
		accounts: "none",
		programError: "NotEnoughAccountKeys",
	},
	anchor_system_accounts: {
		instruction: "initialize",
		accounts: "none",
		programError: "NotEnoughAccountKeys",
	},
	anchor_sysvars: {
		instruction: "sysvars",
		accounts: "none",
		programError: "NotEnoughAccountKeys",
	},
	counter_program: {
		instruction: "initialize",
		accounts: "none",
		programError: "NotEnoughAccountKeys",
	},
	escrow_program: {
		instruction: "make",
		accounts: "none",
		programError: "NotEnoughAccountKeys",
	},
	hello_solana: { instruction: "hello", accounts: "payerSigner" },
	optional_accounts_program: {
		instruction: "init",
		accounts: "none",
		programError: "NotEnoughAccountKeys",
	},
	pina_bpf: { instruction: "hello", accounts: "none" },
	profile_program: {
		instruction: "initialize",
		accounts: "none",
		programError: "NotEnoughAccountKeys",
	},
	prop_amm_program: {
		instruction: "initialize",
		accounts: "none",
		programError: "NotEnoughAccountKeys",
	},
	role_registry_program: {
		instruction: "initialize",
		accounts: "none",
		programError: "NotEnoughAccountKeys",
	},
	staking_rewards_program: {
		instruction: "initializePool",
		accounts: "none",
		programError: "NotEnoughAccountKeys",
	},
	todo_program: {
		instruction: "initialize",
		accounts: "none",
		programError: "NotEnoughAccountKeys",
	},
	transfer_sol: {
		instruction: "cpiTransfer",
		accounts: "none",
		programError: "NotEnoughAccountKeys",
	},
	vesting_program: {
		instruction: "initialize",
		accounts: "none",
		programError: "NotEnoughAccountKeys",
	},
};

const ACCESS_GUARD_PROGRAMS: Partial<
	Record<ExampleProgram, ExpectedProgramError>
> = {
	anchor_declare_program: "MissingRequiredSignature",
	anchor_floats: "InvalidAccountData",
	anchor_realloc: "InvalidAccountData",
	anchor_system_accounts: "MissingRequiredSignature",
	counter_program: "InvalidAccountData",
	escrow_program: "InvalidAccountData",
	hello_solana: "MissingRequiredSignature",
	optional_accounts_program: "MissingRequiredSignature",
	profile_program: "InvalidAccountData",
	prop_amm_program: "InvalidAccountData",
	role_registry_program: "InvalidAccountData",
	staking_rewards_program: "InvalidAccountData",
	todo_program: "InvalidAccountData",
	transfer_sol: "InvalidAccountData",
	vesting_program: "InvalidAccountData",
};

async function runExpectedEntrypointCase(
	descriptor: ExampleDescriptor,
	submit: (instruction: Instruction) => Promise<void>,
	payerAddress: string,
): Promise<void> {
	const expected = EXPECTED_ENTRYPOINT_CASES[descriptor.name];
	const data = encodeInstruction(
		instructionByName(descriptor, expected.instruction),
	);
	const accounts = expected.accounts === "payerSigner"
		? [{
			address: address(payerAddress),
			role: AccountRole.READONLY_SIGNER,
		}]
		: [];

	if (expected.programError) {
		await assertRejected(
			() => submit(rawInstruction(descriptor.programId, data, accounts)),
			`${descriptor.name}.${expected.instruction} did not reject its documented boundary`,
			expected.programError,
		);
		return;
	}

	await submit(rawInstruction(descriptor.programId, data, accounts));
}

async function runAccessGuardCase(
	descriptor: ExampleDescriptor,
	submit: (instruction: Instruction) => Promise<void>,
	payerAddress: string,
): Promise<void> {
	const expectedProgramError = ACCESS_GUARD_PROGRAMS[descriptor.name];
	if (!expectedProgramError) return;

	const instruction = instructionByName(
		descriptor,
		descriptor.name === "optional_accounts_program"
			? "inspect"
			: EXPECTED_ENTRYPOINT_CASES[descriptor.name].instruction,
	);
	const accountCount = instruction.accounts?.length ?? 0;
	assert.ok(accountCount > 0, `${descriptor.name} access case has no accounts`);
	const attackerAccounts = descriptor.name === "optional_accounts_program"
		? [
			{
				address: address(payerAddress),
				role: AccountRole.READONLY_SIGNER,
			},
			{
				address: address(descriptor.programId),
				role: AccountRole.READONLY,
			},
			{
				address: address(Surfnet.newKeypair().publicKey),
				role: AccountRole.READONLY,
			},
		]
		: Array.from({ length: accountCount }, () => ({
			address: address(Surfnet.newKeypair().publicKey),
			role: AccountRole.READONLY,
		}));
	assert.equal(attackerAccounts.length, accountCount);

	await assertRejected(
		() =>
			submit(rawInstruction(
				descriptor.programId,
				encodeInstruction(instruction),
				attackerAccounts,
			)),
		`${descriptor.name} accepted attacker-controlled readonly account metadata`,
		expectedProgramError,
	);
}

async function runSpecificGuards(
	descriptor: ExampleDescriptor,
	submit: (instruction: Instruction) => Promise<void>,
	payer: TestSigner,
	surfnet: Surfnet,
): Promise<void> {
	const payerAddress = String(payer.address);
	const payerSigner = {
		address: address(payerAddress),
		role: AccountRole.READONLY_SIGNER,
	};
	const payerWritableSigner = {
		address: address(payerAddress),
		role: AccountRole.WRITABLE_SIGNER,
	};

	switch (descriptor.name) {
		case "anchor_realloc": {
			await runAnchorReallocGuards(descriptor, submit, payer, surfnet);
			return;
		}
		case "hello_solana": {
			const data = encodeInstruction(instructionByName(descriptor, "hello"));
			await assertRejected(
				() => {
					const unsignedUser = Surfnet.newKeypair().publicKey;
					return submit(rawInstruction(descriptor.programId, data, [{
						address: address(unsignedUser),
						role: AccountRole.READONLY,
					}]));
				},
				"hello_solana accepted an unsigned user account",
				"MissingRequiredSignature",
			);
			return;
		}
		case "anchor_duplicate_mutable_accounts": {
			const data = encodeInstruction(
				instructionByName(descriptor, "failsDuplicateMutable"),
			);
			await assertRejected(
				() =>
					submit(rawInstruction(descriptor.programId, data, [
						payerWritableSigner,
						payerWritableSigner,
					])),
				"duplicate mutable accounts were accepted",
				{ Custom: 2040n },
			);
			await assertRejected(
				() => {
					const nonWritable = Surfnet.newKeypair().publicKey;
					return submit(rawInstruction(descriptor.programId, data, [
						{ address: address(nonWritable), role: AccountRole.READONLY },
						{ address: address(nonWritable), role: AccountRole.READONLY },
					]));
				},
				"a non-writable mutable account was accepted",
				"InvalidAccountData",
			);
			return;
		}
		case "anchor_declare_program": {
			const data = encodeInstruction(
				instructionByName(descriptor, "validateExternalProgram"),
			);
			await assertRejected(
				() =>
					submit(rawInstruction(descriptor.programId, data, [payerSigner, {
						address: address(payerAddress),
						role: AccountRole.READONLY,
					}])),
				"an untrusted external program address was accepted",
				"IncorrectProgramId",
			);
			return;
		}
		case "anchor_system_accounts": {
			const data = encodeInstruction(
				instructionByName(descriptor, "initialize"),
			);
			const nonSystemWallet = Surfnet.newKeypair().publicKey;
			surfnet.setAccount(
				nonSystemWallet,
				1,
				new Uint8Array(),
				descriptor.programId,
			);
			await assertRejected(
				() =>
					submit(rawInstruction(descriptor.programId, data, [payerSigner, {
						address: address(nonSystemWallet),
						role: AccountRole.READONLY,
					}])),
				"a wallet with a non-system owner was accepted",
				"InvalidAccountOwner",
			);
			return;
		}
		case "anchor_sysvars": {
			const data = encodeInstruction(instructionByName(descriptor, "sysvars"));
			await assertRejected(
				() =>
					submit(rawInstruction(descriptor.programId, data, [
						{ address: address(payerAddress), role: AccountRole.READONLY },
						{ address: address(payerAddress), role: AccountRole.READONLY },
						{ address: address(payerAddress), role: AccountRole.READONLY },
					])),
				"forged sysvar account addresses were accepted",
				"InvalidAccountOwner",
			);
			return;
		}
		default:
			return;
	}
}

const SYSTEM_PROGRAM_ID = "11111111111111111111111111111111";
const SAMPLE_HEADER_SIZE = 34;

function reallocInstructionData(
	discriminator: number,
	len: number,
): Uint8Array {
	assert.ok(Number.isInteger(len) && len >= 0 && len <= 0xffff);
	return Uint8Array.of(discriminator, len & 0xff, len >>> 8);
}

async function deriveSampleAddress(
	descriptor: ExampleDescriptor,
	authority: string,
) {
	return getProgramDerivedAddress({
		programAddress: address(descriptor.programId),
		seeds: ["sample", getAddressEncoder().encode(address(authority))],
	});
}

function assertSampleHeader(
	data: Uint8Array,
	bump: number,
	authority: string,
): void {
	assert.ok(
		data.length >= SAMPLE_HEADER_SIZE,
		"sample must retain its authenticated header",
	);
	assert.equal(data[0], 1, "sample discriminator changed");
	assert.equal(data[1], bump, "sample PDA bump changed");
	assert.deepEqual(
		data.slice(2, SAMPLE_HEADER_SIZE),
		getAddressEncoder().encode(address(authority)),
		"sample authority changed",
	);
}

// These cases exercise the security contract introduced by the authority-bound
// `Sample` PDA. They demonstrate the listed attacks fail without changing the
// victim account; they are not a proof that the program has no other bugs.
async function runAnchorReallocGuards(
	descriptor: ExampleDescriptor,
	submit: (instruction: Instruction) => Promise<void>,
	payer: TestSigner,
	surfnet: Surfnet,
): Promise<void> {
	const authority = String(payer.address);
	const payerWritableSigner = {
		address: payer.address,
		role: AccountRole.WRITABLE_SIGNER,
	};
	const systemProgram = {
		address: address(SYSTEM_PROGRAM_ID),
		role: AccountRole.READONLY,
	};
	const [sample, bump] = await deriveSampleAddress(descriptor, authority);
	const sampleWritable = { address: sample, role: AccountRole.WRITABLE };

	await submit(rawInstruction(descriptor.programId, Uint8Array.of(2, bump), [
		payerWritableSigner,
		sampleWritable,
		systemProgram,
	]));
	const initializedData = await fetchAccountData(surfnet, String(sample));
	assert.equal(initializedData.length, SAMPLE_HEADER_SIZE);
	assertSampleHeader(initializedData, bump, authority);

	await submit(rawInstruction(
		descriptor.programId,
		reallocInstructionData(0, 98),
		[payerWritableSigner, sampleWritable, systemProgram],
	));
	const grownData = await fetchAccountData(surfnet, String(sample));
	assert.equal(grownData.length, 98, "authorized growth did not resize sample");
	assertSampleHeader(grownData, bump, authority);

	const attacker = await createKeyPairSignerFromBytes(
		new Uint8Array(Surfnet.newKeypair().secretKey),
	);
	surfnet.fundSol(String(attacker.address), 1_000_000_000);
	const attackerWritableSigner = {
		address: attacker.address,
		role: AccountRole.WRITABLE_SIGNER,
	};
	await assertRejected(
		() =>
			submit(addSignersToInstruction(
				[attacker],
				rawInstruction(
					descriptor.programId,
					reallocInstructionData(0, SAMPLE_HEADER_SIZE),
					[attackerWritableSigner, sampleWritable, systemProgram],
				),
			)),
		"an unrelated signer resized the victim's canonical sample PDA",
		"InvalidSeeds",
	);
	assert.deepEqual(
		await fetchAccountData(surfnet, String(sample)),
		grownData,
		"an unrelated signer mutated the victim sample",
	);

	const forgedAddress = Surfnet.newKeypair().publicKey;
	const forgedData = new Uint8Array(SAMPLE_HEADER_SIZE);
	forgedData[0] = 1;
	forgedData[1] = bump;
	forgedData.set(getAddressEncoder().encode(payer.address), 2);
	surfnet.setAccount(
		forgedAddress,
		1_000_000,
		forgedData,
		descriptor.programId,
	);
	await assertRejected(
		() =>
			submit(rawInstruction(
				descriptor.programId,
				reallocInstructionData(0, 98),
				[
					payerWritableSigner,
					{ address: address(forgedAddress), role: AccountRole.WRITABLE },
					systemProgram,
				],
			)),
		"a forged program-owned Sample header bypassed canonical PDA validation",
		"InvalidSeeds",
	);
	assert.deepEqual(
		await fetchAccountData(surfnet, forgedAddress),
		forgedData,
		"a forged Sample account was mutated",
	);

	await assertRejected(
		() =>
			submit(rawInstruction(
				descriptor.programId,
				reallocInstructionData(1, 98),
				[payerWritableSigner, sampleWritable, sampleWritable, systemProgram],
			)),
		"duplicate resize targets were accepted",
		{ Custom: 3017n },
	);
	assert.deepEqual(
		await fetchAccountData(surfnet, String(sample)),
		grownData,
		"duplicate resize targets mutated the sample",
	);

	await submit(rawInstruction(
		descriptor.programId,
		reallocInstructionData(0, SAMPLE_HEADER_SIZE),
		[payerWritableSigner, sampleWritable, systemProgram],
	));
	const shrunkData = await fetchAccountData(surfnet, String(sample));
	assert.equal(shrunkData.length, SAMPLE_HEADER_SIZE);
	assertSampleHeader(shrunkData, bump, authority);
}

async function runExample(descriptor: ExampleDescriptor): Promise<void> {
	const surfnet = Surfnet.start();
	try {
		surfnet.deploy({
			programId: descriptor.programId,
			soPath: descriptor.artifactPath,
		});

		const { payer, submit } = await createSubmitter(surfnet);
		const firstInstruction = instructionByName(
			descriptor,
			EXPECTED_ENTRYPOINT_CASES[descriptor.name].instruction,
		);
		const canonicalData = encodeInstruction(firstInstruction);
		const invalidData = canonicalData.slice();
		invalidData.fill(0xff, 0, Math.min(1, invalidData.length));

		await assertRejected(
			() => submit(rawInstruction(descriptor.programId, invalidData)),
			`${descriptor.name} accepted an unknown discriminator`,
			"InvalidInstructionData",
		);

		const wrongProgramId = Surfnet.newKeypair().publicKey;
		surfnet.deploy({
			programId: wrongProgramId,
			soPath: descriptor.artifactPath,
		});
		await assertRejected(
			() => submit(rawInstruction(wrongProgramId, canonicalData)),
			`${descriptor.name} ran when deployed at an unexpected program ID`,
			"IncorrectProgramId",
		);

		await runExpectedEntrypointCase(
			descriptor,
			submit,
			String(payer.address),
		);
		await runAccessGuardCase(descriptor, submit, String(payer.address));

		await runSpecificGuards(
			descriptor,
			submit,
			payer,
			surfnet,
		);
	} finally {
		surfnet.stop();
	}
}

assertInventory();
console.log(
	`Running Surfpool security checks for ${EXAMPLE_PROGRAMS.length} examples.`,
);

for (const name of EXAMPLE_PROGRAMS) {
	const idl = readIdl(name);
	const descriptor: ExampleDescriptor = {
		name,
		programId: idl.program.publicKey,
		artifactPath: artifactPath(name),
		idl,
	};
	console.log(`- ${name}`);
	await runExample(descriptor);
}

console.log("Surfpool security checks passed for every example program.");
