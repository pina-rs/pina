/**
 * Language-neutral CLI model extracted from a Codama root node.
 *
 * Both the TypeScript and Dart emitters consume this shape so the mapping
 * rules (typed argument flags, account resolution, PDA derivation, fetch
 * commands) stay identical across languages.
 */

export type ArgKind =
	| { kind: "number"; format: string }
	| { kind: "bool" }
	| { kind: "pubkey" }
	| { kind: "fixedBytes"; size: number }
	| { kind: "byteVec"; capacity: number }
	| { kind: "fixedString"; capacity: number };

export interface ArgModel {
	snake: string;
	camel: string;
	docs: string[];
	type: ArgKind;
}

export type SeedModel =
	| { seed: "utf8"; value: string }
	| { seed: "pubkey"; value: string }
	| { seed: "account"; account: string }
	| { seed: "arg"; arg: string; type: ArgKind };

export type Resolution =
	| { resolution: "constant"; address: string }
	| { resolution: "pda"; pascal: string; seeds: SeedModel[] }
	| { resolution: "payer" }
	| { resolution: "required" };

export interface AccountRefModel {
	snake: string;
	camel: string;
	docs: string[];
	isOptional: boolean;
	resolution: Resolution;
}

export interface InstructionModel {
	snake: string;
	camel: string;
	pascal: string;
	docs: string[];
	args: ArgModel[];
	accounts: AccountRefModel[];
	/** The generated client exposes an `…InstructionAsync` builder when any
	 * account default needs PDA resolution at send time. */
	hasAsyncBuilder: boolean;
}

export type FieldKind =
	| { kind: "number" }
	| { kind: "bool" }
	| { kind: "pubkey" }
	| { kind: "string" }
	| { kind: "bytes" }
	| { kind: "vec" }
	| { kind: "option" };

export interface FieldModel {
	snake: string;
	camel: string;
	type: FieldKind;
}

export interface FetchSeedModel {
	snake: string;
	camel: string;
	type: ArgKind;
	constant: SeedModel | null;
}

export interface AccountModel {
	snake: string;
	camel: string;
	pascal: string;
	docs: string[];
	seeds: FetchSeedModel[] | null;
	fields: FieldModel[];
	/** Pascal name of the linked PDA node, e.g. `Counter` for `findCounterPda`. */
	pdaPascal: string | null;
}

export interface ErrorModel {
	pascal: string;
	code: number;
	message: string;
}

export interface CliModel {
	programCamel: string;
	programSnake: string;
	programKebab: string;
	programAddress: string;
	programVersion: string;
	about: string;
	instructions: InstructionModel[];
	accounts: AccountModel[];
	errors: ErrorModel[];
}

const toSnake = (value: string): string =>
	value.replace(/([a-z0-9])([A-Z])/g, "$1_$2").toLowerCase();

const toCamel = (value: string): string => {
	const snake = toSnake(value);
	return snake.replace(
		/_([a-z0-9])/g,
		(_, letter: string) => letter.toUpperCase(),
	);
};

const toPascal = (value: string): string => {
	const camel = toCamel(value);
	return camel.charAt(0).toUpperCase() + camel.slice(1);
};

const numberFormats = new Set([
	"u8",
	"i8",
	"u16",
	"i16",
	"u32",
	"i32",
	"u64",
	"i64",
]);

