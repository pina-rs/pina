#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import {
	chmodSync,
	copyFileSync,
	existsSync,
	mkdirSync,
	mkdtempSync,
	readdirSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const repositoryRoot = resolve(scriptDirectory, "../..");

export const platforms = Object.freeze([
	{
		archiveExtension: "tar.gz",
		binaryName: "pina",
		cpu: "arm64",
		label: "Linux arm64 (glibc)",
		libc: "glibc",
		os: "linux",
		packageName: "@pina-rs/cli-linux-arm64-gnu",
		target: "aarch64-unknown-linux-gnu",
	},
	{
		archiveExtension: "tar.gz",
		binaryName: "pina",
		cpu: "arm64",
		label: "Linux arm64 (musl)",
		libc: "musl",
		os: "linux",
		packageName: "@pina-rs/cli-linux-arm64-musl",
		target: "aarch64-unknown-linux-musl",
	},
	{
		archiveExtension: "tar.gz",
		binaryName: "pina",
		cpu: "arm64",
		label: "macOS arm64",
		os: "darwin",
		packageName: "@pina-rs/cli-darwin-arm64",
		target: "aarch64-apple-darwin",
	},
	{
		archiveExtension: "zip",
		binaryName: "pina.exe",
		cpu: "arm64",
		label: "Windows arm64",
		os: "win32",
		packageName: "@pina-rs/cli-win32-arm64-msvc",
		target: "aarch64-pc-windows-msvc",
	},
	{
		archiveExtension: "tar.gz",
		binaryName: "pina",
		cpu: "x64",
		label: "Linux x64 (glibc)",
		libc: "glibc",
		os: "linux",
		packageName: "@pina-rs/cli-linux-x64-gnu",
		target: "x86_64-unknown-linux-gnu",
	},
	{
		archiveExtension: "tar.gz",
		binaryName: "pina",
		cpu: "x64",
		label: "Linux x64 (musl)",
		libc: "musl",
		os: "linux",
		packageName: "@pina-rs/cli-linux-x64-musl",
		target: "x86_64-unknown-linux-musl",
	},
	{
		archiveExtension: "tar.gz",
		binaryName: "pina",
		cpu: "x64",
		label: "macOS x64",
		os: "darwin",
		packageName: "@pina-rs/cli-darwin-x64",
		target: "x86_64-apple-darwin",
	},
	{
		archiveExtension: "zip",
		binaryName: "pina.exe",
		cpu: "x64",
		label: "Windows x64",
		os: "win32",
		packageName: "@pina-rs/cli-win32-x64-msvc",
		target: "x86_64-pc-windows-msvc",
	},
	{
		archiveExtension: "tar.gz",
		binaryName: "pina",
		cpu: "x64",
		label: "FreeBSD x64",
		os: "freebsd",
		packageName: "@pina-rs/cli-freebsd-x64",
		target: "x86_64-unknown-freebsd",
	},
]);

export function parseArguments(arguments_) {
	const parsed = {};

	for (let index = 0; index < arguments_.length; index += 1) {
		const key = arguments_[index];
		const value = arguments_[index + 1];
		if (
			!key.startsWith("--") || value === undefined || value.startsWith("--")
		) {
			continue;
		}

		parsed[key.slice(2)] = value;
		index += 1;
	}

	return parsed;
}

export function packageDirectoryName(packageName) {
	const pinaScope = "@pina-rs/";
	if (packageName.startsWith(pinaScope)) {
		return `pina__${packageName.slice(pinaScope.length)}`;
	}

	return packageName.replace(/^@/, "").replace("/", "__");
}

function run(command, arguments_, options = {}) {
	const result = spawnSync(command, arguments_, {
		cwd: options.cwd,
		encoding: "utf8",
		stdio: options.stdio ?? "pipe",
	});

	if (result.status !== 0) {
		const detail = result.stderr || result.stdout ||
			`exit code ${result.status ?? "unknown"}`;
		throw new Error(`${command} ${arguments_.join(" ")} failed: ${detail}`);
	}
}

function findArchive(assetsDirectory, specification, releaseTag) {
	const archiveName =
		`pina-${specification.target}-${releaseTag}.${specification.archiveExtension}`;
	const archivePath = join(assetsDirectory, archiveName);
	if (!existsSync(archivePath)) {
		throw new Error(`Missing release asset: ${archiveName}`);
	}

	return archivePath;
}

function extractArchive(archivePath, destinationDirectory) {
	mkdirSync(destinationDirectory, { recursive: true });
	if (archivePath.endsWith(".zip")) {
		run("unzip", ["-q", archivePath, "-d", destinationDirectory]);
		return;
	}

	if (archivePath.endsWith(".tar.gz")) {
		run("tar", ["-xzf", archivePath, "-C", destinationDirectory]);
		return;
	}

	throw new Error(`Unsupported release archive: ${basename(archivePath)}`);
}

function findBinary(directory, binaryName) {
	for (const entry of readdirSync(directory, { withFileTypes: true })) {
		const entryPath = join(directory, entry.name);
		if (entry.isDirectory()) {
			const nested = findBinary(entryPath, binaryName);
			if (nested !== null) {
				return nested;
			}
		} else if (entry.name === binaryName) {
			return entryPath;
		}
	}

	return null;
}

export function populatePlatformPackage(
	{ assetsDirectory, packagesDirectory, releaseTag, specification },
) {
	const archivePath = findArchive(assetsDirectory, specification, releaseTag);
	const extractionDirectory = mkdtempSync(
		join(tmpdir(), `pina-${specification.target}-`),
	);
	extractArchive(archivePath, extractionDirectory);

	const binaryPath = findBinary(extractionDirectory, specification.binaryName);
	if (binaryPath === null) {
		throw new Error(
			`Release asset ${
				basename(archivePath)
			} does not contain ${specification.binaryName}`,
		);
	}

	const packageDirectory = join(
		packagesDirectory,
		packageDirectoryName(specification.packageName),
	);
	const binaryDirectory = join(packageDirectory, "bin");
	if (!existsSync(join(packageDirectory, "package.json"))) {
		throw new Error(
			`Missing package manifest for ${specification.packageName}`,
		);
	}

	mkdirSync(binaryDirectory, { recursive: true });
	const destination = join(binaryDirectory, specification.binaryName);
	copyFileSync(binaryPath, destination);
	if (specification.binaryName === "pina") {
		chmodSync(destination, 0o755);
	}
}

export function main(arguments_ = process.argv.slice(2)) {
	const parsed = parseArguments(arguments_);
	const releaseTag = parsed["release-tag"];
	const assetsDirectory = parsed["assets-dir"];
	if (releaseTag === undefined || assetsDirectory === undefined) {
		throw new Error(
			"Usage: package-cli.mjs --release-tag <vX.Y.Z> --assets-dir <directory>",
		);
	}

	const resolvedAssetsDirectory = resolve(assetsDirectory);
	const packagesDirectory = join(repositoryRoot, "packages");
	for (const specification of platforms) {
		populatePlatformPackage({
			assetsDirectory: resolvedAssetsDirectory,
			packagesDirectory,
			releaseTag,
			specification,
		});
		console.log(
			`Prepared ${specification.packageName} from ${specification.target}`,
		);
	}
}

if (
	process.argv[1] &&
	resolve(process.argv[1]) === resolve(fileURLToPath(import.meta.url))
) {
	main();
}
