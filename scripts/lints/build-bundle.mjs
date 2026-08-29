#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
	cpSync,
	mkdirSync,
	mkdtempSync,
	readFileSync,
	realpathSync,
	rmSync,
	statSync,
	writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const repositoryRoot = resolve(
	dirname(fileURLToPath(import.meta.url)),
	"../..",
);
const catalogPath = join(repositoryRoot, "crates/pina_cli/lints.json");

function fail(message) {
	throw new Error(message);
}

function parseArguments(arguments_) {
	const options = {};
	for (let index = 0; index < arguments_.length; index += 2) {
		const name = arguments_[index];
		const value = arguments_[index + 1];
		if (!name?.startsWith("--") || value === undefined) {
			fail("Expected --target, --release-tag, and --output-dir arguments.");
		}
		options[name.slice(2)] = value;
	}
	if (!options.target || !options["release-tag"] || !options["output-dir"]) {
		fail("Expected --target, --release-tag, and --output-dir arguments.");
	}
	return {
		outputDirectory: resolve(options["output-dir"]),
		releaseTag: options["release-tag"],
		target: options.target,
	};
}

function run(command, arguments_, options = {}) {
	return execFileSync(command, arguments_, {
		cwd: repositoryRoot,
		encoding: "utf8",
		stdio: options.capture ? ["ignore", "pipe", "inherit"] : "inherit",
		...options,
	});
}

function sha256(path) {
	return createHash("sha256").update(readFileSync(path)).digest("hex");
}

export function libraryFilename(name, toolchain, target) {
	if (target.includes("windows")) {
		return `${name}@${toolchain}.dll`;
	}
	if (target.includes("apple-darwin")) {
		return `lib${name}@${toolchain}.dylib`;
	}
	return `lib${name}@${toolchain}.so`;
}

function validateRelease(releaseTag) {
	if (!/^v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(releaseTag)) {
		fail(`Invalid release tag: ${releaseTag}`);
	}
	const version = releaseTag.slice(1);
	const metadata = JSON.parse(
		run("cargo", ["metadata", "--no-deps", "--format-version", "1"], {
			capture: true,
		}),
	);
	const cli = metadata.packages.find((item) => item.name === "pina_cli");
	if (!cli || cli.version !== version) {
		fail(
			`Release ${releaseTag} does not match pina_cli version ${
				cli?.version ?? "missing"
			}.`,
		);
	}
	return version;
}

function validateHost(target, catalog) {
	const rustc = run("rustc", ["-vV"], { capture: true });
	const host = /^host: (.+)$/m.exec(rustc)?.[1];
	if (host !== target) {
		fail(
			`Lint bundles must be built natively: rustc host is ${host}, requested ${target}.`,
		);
	}
	const activeToolchain = run("rustup", ["show", "active-toolchain"], {
		capture: true,
	})
		.trim()
		.split(/\s+/u)[0];
	const expectedToolchain = `${catalog.toolchain}-${target}`;
	if (activeToolchain !== expectedToolchain) {
		fail(
			`Active toolchain is ${activeToolchain}; expected ${expectedToolchain}.`,
		);
	}
	return expectedToolchain;
}

export function buildBundle(arguments_) {
	const { outputDirectory, releaseTag, target } = parseArguments(arguments_);
	const catalog = JSON.parse(readFileSync(catalogPath, "utf8"));
	if (catalog.schemaVersion !== 1 || catalog.libraries.length === 0) {
		fail("The lint catalog is empty or uses an unsupported schema.");
	}
	if (new Set(catalog.libraries).size !== catalog.libraries.length) {
		fail("The lint catalog contains duplicate library names.");
	}
	if (!catalog.targets.includes(target)) {
		fail(`Target ${target} is not in the lint release catalog.`);
	}
	const version = validateRelease(releaseTag);
	const toolchain = validateHost(target, catalog);
	const temporaryDirectory = mkdtempSync(join(tmpdir(), "pina-lint-bundle-"));
	const targetDirectory = join(temporaryDirectory, "target");
	const stagingDirectory = join(temporaryDirectory, "bundle");
	const linkerVariable = `CARGO_TARGET_${
		target.replaceAll("-", "_").toUpperCase()
	}_LINKER`;
	mkdirSync(stagingDirectory);
	mkdirSync(outputDirectory, { recursive: true });

	try {
		const libraries = [];
		for (const name of catalog.libraries) {
			const manifestPath = join(repositoryRoot, "lints", name, "Cargo.toml");
			run("cargo", ["build", "--release", "--manifest-path", manifestPath], {
				env: {
					...process.env,
					CARGO_INCREMENTAL: "0",
					CARGO_TARGET_DIR: targetDirectory,
					[linkerVariable]: "dylint-link",
				},
			});
			const file = libraryFilename(name, toolchain, target);
			const source = realpathSync(join(targetDirectory, "release", file));
			const destination = join(stagingDirectory, file);
			cpSync(source, destination);
			libraries.push({
				name,
				file,
				sha256: sha256(destination),
				size: statSync(destination).size,
			});
		}

		writeFileSync(
			join(stagingDirectory, "manifest.json"),
			`${
				JSON.stringify(
					{
						schema_version: 1,
						version,
						target,
						toolchain,
						dylint_version: catalog.dylintVersion,
						libraries,
					},
					null,
					"\t",
				)
			}\n`,
		);

		const archiveName = `pina-lints-${target}-${releaseTag}.tar.gz`;
		const archivePath = join(outputDirectory, archiveName);
		run("tar", [
			"-czf",
			archivePath,
			"-C",
			stagingDirectory,
			"manifest.json",
			...libraries.map((library) => library.file),
		]);
		console.log(
			JSON.stringify({
				archive: archivePath,
				name: basename(archivePath),
				sha256: sha256(archivePath),
				size: statSync(archivePath).size,
			}),
		);
		return archivePath;
	} finally {
		rmSync(temporaryDirectory, { force: true, recursive: true });
	}
}

if (
	process.argv[1] &&
	pathToFileURL(resolve(process.argv[1])).href === import.meta.url
) {
	buildBundle(process.argv.slice(2));
}
