import assert from "node:assert/strict";
import test from "node:test";

import {
	checkZstdCallers,
	checkZstdReviewDate,
} from "../check-zstd-advisories.ts";

const ZSTD_CALLERS = [
	"zstd v0.13.3",
	"agave-snapshots v4.2.2",
	"solana-account-decoder v4.2.2",
	"solana-account-decoder-client-types v4.2.2",
].join("\n");

test("accepts only the reviewed dependency callers", () => {
	checkZstdCallers("zstd", `${ZSTD_CALLERS}\n`);
	checkZstdCallers("zstd-safe", "zstd-safe v7.3.0\r\nzstd v0.13.3\r\n");
});

test("rejects a new direct caller and a changed caller version", () => {
	assert.throws(
		() => checkZstdCallers("zstd", `${ZSTD_CALLERS}\nnew-caller v1.0.0`),
		/callers changed/,
	);
	assert.throws(
		() =>
			checkZstdCallers(
				"zstd",
				ZSTD_CALLERS.replace(
					"agave-snapshots v4.2.2",
					"agave-snapshots v4.2.3",
				),
			),
		/callers changed/,
	);
});

test("rejects a direct zstd-safe dependency that could enable seekable", () => {
	assert.throws(
		() =>
			checkZstdCallers(
				"zstd-safe",
				"zstd-safe v7.3.0\nzstd v0.13.3\nnew-caller v1.0.0",
			),
		/callers changed/,
	);
});

test("rejects a source substitution and a missing dependency", () => {
	assert.throws(
		() =>
			checkZstdCallers(
				"zstd",
				ZSTD_CALLERS.replace("zstd v0.13.3", "zstd v0.13.3 (local-source)"),
			),
		/callers changed/,
	);
	assert.throws(() => checkZstdCallers("zstd", ""), /callers changed/);
});

test("expires at the review deadline and rejects an invalid clock value", () => {
	checkZstdReviewDate(new Date("2026-11-24T23:59:59.999Z"));
	assert.throws(
		() => checkZstdReviewDate(new Date("2026-11-25T00:00:00Z")),
		/expired/,
	);
	assert.throws(() => checkZstdReviewDate(new Date("invalid")), /expired/);
});