function argKind(type: unknown, context: string): ArgKind {
	const node = type as {
		kind?: string;
		format?: string;
		size?: number;
		type?: unknown;
	};
	switch (node?.kind) {
		case "numberTypeNode": {
			if (!numberFormats.has(String(node.format))) {
				throw new Error(
					`${context}: number format \`${node.format}\` is not supported by CLI generation`,
				);
			}
			return { kind: "number", format: String(node.format) };
		}
		case "booleanTypeNode":
			return { kind: "bool" };
		case "publicKeyTypeNode":
			return { kind: "pubkey" };
		case "fixedSizeTypeNode": {
			const inner = node.type as {
				kind?: string;
				type?: unknown;
				count?: unknown;
			};
			if (inner?.kind === "bytesTypeNode") {
				return { kind: "fixedBytes", size: Number(node.size) };
			}
			if (inner?.kind === "sizePrefixTypeNode") {
				const prefixed = inner.type as { kind?: string };
				if (prefixed?.kind === "stringTypeNode") {
					return { kind: "fixedString", capacity: Number(node.size) };
				}
				throw new Error(
					`${context}: only UTF-8 size-prefixed strings are supported`,
				);
			}
			if (inner?.kind === "arrayTypeNode") {
				const item =
					(inner as { item?: { kind?: string; format?: string } }).item;
				const count = inner.count as { kind?: string; value?: number };
				if (item?.kind !== "numberTypeNode" || item.format !== "u8") {
					throw new Error(`${context}: only u8 array items are supported`);
				}
				if (count?.kind === "fixedCountNode") {
					return { kind: "fixedBytes", size: Number(count.value) };
				}
				return { kind: "byteVec", capacity: Number(node.size) };
			}
			throw new Error(
				`${context}: fixed-size bytes, strings, or u8 arrays are supported`,
			);
		}
		default:
			throw new Error(
				`${context}: type \`${
					node?.kind ?? "unknown"
				}\` is not supported by CLI generation`,
			);
	}
}

function isOmitted(field: { defaultValueStrategy?: string }): boolean {
	return field.defaultValueStrategy === "omitted";
}

function seedModel(
	seed: unknown,
	resolved: string[],
	args: ArgModel[],
	context: string,
): SeedModel {
	const node = seed as {
		name?: string;
		value?: {
			kind?: string;
			name?: string;
			string?: string;
			publicKey?: string;
		};
	};
	const value = node.value;
	if (!value) {
		throw new Error(`${context}: PDA seed \`${node.name}\` has no value`);
	}
	switch (value.kind) {
		case "accountValueNode": {
			const account = toSnake(String(value.name));
			if (!resolved.includes(account)) {
				throw new Error(
					`${context}: PDA seed \`${node.name}\` depends on account \`${account}\`, which must be declared first`,
				);
			}
			return { seed: "account", account };
		}
		case "argumentValueNode": {
			const arg = toSnake(String(value.name));
			const match = args.find((candidate) => candidate.snake === arg);
			if (
				!match || (match.type.kind !== "pubkey" && match.type.kind !== "number")
			) {
				throw new Error(
					`${context}: PDA seed \`${node.name}\` must reference a pubkey or number argument`,
				);
			}
			return { seed: "arg", arg, type: match.type };
		}
		case "stringValueNode":
			return { seed: "utf8", value: String(value.string) };
		case "publicKeyValueNode":
			return { seed: "pubkey", value: String(value.publicKey) };
		case "numberValueNode":
			return {
				seed: "utf8",
				value: String((value as { number?: number }).number),
			};
		default:
			throw new Error(
				`${context}: PDA seed \`${node.name}\` uses an unsupported \`${value.kind}\` value`,
			);
	}
}

