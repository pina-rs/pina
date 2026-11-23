import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import test from "node:test";

import {
	discoverChildPackageNames,
	discoverSecureFixturePackageNames,
	isPathInside,
	MUTANT_PACKAGES,
	parseAdvisoryIgnores,
	parseManifestPackageName,
	REPO_ROOT,
} from "../ci-tasks.ts";

test("parseManifestPackageName extracts package names from Cargo manifests", () => {
	assert.equal(
		parseManifestPackageName(
			'[package]\nname = "pina_cli"\nversion = "0.1.0"\n',
		),
		"pina_cli",
	);
	assert.equal(parseManifestPackageName("[workspace]\nmembers = []\n"), null);
});

test("parseAdvisoryIgnores parses single-line and multi-line deny.toml ignore lists", () => {
	const sample = `
[bans]
ignore = ["RUSTSEC-0000-0000"]

[advisories]
ignore = [
  "RUSTSEC-2024-0001", # reason
  "RUSTSEC-2025-0002",
]

[licenses]
ignore = ["RUSTSEC-9999-9999"]
`;
	assert.deepEqual(parseAdvisoryIgnores(sample), [
		"RUSTSEC-2024-0001",
		"RUSTSEC-2025-0002",
	]);

	const inlineSample = `[advisories]\nignore = ["RUSTSEC-2026-0042"]\n`;
	assert.deepEqual(parseAdvisoryIgnores(inlineSample), ["RUSTSEC-2026-0042"]);

	const commentBracketSample =
		`[advisories]\nignore = [ # see [notes]\n  "RUSTSEC-2026-0099",\n]\n`;
	assert.deepEqual(parseAdvisoryIgnores(commentBracketSample), [
		"RUSTSEC-2026-0099",
	]);

	// Verify the repository's real deny.toml parses cleanly.
	const realDeny = readFileSync(join(REPO_ROOT, "deny.toml"), "utf8");
	const realIgnores = parseAdvisoryIgnores(realDeny);
	for (const id of realIgnores) {
		assert.match(id, /^RUSTSEC-\d+-\d+$/u);
	}
});

test("workspace package discovery finds generated clients, examples, and secure fixtures", () => {
	const generatedClients = discoverChildPackageNames(
		join(REPO_ROOT, "codama/clients/rust"),
	);
	assert.ok(generatedClients.length > 0);
	assert.ok(generatedClients.includes("counter-program-client"));

	const examples = discoverChildPackageNames(join(REPO_ROOT, "examples"));
	assert.ok(examples.includes("counter_program"));

	const secureFixtures = discoverSecureFixturePackageNames(REPO_ROOT);
	assert.ok(secureFixtures.length > 0);
	assert.ok(secureFixtures.every((name) => name.includes("secure")));
});

test("isPathInside distinguishes workspace-internal and external directories", () => {
	assert.equal(isPathInside("/work/pina", "/work/pina"), true);
	assert.equal(isPathInside("/work/pina", "/work/pina/.cargo"), true);
	assert.equal(isPathInside("/work/pina", "/work/pina/..cache"), true);
	assert.equal(isPathInside("/work/pina", "/home/runner/.cargo"), false);
	assert.equal(isPathInside("/work/pina", "/work/pina-other"), false);
});

test("MUTANT_PACKAGES stays aligned with core crate inventory", () => {
	assert.deepEqual([...MUTANT_PACKAGES], [
		"pina",
		"pina_cli",
		"pina_codama_renderer",
		"pina_macros",
		"pina_profile",
	]);
});
