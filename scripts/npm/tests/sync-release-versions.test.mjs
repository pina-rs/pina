import assert from "node:assert/strict";
import { test } from "node:test";

import { platforms } from "../package-cli.mjs";
import { synchronizePlatformDependencies } from "../sync-release-versions.mjs";

function releaseManifests(version) {
	return platforms.map((specification) => ({
		name: specification.packageName,
		version,
	}));
}

function cliManifest(version, dependencyVersion) {
	return {
		name: "@pina-rs/cli",
		optionalDependencies: Object.fromEntries(
			platforms.map((specification) => [
				specification.packageName,
				`^${dependencyVersion}`,
			]),
		),
		version,
	};
}

test("release versions synchronize every platform dependency", () => {
	const manifest = synchronizePlatformDependencies(
		cliManifest("0.10.0", "0.9.0"),
		releaseManifests("0.10.0"),
	);

	assert.deepEqual(
		new Set(Object.values(manifest.optionalDependencies)),
		new Set(["^0.10.0"]),
	);
});

test("release versions reject a platform version mismatch", () => {
	const manifests = releaseManifests("0.10.0");
	manifests[0].version = "0.9.0";

	assert.throws(
		() =>
			synchronizePlatformDependencies(
				cliManifest("0.10.0", "0.9.0"),
				manifests,
			),
		/@pina-rs\/cli-linux-arm64-gnu is 0\.9\.0, expected 0\.10\.0/,
	);
});

test("release versions reject an incomplete dependency matrix", () => {
	const manifest = cliManifest("0.10.0", "0.9.0");
	delete manifest.optionalDependencies[platforms[0].packageName];

	assert.throws(
		() => synchronizePlatformDependencies(manifest, releaseManifests("0.10.0")),
		/optionalDependencies must match the release matrix/,
	);
});
