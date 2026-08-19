import {
	getU16Decoder,
	getU8Decoder,
	isNone,
	isSome,
	type ReadonlyUint8Array,
} from "@solana/kit";
import { describe, expect, test } from "vitest";

import {
	getAddTagInstructionDataEncoder,
	getInitializeInstructionDataDecoder,
	getInitializeInstructionDataEncoder,
	getProfileStateDecoder,
	getProfileStateEncoder,
} from "../../../clients/js/profile_program/src";
import {
	getZeroPodEnumDecoder,
	getZeroPodOptionTagDecoder,
} from "../../../clients/js/profile_program/src/generated/zeropodCodecs";

describe("profile generated codecs", () => {
	test("instruction encoders prepend their discriminator", () => {
		const initialize = getInitializeInstructionDataEncoder().encode({
			bump: 42,
			name: boundedText("A", 32),
			bio: boundedText("hello", 128),
		});
		const addTag = getAddTagInstructionDataEncoder().encode({ tag: 10 });

		expect(initialize).toHaveLength(164);
		expect(Array.from(initialize.slice(0, 4))).toEqual([0, 42, 1, 65]);
		expect(addTag).toHaveLength(9);
		expect(Array.from(addTag.slice(0, 2))).toEqual([2, 10]);
	});

	test("account decoder consumes the discriminator before state", () => {
		const data = new Uint8Array(240);
		data[0] = 1;
		data[1] = 42;
		data[2] = 1;
		data[3] = "A".charCodeAt(0);
		data[239] = 1;

		const state = getProfileStateDecoder().decode(data);

		expect(state.discriminator).toBe(1);
		expect(state.bump).toBe(42);
		expect(readBoundedText(state.name)).toBe("A");
		expect(readBoundedText(state.bio)).toBe("");
		expect(readBoundedTags(state.tags)).toEqual([]);
		expect(isNone(state.favoriteTag)).toBe(true);
		expect(state.active).toBe(true);
	});

	test("semantic helpers reject bounded lengths beyond capacity", () => {
		const invalidName = new Uint8Array(240);
		invalidName[0] = 1;
		invalidName[2] = 33;

		const invalidTags = new Uint8Array(240);
		invalidTags[0] = 1;
		invalidTags[164] = 9;

		expect(() =>
			readBoundedText(getProfileStateDecoder().decode(invalidName).name)
		).toThrow(/capacity/);
		expect(() =>
			readBoundedTags(getProfileStateDecoder().decode(invalidTags).tags)
		).toThrow(/capacity/);
	});

	test("bounded construction helpers reject values beyond capacity", () => {
		expect(() => boundedText("x".repeat(33), 32)).toThrow(/capacity/);
		expect(() =>
			boundedTags(Array.from({ length: 9 }, (_, index) => BigInt(index)))
		)
			.toThrow(/capacity/);
	});

	test("decoders reject the wrong discriminator and non-canonical booleans", () => {
		const wrongInstruction = new Uint8Array(164);
		wrongInstruction[0] = 1;
		expect(() => getInitializeInstructionDataDecoder().decode(wrongInstruction))
			.toThrow(/invalid discriminator/);

		const account = emptyProfileAccount();
		account[239] = 2;
		expect(() => getProfileStateDecoder().decode(account)).toThrow(
			/invalid zeropod boolean/,
		);
	});

	test("option codecs preserve values and reject non-canonical tags", () => {
		const encoded = getProfileStateEncoder().encode({
			bump: 7,
			name: boundedText("", 32),
			bio: boundedText("", 128),
			tags: boundedTags([]),
			favoriteTag: 42n,
			active: true,
		});
		expect(Array.from(encoded.slice(230, 239))).toEqual([
			1,
			42,
			0,
			0,
			0,
			0,
			0,
			0,
			0,
		]);
		const decoded = getProfileStateDecoder().decode(encoded);
		expect(isSome(decoded.favoriteTag)).toBe(true);
		if (isSome(decoded.favoriteTag)) {
			expect(decoded.favoriteTag.value).toBe(42n);
		}

		const invalid = emptyProfileAccount();
		invalid[230] = 2;
		expect(() => getProfileStateDecoder().decode(invalid)).toThrow(
			/invalid zeropod option tag/,
		);

		const inactivePayload = emptyProfileAccount();
		inactivePayload.fill(0xff, 231, 239);
		expect(
			isNone(getProfileStateDecoder().decode(inactivePayload).favoriteTag),
		).toBe(true);
	});

	test("wide option tags accept only zero and one", () => {
		const decoder = getZeroPodOptionTagDecoder(getU16Decoder());
		expect(decoder.decode(Uint8Array.of(0, 0))).toBe(0);
		expect(decoder.decode(Uint8Array.of(1, 0))).toBe(1);
		expect(() => decoder.decode(Uint8Array.of(2, 0))).toThrow(
			/invalid zeropod option tag/,
		);
		expect(() => decoder.decode(Uint8Array.of(0, 1))).toThrow(
			/invalid zeropod option tag/,
		);
	});

	test("bounded text validation is strict UTF-8 and preserves embedded NULs", () => {
		const account = emptyProfileAccount();
		account.set([3, 65, 0, 66], 2);
		expect(readBoundedText(getProfileStateDecoder().decode(account).name))
			.toBe("A\0B");

		account.set([1, 0xff, 0, 0], 2);
		expect(() => readBoundedText(getProfileStateDecoder().decode(account).name))
			.toThrow();
	});

	test("enum validation rejects undeclared numeric variants", () => {
		const decoder = getZeroPodEnumDecoder(getU8Decoder(), [0, 1]);
		expect(decoder.decode(Uint8Array.of(1))).toBe(1);
		expect(() => decoder.decode(Uint8Array.of(2))).toThrow(
			/invalid zeropod enum discriminant/,
		);
	});
});

function emptyProfileAccount(): Uint8Array {
	const account = new Uint8Array(240);
	account[0] = 1;
	return account;
}

function boundedText(value: string, capacity: number): Uint8Array {
	const valueBytes = new TextEncoder().encode(value);
	if (valueBytes.length > capacity) {
		throw new RangeError("bounded text exceeds capacity");
	}

	const bytes = new Uint8Array(capacity + 1);
	bytes[0] = valueBytes.length;
	bytes.set(valueBytes, 1);
	return bytes;
}

function readBoundedText(bytes: ReadonlyUint8Array): string {
	const length = bytes[0] ?? 0;
	if (length > bytes.length - 1) {
		throw new RangeError("bounded text length exceeds capacity");
	}

	return new TextDecoder("utf-8", { fatal: true }).decode(
		bytes.subarray(1, length + 1),
	);
}

function boundedTags(values: readonly bigint[]): Uint8Array {
	const capacity = 8;
	if (values.length > capacity) {
		throw new RangeError("bounded tags exceed capacity");
	}

	const bytes = new Uint8Array(2 + capacity * 8);
	const view = new DataView(bytes.buffer);
	view.setUint16(0, values.length, true);
	for (const [index, value] of values.entries()) {
		view.setBigUint64(2 + index * 8, value, true);
	}
	return bytes;
}

function readBoundedTags(bytes: ReadonlyUint8Array): bigint[] {
	const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
	const length = view.getUint16(0, true);
	if (length > 8) {
		throw new RangeError("bounded tag length exceeds capacity");
	}

	return Array.from(
		{ length },
		(_, index) => view.getBigUint64(2 + index * 8, true),
	);
}
