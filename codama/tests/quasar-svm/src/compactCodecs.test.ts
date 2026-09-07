import { address } from "@solana/kit";
import { describe, expect, test } from "vitest";

import {
	getJournalDecoder,
	getJournalEncoder,
} from "../../../clients/js/compact_accounts/src/generated";

const SYSTEM_ADDRESS = address("11111111111111111111111111111111");

describe("compact generated codecs", () => {
	test("matches the compact multi-tail PinaPod wire layout", () => {
		const encoded = getJournalEncoder().encode({
			bump: 7,
			authority: SYSTEM_ADDRESS,
			revision: 4,
			featuredEntry: null,
			title: "",
			entries: [5n, 8n],
			markers: [21, 34],
			note: "ok",
		});

		expect(encoded).toHaveLength(80);
		expect(Array.from(encoded.slice(48, 50))).toEqual([2, 0]);
		expect(Array.from(encoded.slice(50, 58))).toEqual([2, 0, 0, 0, 0, 0, 0, 0]);
		expect(encoded[58]).toBe(1);
		expect(Array.from(encoded.slice(59, 67))).toEqual([5, 0, 0, 0, 0, 0, 0, 0]);
		expect(Array.from(encoded.slice(75, 80))).toEqual([21, 34, 2, 111, 107]);

		const decoded = getJournalDecoder().decode(encoded);
		expect(decoded.entries).toEqual([5n, 8n]);
		expect(decoded.markers).toEqual([21, 34]);
		expect(decoded.note).toEqual({ __option: "Some", value: "ok" });
	});

	test("rejects every compact capacity before encoding", () => {
		const base = {
			bump: 7,
			authority: SYSTEM_ADDRESS,
			revision: 4,
			featuredEntry: null,
			title: "",
			entries: [] as bigint[],
			markers: [] as number[],
			note: null,
		};

		expect(() => getJournalEncoder().encode({ ...base, title: "x".repeat(25) }))
			.toThrow(/capacity/);
		expect(() =>
			getJournalEncoder().encode({ ...base, entries: Array(9).fill(0n) })
		).toThrow(/capacity/);
		expect(() =>
			getJournalEncoder().encode({ ...base, markers: Array(9).fill(0) })
		).toThrow(/capacity/);
		expect(() => getJournalEncoder().encode({ ...base, note: "😀".repeat(17) }))
			.toThrow(/capacity/);
		expect(() => getJournalEncoder().encode({ ...base, note: "\ud800" }))
			.toThrow(/Unicode scalar values/);
	});

	test("rejects malformed counts, tags, UTF-8, and discriminators", () => {
		const empty = getJournalEncoder().encode({
			bump: 7,
			authority: SYSTEM_ADDRESS,
			revision: 4,
			featuredEntry: null,
			title: "",
			entries: [],
			markers: [],
			note: null,
		});
		const excessiveEntries = Uint8Array.from(empty);
		excessiveEntries[48] = 9;
		const excessiveMarkers = Uint8Array.from(empty);
		excessiveMarkers[50] = 9;
		const invalidOption = Uint8Array.from(empty);
		invalidOption[58] = 2;
		const invalidDiscriminator = Uint8Array.from(empty);
		invalidDiscriminator[0] = 2;
		const invalidUtf8 = Uint8Array.from(
			getJournalEncoder().encode({
				bump: 7,
				authority: SYSTEM_ADDRESS,
				revision: 4,
				featuredEntry: null,
				title: "",
				entries: [],
				markers: [],
				note: "x",
			}),
		);
		invalidUtf8[60] = 0xff;
		expect(() => getJournalDecoder().decode(excessiveEntries)).toThrow(
			/capacity/,
		);
		expect(() => getJournalDecoder().decode(excessiveMarkers)).toThrow(
			/capacity/,
		);
		expect(() => getJournalDecoder().decode(invalidOption)).toThrow(
			/option tag/,
		);
		expect(() => getJournalDecoder().decode(invalidDiscriminator)).toThrow(
			/invalid discriminator/,
		);
		expect(() => getJournalDecoder().decode(invalidUtf8)).toThrow();
	});
});