function fieldKind(type: unknown, context: string): FieldKind {
	const unwrap = (node: unknown): unknown => {
		const inner = node as { kind?: string; type?: unknown };
		if (
			inner?.kind === "preOffsetTypeNode" ||
			inner?.kind === "postOffsetTypeNode"
		) {
			return unwrap(inner.type);
		}
		return node;
	};
	const arg = (node: unknown): ArgKind => argKind(node, context);

	const node = unwrap(type) as {
		kind?: string;
		type?: unknown;
		item?: unknown;
		count?: unknown;
		fixed?: boolean;
	};
	switch (node?.kind) {
		case "numberTypeNode":
			arg(node);
			return { kind: "number" };
		case "booleanTypeNode":
			return { kind: "bool" };
		case "publicKeyTypeNode":
			return { kind: "pubkey" };
		case "bytesTypeNode":
			return { kind: "bytes" };
		case "fixedSizeTypeNode": {
			const inner = unwrap(node.type) as { kind?: string; type?: unknown };
			if (inner?.kind === "bytesTypeNode") {
				return { kind: "bytes" };
			}
			if (inner?.kind === "sizePrefixTypeNode") {
				const prefixed = inner.type as { kind?: string };
				if (prefixed?.kind === "stringTypeNode") {
					return { kind: "string" };
				}
				throw new Error(
					`${context}: only UTF-8 size-prefixed strings are supported`,
				);
			}
			if (inner?.kind === "arrayTypeNode") {
				const item =
					(inner as { item?: { kind?: string; format?: string } }).item;
				if (
					item?.kind !== "numberTypeNode" ||
					!numberFormats.has(String(item.format))
				) {
					throw new Error(`${context}: only number array items are supported`);
				}
				return { kind: "vec" };
			}
			throw new Error(`${context}: unsupported fixed-size field`);
		}
		case "sizePrefixTypeNode": {
			const prefixed = node.type as { kind?: string };
			if (prefixed?.kind === "stringTypeNode") {
				return { kind: "string" };
			}
			throw new Error(
				`${context}: only UTF-8 size-prefixed strings are supported`,
			);
		}
		case "optionTypeNode": {
			const item = unwrap(node.item) as { kind?: string; type?: unknown };
			if (item?.kind === "numberTypeNode") {
				arg(item);
				return { kind: "option" };
			}
			if (item?.kind === "sizePrefixTypeNode") {
				const prefixed = item.type as { kind?: string };
				if (prefixed?.kind === "stringTypeNode") {
					return { kind: "string" };
				}
			}
			throw new Error(
				`${context}: only optional numbers and strings are supported`,
			);
		}
		case "arrayTypeNode": {
			const item = node.item as { kind?: string; format?: string };
			if (
				item?.kind !== "numberTypeNode" ||
				!numberFormats.has(String(item.format))
			) {
				throw new Error(`${context}: only number array items are supported`);
			}
			return { kind: "vec" };
		}
		default:
			throw new Error(
				`${context}: type \`${
					node?.kind ?? "unknown"
				}\` is not supported by CLI generation`,
			);
	}
}

function fetchSeeds(
	pda: { seeds?: unknown[] },
	context: string,
): FetchSeedModel[] {
	return (pda.seeds ?? []).flatMap((seed): FetchSeedModel[] => {
		const node = seed as {
			kind?: string;
			name?: string;
			type?: unknown;
			value?: {
				kind?: string;
				string?: string;
				publicKey?: string;
				number?: number;
			};
		};
		if (node.kind === "constantPdaSeedNode") {
			const value = node.value;
			if (value?.kind === "stringValueNode") {
				return [{
					snake: "",
					camel: "",
					type: { kind: "pubkey" },
					constant: { seed: "utf8", value: String(value.string) },
				}];
			}
			if (value?.kind === "publicKeyValueNode") {
				return [{
					snake: "",
					camel: "",
					type: { kind: "pubkey" },
					constant: { seed: "pubkey", value: String(value.publicKey) },
				}];
			}
			if (value?.kind === "numberValueNode") {
				return [{
					snake: "",
					camel: "",
					type: { kind: "number", format: "u64" },
					constant: { seed: "utf8", value: String(value.number) },
				}];
			}
			throw new Error(`${context}: unsupported constant PDA seed`);
		}
		if (node.kind === "variablePdaSeedNode") {
			return [{
				snake: toSnake(String(node.name)),
				camel: toCamel(String(node.name)),
				type: argKind(node.type, context),
				constant: null,
			}];
		}
		throw new Error(`${context}: unsupported PDA seed node`);
	});
}

