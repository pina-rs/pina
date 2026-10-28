#!/usr/bin/env node

import { execFileSync } from "node:child_process";

// These immutable crate versions were reviewed against RUSTSEC-2026-0391,
// RUSTSEC-2026-0398, and RUSTSEC-2026-0403. See deny.toml for the call sites.
const REVIEWED_TREES = {
	zstd: [
		"zstd v0.13.3",
		"agave-snapshots v4.2.2",
		"solana-account-decoder v4.2.2",
		"solana-account-decoder-client-types v4.2.2",
	],
	"zstd-safe": ["zstd-safe v7.3.0", "zstd v0.13.3"],
} as const;

export function checkZstdReviewDate(now: Date): void {
	if (!(now.getTime() < Date.parse("2026-11-25T00:00:00Z"))) {
		throw new Error(
			"The temporary zstd advisory review has expired; upgrade the Solana train or review its callers again",
		);
	}
}

export function checkZstdCallers(
	crate: keyof typeof REVIEWED_TREES,
	tree: string,
): void {
	const actual = tree.trim().split(/\r?\n/).sort();
	const expected = [...REVIEWED_TREES[crate]].sort();

	if (
		actual.length !== expected.length ||
		actual.some((line, index) => line !== expected[index])
	) {
		throw new Error(
			`The ${crate} dependency callers changed; review the temporary advisory exceptions in deny.toml`,
		);
	}
}

if (import.meta.main) {
	checkZstdReviewDate(new Date());

	for (const crate of ["zstd", "zstd-safe"] as const) {
		const tree = execFileSync("cargo", [
			"tree",
			"--locked",
			"--workspace",
			"--all-features",
			"--target",
			"all",
			"--invert",
			crate,
			"--depth",
			"1",
			"--prefix",
			"none",
			"--format",
			"{p}",
			"--edges",
			"normal,build,dev",
			"--no-dedupe",
		], { cwd: new URL("..", import.meta.url), encoding: "utf8" });

		// An unversioned inversion also fails if a second crate version appears.
		checkZstdCallers(crate, tree);
	}
}
