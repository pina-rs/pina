import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { describe, expect, test } from "vitest";

import {
	getProfileStateDecoder,
	getProfileStateEncoder,
} from "../../../codama/clients/js/profile_program/src/generated/accounts/profileState";
import {
	getInitializeInstructionDataDecoder,
	getInitializeInstructionDataEncoder,
} from "../../../codama/clients/js/profile_program/src/generated/instructions/initialize";

interface WireFixture {
	readonly encodedHex: string;
	readonly invalidDiscriminatorOffset: number;
	readonly size: number;
}

interface ProfileStateFixture extends WireFixture {
	readonly invalidBooleanOffset: number;
	readonly invalidOptionOffset: number;
	readonly nameOffset: number;
	readonly tagsOffset: number;
}

interface ContractFixture {
	readonly schemaVersion: number;
	readonly profileState: ProfileStateFixture;
	readonly initializeInstruction: WireFixture;
}

const fixturePath = fileURLToPath(
	new URL("../../../codama/contracts/profile_program.json", import.meta.url),
);
const fixture = JSON.parse(
	readFileSync(fixturePath, "utf8"),
) as ContractFixture;

describe("Dart and JavaScript PinaPod contract parity", () => {
	test("profile state matches the shared 240-byte golden", () => {
		const encoded = getProfileStateEncoder().encode({
			active: true,
			bio: "bio",
			bump: 254,
			favoriteTag: 42n,
			name: "A\0B",
			tags: [7n, 9n],
		});

		expect(encoded.length).toBe(fixture.profileState.size);
		expect(Buffer.from(encoded).toString("hex")).toBe(
			fixture.profileState.encodedHex,
		);
	});

	test("initialize data matches the shared 164-byte golden", () => {
		const encoded = getInitializeInstructionDataEncoder().encode({
			bio: "bio",
			bump: 9,
			name: "name",
		});

		expect(encoded.length).toBe(fixture.initializeInstruction.size);
		expect(Buffer.from(encoded).toString("hex")).toBe(
			fixture.initializeInstruction.encodedHex,
		);
	});

	test("rejects the same malformed discriminator, option, and boolean tags", () => {
		const accountDecoder = getProfileStateDecoder();
		const accountGolden = Uint8Array.from(
			Buffer.from(fixture.profileState.encodedHex, "hex"),
		);
		const badAccountOffsets = [
			fixture.profileState.invalidDiscriminatorOffset,
			fixture.profileState.invalidOptionOffset,
			fixture.profileState.invalidBooleanOffset,
		];

		for (const offset of badAccountOffsets) {
			const malformed = Uint8Array.from(accountGolden);
			malformed[offset] = 2;
			expect(() => accountDecoder.decode(malformed)).toThrow();
		}

		const instructionDecoder = getInitializeInstructionDataDecoder();
		const malformedInstruction = Uint8Array.from(
			Buffer.from(fixture.initializeInstruction.encodedHex, "hex"),
		);
		malformedInstruction[
			fixture.initializeInstruction.invalidDiscriminatorOffset
		] = 1;
		expect(() => instructionDecoder.decode(malformedInstruction)).toThrow();
	});

	test("rejects malformed bounded string and vector contents", () => {
		const decoder = getProfileStateDecoder();
		const golden = Uint8Array.from(
			Buffer.from(fixture.profileState.encodedHex, "hex"),
		);
		const invalidUtf8 = Uint8Array.from(golden);
		invalidUtf8[fixture.profileState.nameOffset] = 2;
		invalidUtf8[fixture.profileState.nameOffset + 1] = 0xc3;
		invalidUtf8[fixture.profileState.nameOffset + 2] = 0x28;
		expect(() => decoder.decode(invalidUtf8)).toThrow();

		const oversizedVector = Uint8Array.from(golden);
		oversizedVector[fixture.profileState.tagsOffset] = 9;
		oversizedVector[fixture.profileState.tagsOffset + 1] = 0;
		expect(() => decoder.decode(oversizedVector)).toThrow();
	});

	test("treats inactive option capacity as unobservable", () => {
		const decoder = getProfileStateDecoder();
		const encoded = Uint8Array.from(
			getProfileStateEncoder().encode({
				active: false,
				bio: "bio",
				bump: 1,
				favoriteTag: null,
				name: "name",
				tags: [],
			}),
		);

		for (
			let index = fixture.profileState.invalidOptionOffset + 1;
			index < 239;
			index += 1
		) {
			encoded[index] = 0xa5;
		}

		expect(decoder.decode(encoded).favoriteTag).toEqual({ __option: "None" });
	});

	test("rejects fixed-capacity overflow instead of truncating", () => {
		expect(() =>
			getProfileStateEncoder().encode({
				active: false,
				bio: "bio",
				bump: 1,
				favoriteTag: null,
				name: "x".repeat(33),
				tags: [],
			})
		).toThrow();

		expect(() =>
			getProfileStateEncoder().encode({
				active: false,
				bio: "bio",
				bump: 1,
				favoriteTag: null,
				name: "name",
				tags: Array.from({ length: 9 }, (_, index) => BigInt(index)),
			})
		).toThrow();
	});
});
