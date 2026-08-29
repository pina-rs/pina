#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
	chmodSync,
	cpSync,
	mkdirSync,
	mkdtempSync,
	readFileSync,
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
const toolNames = ["cargo-dylint", "dylint-link"];

function fail(message) {
	throw new Error(message);
}

function parseArguments(arguments_) {
	const options = {};
	for (let index = 0; index < arguments_.length; index += 2) {
		const name = arguments_[index];
		const value = arguments_[index + 1];
		if (!name?.startsWith("--") || value === undefined) {
			fail("Expected --target and --output-dir arguments.");
		}
		options[name.slice(2)] = value;
	}
	if (!options.target || !options["output-dir"]) {
		fail("Expected --target and --output-dir arguments.");
	}
	return {
		outputDirectory: resolve(options["output-dir"]),
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

export function executableFilename(name, target) {
	return target.includes("windows") ? `${name}.exe` : name;
}

function validateHost(target) {
	const rustc = run("rustc", ["-vV"], { capture: true });
	const host = /^host: (.+)$/m.exec(rustc)?.[1];
	if (host !== target) {
		fail(
			`Dylint tools must be built natively: rustc host is ${host}, requested ${target}.`,
		);
	}
}

export function buildDylintTools(arguments_) {
	const { outputDirectory, target } = parseArguments(arguments_);
	const catalog = JSON.parse(readFileSync(catalogPath, "utf8"));
	const version = catalog.dylintVersion;
	if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(version)) {
		fail(`Invalid Dylint version in lint catalog: ${version}`);
	}
	if (!catalog.targets.includes(target)) {
		fail(`Target ${target} is not in the lint release catalog.`);
	}
	validateHost(target);

	const temporaryDirectory = mkdtempSync(join(tmpdir(), "pina-dylint-tools-"));
	const installDirectory = join(temporaryDirectory, "install");
	const stagingDirectory = join(temporaryDirectory, "bundle");
	mkdirSync(stagingDirectory);
	mkdirSync(outputDirectory, { recursive: true });

	try {
		for (const name of toolNames) {
			run("cargo", [
				"install",
				"--locked",
				"--force",
				"--root",
				installDirectory,
				"--version",
				`=${version}`,
				name,
			], {
				env: {
					...process.env,
					CARGO_INCREMENTAL: "0",
					CARGO_TARGET_DIR: join(temporaryDirectory, "target", name),
				},
			});
		}

		const executables = toolNames.map((name) => {
			const file = executableFilename(name, target);
			const source = join(installDirectory, "bin", file);
			const destination = join(stagingDirectory, file);
			cpSync(source, destination);
			if (!target.includes("windows")) {
				chmodSync(destination, 0o755);
			}
			return {
				name,
				file,
				sha256: sha256(destination),
				size: statSync(destination).size,
			};
		});

		writeFileSync(
			join(stagingDirectory, "manifest.json"),
			`${
				JSON.stringify(
					{
						schema_version: 1,
						dylint_version: version,
						target,
						executables,
					},
					null,
					"\t",
				)
			}\n`,
		);

		const archiveName = `pina-dylint-tools-${target}-v${version}.tar.gz`;
		const archivePath = join(outputDirectory, archiveName);
		run("tar", [
			"-czf",
			archivePath,
			"-C",
			stagingDirectory,
			"manifest.json",
			...executables.map((executable) => executable.file),
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
	buildDylintTools(process.argv.slice(2));
}
