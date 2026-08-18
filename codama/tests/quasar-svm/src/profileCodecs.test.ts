import { getU8Decoder } from "@solana/kit";
import { describe, expect, test } from "vitest";

import {
	getAddTagInstructionDataEncoder,
	getInitializeInstructionDataDecoder,
	getInitializeInstructionDataEncoder,
	getProfileStateDecoder,
	getProfileStateEncoder,
} from "../../../clients/js/profile_program/src";
import { getZeroPodEnumDecoder } from "../../../clients/js/profile_program/src/generated/zeropodCodecs";

describe("profile generated codecs", () => {
	test("instruction encoders prepend their discriminator", () => {
		const initialize = getInitializeInstructionDataEncoder().encode({
			bump: 42,
			name: "A",
			bio: "hello",
		});
		const addTag = getAddTagInstructionDataEncoder().encode({ tag: 10 });

		expect(initialize).toHaveLength(164);
		expect(Array.from(initialize.slice(0, 4))).toEqual([0, 42, 1, 65]);
		expect(addTag).toHaveLength(9);
		expect(Array.from(addTag.slice(0, 2))).toEqual([2, 10]);
	});

	test("account decoder consumes the discriminator before state", () => {
		const data = new Uint8Array(231);
		data[0] = 1;
		data[1] = 42;
		data[2] = 1;
		data[3] = "A".charCodeAt(0);
		data[230] = 1;

		const state = getProfileStateDecoder().decode(data);

		expect(state.discriminator).toBe(1);
		expect(state.bump).toBe(42);
		expect(state.name).toBe("A");
		expect(state.bio).toBe("");
		expect(state.tags).toEqual([]);
		expect(state.active).toBe(true);
	});

	test("account decoder rejects collection lengths beyond capacity", () => {
		const invalidName = new Uint8Array(231);
		invalidName[0] = 1;
		invalidName[2] = 33;

		const invalidTags = new Uint8Array(231);
		invalidTags[0] = 1;
		invalidTags[164] = 9;

		expect(() => getProfileStateDecoder().decode(invalidName)).toThrow();
		expect(() => getProfileStateDecoder().decode(invalidTags)).toThrow();
	});

	test("encoders reject collections that exceed their capacity", () => {
		expect(() =>
			getInitializeInstructionDataEncoder().encode({
				bump: 7,
				name: "x".repeat(33),
				bio: "",
			})
		).toThrow(/capacity/);

		expect(() =>
			getProfileStateEncoder().encode({
				bump: 7,
				name: "",
				bio: "",
				tags: Array.from({ length: 9 }, (_, index) => BigInt(index)),
				active: true,
			})
		).toThrow(/capacity/);
	});

	test("decoders reject the wrong discriminator and non-canonical booleans", () => {
		const wrongInstruction = new Uint8Array(164);
		wrongInstruction[0] = 1;
		expect(() => getInitializeInstructionDataDecoder().decode(wrongInstruction))
			.toThrow(/invalid discriminator/);

		const account = emptyProfileAccount();
		account[230] = 2;
		expect(() => getProfileStateDecoder().decode(account)).toThrow(
			/invalid zeropod boolean/,
		);
	});

	test("string decoding is strict UTF-8 and preserves embedded NULs", () => {
		const account = emptyProfileAccount();
		account.set([3, 65, 0, 66], 2);
		expect(getProfileStateDecoder().decode(account).name).toBe("A\0B");

		account.set([1, 0xff, 0, 0], 2);
		expect(() => getProfileStateDecoder().decode(account)).toThrow();
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
	const account = new Uint8Array(231);
	account[0] = 1;
	return account;
}
