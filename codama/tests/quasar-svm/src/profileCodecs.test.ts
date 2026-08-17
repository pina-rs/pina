import { describe, expect, test } from "vitest";

import {
	getAddTagInstructionDataEncoder,
	getInitializeInstructionDataEncoder,
	getProfileStateDecoder,
} from "../../../clients/js/profile_program/src";

describe("profile generated codecs", () => {
	test("instruction encoders prepend their discriminator", () => {
		const initialize = getInitializeInstructionDataEncoder().encode({
			bump: 42,
			name: new Uint8Array(33),
			bio: new Uint8Array(129),
		});
		const addTag = getAddTagInstructionDataEncoder().encode({ tag: 10 });

		expect(initialize).toHaveLength(164);
		expect(Array.from(initialize.slice(0, 2))).toEqual([0, 42]);
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
		expect(Array.from(state.name.slice(0, 2))).toEqual([1, 65]);
		expect(state.active).toBe(true);
	});
});
