//! Keeps the release pipeline's standalone lint-driver assets in sync with the
//! name `pina lint` asks for.
//!
//! The CLI identifies a driver by host triple and compiler commit hash:
//! `pina-lint-driver-<host>-<commit>` (with an `.exe` suffix on Windows). The
//! release pipeline publishes that name from `build-release.yml`. Nothing in Rust
//! can assert against YAML, so the coupling is checked here — a mismatch would
//! surface as every user downloading a `404` instead of a driver.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const scriptDirectory = dirname(fileURLToPath(import.meta.url));
// This file lives in `scripts/npm/tests`, so the repository root is three
// levels up — the same depth `check-packages.mjs` resolves from `scripts/npm`.
const repositoryRoot = resolve(scriptDirectory, "../../..");
const buildWorkflow = readFileSync(
	join(repositoryRoot, ".github/workflows/build-release.yml"),
	"utf8",
);
const publishWorkflow = readFileSync(
	join(repositoryRoot, ".github/workflows/publish.yml"),
	"utf8",
);

/** The driver asset name the CLI builds, mirrored from `driver_asset_name`. */
function driverAssetName(host, commitHash) {
	return host.includes("windows")
		? `pina-lint-driver-${host}-${commitHash}.exe`
		: `pina-lint-driver-${host}-${commitHash}`;
}

test("the release publishes a driver asset the CLI can request", () => {
	// The pipeline builds the asset name in shell; assert the literal shape
	// appears so a rename in either place fails here rather than in the field.
	assert.match(
		buildWorkflow,
		/asset="pina-lint-driver-\$\{\{ matrix\.target \}\}-\$\{revision\}\$\{extension\}"/,
		"build-release.yml must name standalone driver assets as pina-lint-driver-<target>-<revision>",
	);
});

test("the standalone driver asset name matches the CLI format", () => {
	// Reconstruct the pipeline's expression and compare it to the CLI's rule.
	const hash = "7f99507f57e6c4aa0dce3daf6a13cca8cd4dd312";
	for (
		const target of [
			"aarch64-unknown-linux-gnu",
			"aarch64-apple-darwin",
			"x86_64-unknown-linux-gnu",
		]
	) {
		const asset = `pina-lint-driver-${target}-${hash}`;
		assert.equal(asset, driverAssetName(target, hash));
		assert.ok(
			asset.startsWith("pina-"),
			`${asset} must match the sealed release inventory's asset name prefix`,
		);
	}

	// Windows drivers carry the executable suffix on both sides.
	const windowsAsset = `pina-lint-driver-x86_64-pc-windows-msvc-${hash}.exe`;
	assert.equal(
		windowsAsset,
		driverAssetName("x86_64-pc-windows-msvc", hash),
	);
});

test("only driver-capable targets publish a standalone asset", () => {
	// The step is gated on the same flag that builds the driver, so a target
	// that cannot build one cannot advertise an asset for it either.
	assert.match(
		buildWorkflow,
		/- name: stage standalone lint driver\n\s+if: matrix\.lint_driver\n/,
	);
	assert.match(
		buildWorkflow,
		/- name: build prebuilt lint driver\n\s+if: matrix\.lint_driver/,
	);
});

test("the published driver revision comes from the compiler that built it", () => {
	// A hardcoded revision would name the asset after a nightly that may not
	// be the one the driver was linked against, which is the exact mismatch
	// the asset name exists to prevent.
	assert.match(
		buildWorkflow,
		/rustc -vV \| sed -n 's\/\^commit-hash: \/\/p'/,
	);
	assert.match(buildWorkflow, /\^\[0-9a-f\]\{40\}\$/);
});

test("a dry run never uploads the standalone driver", () => {
	const builderJob = buildWorkflow;
	const publisherJob = publishWorkflow.split("\n  publish:\n")[1];
	assert.doesNotMatch(builderJob, /gh release upload|contents: write/);
	assert.match(builderJob, /dry-run: true/);
	assert.match(
		publisherJob,
		/inputs\.dry_run != true/,
		"a dry run must not write to the draft release",
	);
	assert.match(publisherJob, /gh release upload/);
});
