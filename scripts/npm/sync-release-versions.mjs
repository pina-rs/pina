#!/usr/bin/env node

import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { packageDirectoryName, platforms } from "./package-cli.mjs";

const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const repositoryRoot = resolve(scriptDirectory, "../..");

function readManifest(path) {
	return JSON.parse(readFileSync(path, "utf8"));
}

export function synchronizePlatformDependencies(
	cliManifest,
	platformManifests,
) {
	const optionalDependencies = cliManifest.optionalDependencies;
	if (
		optionalDependencies === undefined ||
		optionalDependencies === null ||
		Array.isArray(optionalDependencies) ||
		typeof optionalDependencies !== "object"
	) {
		throw new Error("@pina-rs/cli must define optionalDependencies");
	}

	const expectedNames = new Set(
		platforms.map((specification) => specification.packageName),
	);
	const actualNames = new Set(Object.keys(optionalDependencies));
	if (
		expectedNames.size !== actualNames.size ||
		[...expectedNames].some((name) => !actualNames.has(name))
	) {
		throw new Error(
			"@pina-rs/cli optionalDependencies must match the release matrix",
		);
	}

	const versions = new Map(
		platformManifests.map((manifest) => [manifest.name, manifest.version]),
	);
	for (const specification of platforms) {
		const version = versions.get(specification.packageName);
		if (version === undefined) {
			throw new Error(`Missing manifest for ${specification.packageName}`);
		}

		if (version !== cliManifest.version) {
			throw new Error(
				`${specification.packageName} is ${version}, expected ${cliManifest.version}`,
			);
		}

		optionalDependencies[specification.packageName] = `^${version}`;
	}

	return cliManifest;
}

export function main() {
	const packagesDirectory = join(repositoryRoot, "packages");
	const cliManifestPath = join(
		packagesDirectory,
		packageDirectoryName("@pina-rs/cli"),
		"package.json",
	);
	const cliManifest = readManifest(cliManifestPath);
	const platformManifests = platforms.map((specification) =>
		readManifest(
			join(
				packagesDirectory,
				packageDirectoryName(specification.packageName),
				"package.json",
			),
		)
	);

	synchronizePlatformDependencies(cliManifest, platformManifests);
	writeFileSync(
		cliManifestPath,
		`${JSON.stringify(cliManifest, null, "\t")}\n`,
	);
}

if (
	process.argv[1] &&
	resolve(process.argv[1]) === resolve(fileURLToPath(import.meta.url))
) {
	main();
}
