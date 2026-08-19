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

describe("Dart and JavaScript zeropod contract parity", () => {
	test("profile state matches the shared 240-byte golden", () => {
		const encoded = getProfileStateEncoder().encode({
			active: true,
			bio: boundedText("bio", 129),
			bump: 254,
			favoriteTag: 42n,
			name: boundedText("A\0B", 33),
			tags: tagBytes([7n, 9n]),
		});

		expect(encoded.length).toBe(fixture.profileState.size);
		expect(Buffer.from(encoded).toString("hex")).toBe(
			fixture.profileState.encodedHex,
		);
	});

	test("initialize data matches the shared 164-byte golden", () => {
		const encoded = getInitializeInstructionDataEncoder().encode({
			bio: boundedText("bio", 129),
			bump: 9,
			name: boundedText("name", 33),
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

	test("treats bounded storage contents as opaque fixed bytes", () => {
		const decoder = getProfileStateDecoder();
		const golden = Uint8Array.from(
			Buffer.from(fixture.profileState.encodedHex, "hex"),
		);
		const opaque = Uint8Array.from(golden);
		opaque[fixture.profileState.nameOffset] = 33;
		opaque[fixture.profileState.nameOffset + 1] = 0xc3;
		opaque[fixture.profileState.nameOffset + 2] = 0x28;
		opaque[fixture.profileState.tagsOffset] = 9;

		const decoded = decoder.decode(opaque);
		expect(decoded.name.slice(0, 3)).toEqual(
			Uint8Array.from([33, 0xc3, 0x28]),
		);
		expect(decoded.tags[0]).toBe(9);
	});

	test("treats inactive option capacity as unobservable", () => {
		const decoder = getProfileStateDecoder();
		const encoded = Uint8Array.from(
			getProfileStateEncoder().encode({
				active: false,
				bio: boundedText("bio", 129),
				bump: 1,
				favoriteTag: null,
				name: boundedText("name", 33),
				tags: tagBytes([]),
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
				bio: boundedText("bio", 129),
				bump: 1,
				favoriteTag: null,
				name: new Uint8Array(34),
				tags: tagBytes([]),
			})
		).toThrow();

		expect(() =>
			getProfileStateEncoder().encode({
				active: false,
				bio: boundedText("bio", 129),
				bump: 1,
				favoriteTag: null,
				name: boundedText("name", 33),
				tags: new Uint8Array(67),
			})
		).toThrow();
	});
});

function boundedText(value: string, size: number): Uint8Array {
	const payload = new TextEncoder().encode(value);
	if (payload.length >= size || payload.length > 0xff) {
		throw new RangeError("value does not fit bounded storage");
	}

	const bytes = new Uint8Array(size);
	bytes[0] = payload.length;
	bytes.set(payload, 1);
	return bytes;
}

function tagBytes(values: readonly bigint[]): Uint8Array {
	const capacity = 8;
	if (values.length > capacity) {
		throw new RangeError("values do not fit bounded storage");
	}

	const bytes = new Uint8Array(2 + capacity * 8);
	const view = new DataView(bytes.buffer);
	view.setUint16(0, values.length, true);
	values.forEach((value, index) => {
		view.setBigUint64(2 + index * 8, BigInt.asUintN(64, value), true);
	});
	return bytes;
}
