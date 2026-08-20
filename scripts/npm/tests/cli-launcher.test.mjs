import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
	chmodSync,
	cpSync,
	mkdirSync,
	mkdtempSync,
	writeFileSync,
} from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const testDirectory = dirname(fileURLToPath(import.meta.url));
const repositoryRoot = resolve(testDirectory, "../../..");
const launcherPath = join(repositoryRoot, "packages/pina__cli/bin/pina.cjs");
const require = createRequire(import.meta.url);
const launcher = require(launcherPath);

test("launcher maps every published operating-system and CPU pair", () => {
	assert.deepEqual(launcher.getCandidatePackages("darwin", "arm64"), [
		"@pina-rs/cli-darwin-arm64",
	]);
	assert.deepEqual(launcher.getCandidatePackages("darwin", "x64"), [
		"@pina-rs/cli-darwin-x64",
	]);
	assert.deepEqual(launcher.getCandidatePackages("freebsd", "x64"), [
		"@pina-rs/cli-freebsd-x64",
	]);
	assert.deepEqual(launcher.getCandidatePackages("linux", "arm64"), [
		"@pina-rs/cli-linux-arm64-gnu",
		"@pina-rs/cli-linux-arm64-musl",
	]);
	assert.deepEqual(launcher.getCandidatePackages("linux", "x64"), [
		"@pina-rs/cli-linux-x64-gnu",
		"@pina-rs/cli-linux-x64-musl",
	]);
	assert.deepEqual(launcher.getCandidatePackages("win32", "arm64"), [
		"@pina-rs/cli-win32-arm64-msvc",
	]);
	assert.deepEqual(launcher.getCandidatePackages("win32", "x64"), [
		"@pina-rs/cli-win32-x64-msvc",
	]);
	assert.deepEqual(launcher.getCandidatePackages("aix", "ppc64"), []);
});

test("launcher resolves a native binary beside a platform manifest", () => {
	const sandbox = mkdtempSync(join(tmpdir(), "pina-launcher-"));
	const packageDirectory = join(sandbox, "node_modules/@pina-rs/cli-test");
	const binaryDirectory = join(packageDirectory, "bin");
	mkdirSync(binaryDirectory, { recursive: true });
	writeFileSync(
		join(packageDirectory, "package.json"),
		JSON.stringify({ name: "@pina-rs/cli-test" }),
	);
	const binaryName = process.platform === "win32" ? "pina.exe" : "pina";
	writeFileSync(join(binaryDirectory, binaryName), "test");
	if (process.platform !== "win32") {
		chmodSync(join(binaryDirectory, binaryName), 0o755);
	}

	const resolvePackage = (specifier) => {
		assert.equal(specifier, "@pina-rs/cli-test/package.json");
		return join(packageDirectory, "package.json");
	};
	assert.equal(
		launcher.resolveBinary("@pina-rs/cli-test", resolvePackage),
		join(binaryDirectory, binaryName),
	);
});

test("published launcher remains directly executable by Node", () => {
	const sandbox = mkdtempSync(join(tmpdir(), "pina-launcher-copy-"));
	const copiedLauncher = join(sandbox, "pina.cjs");
	cpSync(launcherPath, copiedLauncher);

	const copied = require(copiedLauncher);
	assert.equal(typeof copied.main, "function");
});

test(
	"published launcher executes the installed native package",
	{ skip: process.platform === "win32" },
	() => {
		const candidates = launcher.getCandidatePackages();
		assert.ok(candidates.length > 0);

		const sandbox = mkdtempSync(join(tmpdir(), "pina-launcher-run-"));
		const packageDirectory = join(
			sandbox,
			"node_modules",
			...candidates[0].split("/"),
		);
		const binaryDirectory = join(packageDirectory, "bin");
		mkdirSync(binaryDirectory, { recursive: true });
		writeFileSync(
			join(packageDirectory, "package.json"),
			JSON.stringify({ name: candidates[0], version: "1.0.0" }),
		);
		writeFileSync(
			join(binaryDirectory, "pina"),
			'#!/bin/sh\necho "pina-launcher-ok $*"\n',
			{
				mode: 0o755,
			},
		);

		const copiedLauncher = join(sandbox, "pina.cjs");
		cpSync(launcherPath, copiedLauncher);
		const result = spawnSync(process.execPath, [copiedLauncher, "--help"], {
			encoding: "utf8",
		});

		assert.equal(result.status, 0);
		assert.match(result.stdout, /pina-launcher-ok --help/);
	},
);
