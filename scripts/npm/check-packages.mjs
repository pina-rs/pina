#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { packageDirectoryName, platforms } from "./package-cli.mjs";

const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const repositoryRoot = resolve(scriptDirectory, "../..");
const packagesDirectory = join(repositoryRoot, "packages");
const require = createRequire(import.meta.url);
const launcher = require(join(packagesDirectory, "pina__cli/bin/pina.cjs"));

function readManifest(directoryName) {
	return JSON.parse(
		readFileSync(
			join(packagesDirectory, directoryName, "package.json"),
			"utf8",
		),
	);
}

function assertPublicPackage(manifest) {
	assert.equal(
		manifest.license,
		"Apache-2.0",
		`${manifest.name} must use the workspace license`,
	);
	assert.equal(
		manifest.publishConfig?.access,
		"public",
		`${manifest.name} must publish publicly`,
	);
	assert.equal(
		manifest.publishConfig?.provenance,
		true,
		`${manifest.name} must publish with provenance`,
	);
}

const cliManifest = readManifest("pina__cli");
assert.equal(cliManifest.name, "@pina-rs/cli");
assertPublicPackage(cliManifest);

const platformNames = new Set(
	platforms.map((specification) => specification.packageName),
);
assert.deepEqual(
	new Set(Object.keys(cliManifest.optionalDependencies)),
	platformNames,
);

const publishWorkflow = readFileSync(
	join(repositoryRoot, ".github/workflows/publish.yml"),
	"utf8",
);
const releaseTargets = new Set(
	[...publishWorkflow.matchAll(/^\s+- target: (\S+)$/gm)].map(
		(match) => match[1],
	),
);
assert.deepEqual(
	releaseTargets,
	new Set(platforms.map((specification) => specification.target)),
	"npm platform packages must match the native release matrix",
);

for (const specification of platforms) {
	const directoryName = packageDirectoryName(specification.packageName);
	const manifest = readManifest(directoryName);
	assert.equal(manifest.name, specification.packageName);
	assert.equal(manifest.version, cliManifest.version);
	assert.deepEqual(manifest.os, [specification.os]);
	assert.deepEqual(manifest.cpu, [specification.cpu]);
	assert.deepEqual(
		manifest.libc,
		specification.libc === undefined ? undefined : [specification.libc],
	);
	assert.equal(
		cliManifest.optionalDependencies[manifest.name],
		`^${cliManifest.version}`,
	);
	assert.equal(manifest.repository?.directory, `packages/${directoryName}`);
	assertPublicPackage(manifest);

	const candidates = launcher.getCandidatePackages(
		specification.os,
		specification.cpu,
	);
	assert.ok(
		candidates.includes(specification.packageName),
		`${manifest.name} is missing from the launcher`,
	);
}

const codamaManifest = readManifest("nodes-from-pina");
assert.equal(codamaManifest.name, "@pina-rs/codama-nodes");
assert.equal(codamaManifest.version, cliManifest.version);
assertPublicPackage(codamaManifest);

const skillManifest = readManifest("pina__skill");
assert.equal(skillManifest.name, "@pina-rs/skill");
assert.equal(skillManifest.version, cliManifest.version);
assertPublicPackage(skillManifest);

console.log(
	`Verified ${
		platforms.length + 3
	} @pina-rs npm packages at ${cliManifest.version}`,
);
