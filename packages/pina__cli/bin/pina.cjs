#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const PLATFORM_PACKAGES = Object.freeze({
	darwin: Object.freeze({
		arm64: Object.freeze(["@pina-rs/cli-darwin-arm64"]),
		x64: Object.freeze(["@pina-rs/cli-darwin-x64"]),
	}),
	freebsd: Object.freeze({
		x64: Object.freeze(["@pina-rs/cli-freebsd-x64"]),
	}),
	linux: Object.freeze({
		arm64: Object.freeze([
			"@pina-rs/cli-linux-arm64-gnu",
			"@pina-rs/cli-linux-arm64-musl",
		]),
		x64: Object.freeze([
			"@pina-rs/cli-linux-x64-gnu",
			"@pina-rs/cli-linux-x64-musl",
		]),
	}),
	win32: Object.freeze({
		arm64: Object.freeze(["@pina-rs/cli-win32-arm64-msvc"]),
		x64: Object.freeze(["@pina-rs/cli-win32-x64-msvc"]),
	}),
});

function getCandidatePackages(
	platform = process.platform,
	arch = process.arch,
) {
	return PLATFORM_PACKAGES[platform]?.[arch] ?? [];
}

function resolveBinary(packageName, resolvePackage = require.resolve) {
	try {
		const packageJsonPath = resolvePackage(`${packageName}/package.json`);
		const packageDirectory = path.dirname(packageJsonPath);
		const binaryName = process.platform === "win32" ? "pina.exe" : "pina";
		const binaryPath = path.join(packageDirectory, "bin", binaryName);

		return fs.existsSync(binaryPath) ? binaryPath : null;
	} catch {
		return null;
	}
}

function shouldTryNextPackage(result) {
	return result.error !== undefined || result.status === 126 ||
		result.status === 127;
}

function main(args = process.argv.slice(2)) {
	const candidates = getCandidatePackages();
	if (candidates.length === 0) {
		console.error(
			`@pina-rs/cli does not publish a binary for ${process.platform}/${process.arch}. ` +
				"Install from a GitHub release or run `cargo install pina_cli`.",
		);
		return 1;
	}

	const failures = [];
	for (const packageName of candidates) {
		const binaryPath = resolveBinary(packageName);
		if (binaryPath === null) {
			continue;
		}

		const result = spawnSync(binaryPath, args, {
			stdio: "inherit",
			windowsHide: false,
		});

		if (shouldTryNextPackage(result)) {
			const detail = result.error?.message ??
				`exit code ${result.status ?? "unknown"}`;
			failures.push(`${packageName}: ${detail}`);
			continue;
		}

		return result.status ?? 1;
	}

	console.error(
		"Unable to find or start a compatible Pina binary from the installed packages.",
	);
	console.error(`Tried: ${candidates.join(", ")}`);
	if (failures.length > 0) {
		console.error(failures.join("\n"));
	}
	console.error(
		"Reinstall `@pina-rs/cli`, use a GitHub release, or run `cargo install pina_cli`.",
	);

	return 1;
}

module.exports = {
	PLATFORM_PACKAGES,
	getCandidatePackages,
	main,
	resolveBinary,
	shouldTryNextPackage,
};

if (require.main === module) {
	process.exitCode = main();
}