/** Extract the CLI model from a Codama root node. */
export function extractCliModel(root: {
	program: {
		name: string;
		publicKey: string;
		version: string;
		docs?: string[];
		instructions?: unknown[];
		accounts?: unknown[];
		pdas?: { name: string; seeds?: unknown[] }[];
		errors?: unknown[];
	};
}): CliModel {
	const program = root.program;
	const programSnake = toSnake(program.name);
	const programCamel = toCamel(program.name);
	const firstDoc = (program.docs ?? []).find((line) => line.trim().length > 0);

	const instructions: InstructionModel[] = (program.instructions ?? []).map(
		(raw) => {
			const node = raw as {
				name: string;
				docs?: string[];
				arguments?: {
					name: string;
					docs?: string[];
					type: unknown;
					defaultValueStrategy?: string;
				}[];
				accounts?: {
					name: string;
					docs?: string[];
					isSigner?: string | boolean;
					isOptional?: boolean;
					defaultValue?: {
						kind: string;
						publicKey?: string;
						pda?: { kind: string; name?: string };
						seeds?: unknown[];
					};
				}[];
			};
			const context = `instruction \`${node.name}\``;
			const args = (node.arguments ?? [])
				.filter((argument) => !isOmitted(argument))
				.map((argument) => ({
					snake: toSnake(argument.name),
					camel: toCamel(argument.name),
					docs: argument.docs ?? [],
					type: argKind(
						argument.type,
						`${context} argument \`${argument.name}\``,
					),
				}));

			const resolved: string[] = [];
			const accounts = (node.accounts ?? []).map((account) => {
				const snake = toSnake(account.name);
				const defaultValue = account.defaultValue;
				let resolution: Resolution;
				if (defaultValue?.kind === "publicKeyValueNode") {
					resolution = {
						resolution: "constant",
						address: String(defaultValue.publicKey),
					};
				} else if (defaultValue?.kind === "payerValueNode") {
					resolution = { resolution: "payer" };
				} else if (defaultValue?.kind === "pdaValueNode") {
					const seeds = (defaultValue.seeds ?? []).map((seed) =>
						seedModel(
							seed,
							resolved,
							args,
							`${context} account \`${account.name}\``,
						)
					);
					const pdaName = defaultValue.pda?.name ?? account.name;
					resolution = {
						resolution: "pda",
						pascal: toPascal(String(pdaName)),
						seeds,
					};
				} else if (defaultValue) {
					throw new Error(
						`${context}: account \`${account.name}\` uses an unsupported \`${defaultValue.kind}\` default`,
					);
				} else if (account.isSigner === true || account.isSigner === "either") {
					resolution = { resolution: "payer" };
				} else {
					resolution = { resolution: "required" };
				}
				resolved.push(snake);
				return {
					snake,
					camel: toCamel(account.name),
					docs: account.docs ?? [],
					isOptional: account.isOptional ?? false,
					resolution,
				};
			});

			return {
				snake: toSnake(node.name),
				camel: toCamel(node.name),
				pascal: toPascal(node.name),
				docs: node.docs ?? [],
				args,
				accounts,
				hasAsyncBuilder: accounts.some(
					(account) => account.resolution.resolution === "pda",
				),
			};
		},
	);

	const accounts: AccountModel[] = (program.accounts ?? []).map((raw) => {
		const node = raw as {
			name: string;
			docs?: string[];
			data?: {
				kind: string;
				fields?: {
					name: string;
					type: unknown;
					defaultValueStrategy?: string;
				}[];
			};
			pda?: { kind: string; name: string } | null;
		};
		const pascal = toPascal(node.name);
		const context = `account \`${pascal}\``;
		const pdaName = node.pda?.name;
		const pda = (program.pdas ?? []).find((candidate) =>
			candidate.name === pdaName
		) ?? null;
		return {
			snake: toSnake(node.name),
			camel: toCamel(node.name),
			pascal,
			docs: node.docs ?? [],
			seeds: pda ? fetchSeeds(pda, context) : null,
			pdaPascal: pdaName ? toPascal(pdaName) : null,
			fields: (node.data?.fields ?? [])
				.filter((field) => !isOmitted(field))
				.map((field) => ({
					snake: toSnake(field.name),
					camel: toCamel(field.name),
					type: fieldKind(field.type, `${context} field \`${field.name}\``),
				})),
		};
	});

	const errors: ErrorModel[] = (program.errors ?? []).map((raw) => {
		const node = raw as { name: string; code: number; message?: string };
		return {
			pascal: toPascal(node.name),
			code: Number(node.code),
			message: node.message ?? "",
		};
	});

	return {
		programCamel,
		programSnake,
		programKebab: programSnake.replace(/_/g, "-"),
		programAddress: program.publicKey,
		programVersion: program.version,
		about: firstDoc?.replace(/\.$/, "") ??
			`Command-line interface for ${programCamel}`,
		instructions,
		accounts,
		errors,
	};
}
