import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
	existsSync,
	mkdirSync,
	mkdtempSync,
	readFileSync,
	writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

import {
	packageDirectoryName,
	parseArguments,
	platforms,
	populatePlatformPackage,
} from "../package-cli.mjs";

test("release matrix contains one package for each unique target", () => {
	assert.equal(platforms.length, 9);
	assert.equal(
		new Set(platforms.map((specification) => specification.target)).size,
		platforms.length,
	);
	assert.equal(
		new Set(platforms.map((specification) => specification.packageName)).size,
		platforms.length,
	);
});

test("argument parser rejects another flag as a value", () => {
	assert.deepEqual(
		parseArguments(["--release-tag", "v1.2.3", "--assets-dir", "--unexpected"]),
		{ "release-tag": "v1.2.3" },
	);
});

test("scoped package names map to repository package directories", () => {
	assert.equal(packageDirectoryName("@pina-rs/cli"), "pina__cli");
	assert.equal(
		packageDirectoryName("@pina-rs/cli-linux-x64-gnu"),
		"pina__cli-linux-x64-gnu",
	);
});

test("platform package population extracts and installs a release binary", () => {
	const sandbox = mkdtempSync(join(tmpdir(), "pina-package-cli-"));
	const assetsDirectory = join(sandbox, "assets");
	const archiveSource = join(sandbox, "archive-source");
	const packagesDirectory = join(sandbox, "packages");
	const specification = platforms.find(
		(candidate) => candidate.packageName === "@pina-rs/cli-darwin-arm64",
	);
	assert.ok(specification);

	mkdirSync(assetsDirectory, { recursive: true });
	mkdirSync(join(archiveSource, "nested"), { recursive: true });
	writeFileSync(join(archiveSource, "nested", "pina"), "pina-test-binary");
	const archivePath = join(
		assetsDirectory,
		`pina-${specification.target}-v1.2.3.tar.gz`,
	);
	const tar = spawnSync("tar", [
		"-czf",
		archivePath,
		"-C",
		archiveSource,
		"nested",
	], {
		encoding: "utf8",
	});
	assert.equal(tar.status, 0, tar.stderr);

	const packageDirectory = join(
		packagesDirectory,
		packageDirectoryName(specification.packageName),
	);
	mkdirSync(packageDirectory, { recursive: true });
	writeFileSync(
		join(packageDirectory, "package.json"),
		JSON.stringify({ name: specification.packageName, version: "1.2.3" }),
	);

	populatePlatformPackage({
		assetsDirectory,
		packagesDirectory,
		releaseTag: "v1.2.3",
		specification,
	});

	const installedBinary = join(packageDirectory, "bin", "pina");
	assert.ok(existsSync(installedBinary));
	assert.equal(readFileSync(installedBinary, "utf8"), "pina-test-binary");
});
