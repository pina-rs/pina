import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { libraryFilename } from "./build-bundle.mjs";
import { executableFilename } from "./build-dylint-tools.mjs";

const repositoryRoot = resolve(
	dirname(fileURLToPath(import.meta.url)),
	"../..",
);

test("Dylint bundle filenames match each native loader convention", () => {
	assert.equal(
		libraryFilename(
			"secure_lint",
			"nightly-test-x86_64-unknown-linux-gnu",
			"x86_64-unknown-linux-gnu",
		),
		"libsecure_lint@nightly-test-x86_64-unknown-linux-gnu.so",
	);
	assert.equal(
		libraryFilename(
			"secure_lint",
			"nightly-test-aarch64-apple-darwin",
			"aarch64-apple-darwin",
		),
		"libsecure_lint@nightly-test-aarch64-apple-darwin.dylib",
	);
	assert.equal(
		libraryFilename(
			"secure_lint",
			"nightly-test-x86_64-pc-windows-msvc",
			"x86_64-pc-windows-msvc",
		),
		"secure_lint@nightly-test-x86_64-pc-windows-msvc.dll",
	);
});

test("Dylint tool filenames match Unix and Windows conventions", () => {
	assert.equal(
		executableFilename("cargo-dylint", "aarch64-apple-darwin"),
		"cargo-dylint",
	);
	assert.equal(
		executableFilename("cargo-dylint", "x86_64-pc-windows-msvc"),
		"cargo-dylint.exe",
	);
});

test("the release catalog covers every lint crate and matches the pinned toolchain", () => {
	const catalog = JSON.parse(
		readFileSync(join(repositoryRoot, "crates/pina_cli/lints.json"), "utf8"),
	);
	const lintCrates = readdirSync(join(repositoryRoot, "lints"), {
		withFileTypes: true,
	})
		.filter((entry) => entry.isDirectory())
		.filter((entry) => {
			try {
				readFileSync(join(repositoryRoot, "lints", entry.name, "Cargo.toml"));
				return true;
			} catch {
				return false;
			}
		})
		.map((entry) => entry.name)
		.sort();
	assert.deepEqual(catalog.libraries, lintCrates);
	assert.deepEqual(catalog.targets, [
		"aarch64-unknown-linux-gnu",
		"aarch64-unknown-linux-musl",
		"aarch64-apple-darwin",
		"aarch64-pc-windows-msvc",
		"x86_64-unknown-linux-gnu",
		"x86_64-unknown-linux-musl",
		"x86_64-apple-darwin",
		"x86_64-pc-windows-msvc",
		"x86_64-unknown-freebsd",
	]);

	const rustToolchain = readFileSync(
		join(repositoryRoot, "rust-toolchain.toml"),
		"utf8",
	);
	assert.match(
		rustToolchain,
		new RegExp(`channel = "${catalog.toolchain}"`, "u"),
	);
	const rootManifest = readFileSync(join(repositoryRoot, "Cargo.toml"), "utf8");
	assert.match(
		rootManifest,
		new RegExp(`dylint-link = \\{ version = "${catalog.dylintVersion}"`, "u"),
	);
	const publishWorkflow = readFileSync(
		join(repositoryRoot, ".github/workflows/publish.yml"),
		"utf8",
	);
	for (const target of catalog.targets) {
		assert.match(publishWorkflow, new RegExp(`target: ${target}`, "u"));
	}
	assert.match(
		publishWorkflow,
		/toolchain: \$\{\{ needs\.prepare_dylint_tools\.outputs\.toolchain \}\}/u,
	);
	assert.doesNotMatch(publishWorkflow, /toolchain: nightly-/u);
});
