import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
	existsSync,
	mkdirSync,
	mkdtempSync,
	readFileSync,
	rmSync,
	symlinkSync,
	writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { createManifest, sha256, verifyManifest } from "../release-assets.ts";

/** Exercise the real filesystem inventory and clean up even when verification fails. */
function fixture(t: test.TestContext) {
	const root = mkdtempSync(join(tmpdir(), "pina-release-assets-"));
	const assets = join(root, "sealed-release/assets");
	const manifest = join(root, "sealed-release/manifest.sha256");
	mkdirSync(assets, { recursive: true });
	writeFileSync(join(assets, "pina-linux-v1.tar.gz"), "built archive");
	writeFileSync(join(assets, "pina-driver.sha256"), "driver checksum");
	t.after(() => rmSync(root, { recursive: true, force: true }));

	return { root, assets, manifest };
}

test("seals and verifies the exact builder inventory deterministically", (t) => {
	const { assets, manifest } = fixture(t);
	const digest = createManifest(assets, manifest);
	assert.equal(digest, sha256(readFileSync(manifest)));
	assert.equal(
		readFileSync(manifest, "utf8").split("\n")[0].split("  ")[1],
		"pina-driver.sha256",
	);
	verifyManifest(assets, manifest, digest);
	assert.throws(() => createManifest(assets, manifest), /EEXIST/);
});

for (const mutation of ["replace", "missing", "extra", "manifest"]) {
	test(`rejects ${mutation} after attestation`, (t) => {
		const { assets, manifest } = fixture(t);
		const digest = createManifest(assets, manifest);

		switch (mutation) {
			case "replace":
				writeFileSync(
					join(assets, "pina-linux-v1.tar.gz"),
					"substituted archive",
				);
				break;
			case "missing":
				rmSync(join(assets, "pina-linux-v1.tar.gz"));
				break;
			case "extra":
				writeFileSync(join(assets, "pina-extra.zip"), "unattested");
				break;
			case "manifest":
				writeFileSync(manifest, "replacement inventory");
				break;
		}

		assert.throws(
			() => verifyManifest(assets, manifest, digest),
			/digest mismatch|sealed inventory/,
		);
	});
}

test("rejects an empty inventory, directories, symlinks, and unsafe names", (t) => {
	const { root, assets, manifest } = fixture(t);
	rmSync(assets, { recursive: true });
	mkdirSync(assets);
	assert.throws(() => createManifest(assets, manifest), /empty/);

	for (
		const name of [
			"pina-directory",
			"pina-link",
			"pina-bad\nname",
			"unexpected.zip",
		]
	) {
		const path = join(assets, name);

		if (name === "pina-directory") {
			mkdirSync(path);
		} else if (name === "pina-link") {
			symlinkSync(join(root, "outside"), path);
		} else {
			writeFileSync(path, "unexpected");
		}

		assert.throws(
			() => createManifest(assets, manifest),
			/Invalid release asset/,
		);
		rmSync(path, { recursive: true });
	}
});

test("publisher CLI stops on substitution or an untrusted manifest digest", (t) => {
	const { assets, manifest } = fixture(t);
	const script = new URL("../release-assets.ts", import.meta.url);
	const created = spawnSync(process.execPath, [
		script.pathname,
		"create",
		assets,
		manifest,
	], {
		encoding: "utf8",
	});
	assert.equal(created.status, 0, created.stderr);
	const digest = created.stdout.trim();

	for (const expected of [digest, "invalid", "0".repeat(64)]) {
		const verified = spawnSync(process.execPath, [
			script.pathname,
			"verify",
			assets,
			manifest,
			expected,
		]);
		assert.equal(verified.status, expected === digest ? 0 : 1);
	}

	writeFileSync(
		join(assets, "pina-linux-v1.tar.gz"),
		"substituted after attestation",
	);
	const replaced = spawnSync(process.execPath, [
		script.pathname,
		"verify",
		assets,
		manifest,
		digest,
	]);
	assert.equal(replaced.status, 1);
	assert.match(replaced.stderr.toString(), /sealed inventory/);
	const invalid = spawnSync(process.execPath, [script.pathname]);
	assert.equal(invalid.status, 1);
	assert.match(invalid.stderr.toString(), /Usage:/);
});

/** Run the workflow's actual shell gates, replacing only the external GitHub service. */
function publisherFixture(t: test.TestContext) {
	const setup = fixture(t);
	const bin = join(setup.root, "bin");
	const calls = join(setup.root, "github-calls");
	const digest = createManifest(setup.assets, setup.manifest);
	mkdirSync(bin);
	writeFileSync(
		join(bin, "gh"),
		`#!/usr/bin/env node
import { appendFileSync, cpSync, mkdirSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
const args = process.argv.slice(2);
const root = process.env.RUNNER_TEMP;
const failure = process.env.GH_FAILURE;
appendFileSync(process.env.GH_CALLS, args.join("\\t") + "\\n");
if (args[0] === "attestation") process.exit(failure === "attestation" ? 1 : 0);
if (args[1] === "view") process.exit(failure === "existing" ? 1 : 0);
if (args[1] === "upload") {
  mkdirSync(join(root, "draft"));
  for (const name of readdirSync(join(root, "sealed-release/assets"))) {
    cpSync(join(root, "sealed-release/assets", name), join(root, "draft", name));
  }
  cpSync(join(root, "sealed-release/manifest.sha256"), join(root, "draft/manifest.sha256"));
}
if (args[1] === "download") {
  const directory = args[args.indexOf("--dir") + 1];
  for (const name of readdirSync(join(root, "draft"))) {
    cpSync(join(root, "draft", name), join(directory, name));
  }
  if (failure === "replace") writeFileSync(join(directory, "pina-linux-v1.tar.gz"), "substituted");
  if (failure === "extra") writeFileSync(join(directory, "pina-extra.zip"), "unexpected");
  if (failure === "missing") rmSync(join(directory, "pina-linux-v1.tar.gz"));
  if (failure === "manifest") writeFileSync(join(directory, "manifest.sha256"), "substituted");
}
`,
		{ mode: 0o755 },
	);

	return {
		...setup,
		run(name: string, failure = "") {
			return spawnSync("bash", ["-c", workflowStep(name)], {
				cwd: fileURLToPath(new URL("../..", import.meta.url)),
				encoding: "utf8",
				env: {
					...process.env,
					PATH: `${bin}:${process.env.PATH}`,
					RUNNER_TEMP: setup.root,
					MANIFEST_SHA256: digest,
					GITHUB_REPOSITORY: "example/project",
					GITHUB_SHA: "a".repeat(40),
					RELEASE_TAG: "v1",
					GH_CALLS: calls,
					GH_FAILURE: failure,
				},
			});
		},
		calls() {
			return existsSync(calls) ? readFileSync(calls, "utf8") : "";
		},
	};
}

/** Extract a named multiline run block without maintaining a second copy of the gate. */
function workflowStep(name: string): string {
	const workflow = readFileSync(
		new URL("../../.github/workflows/publish.yml", import.meta.url),
		"utf8",
	);
	const step = workflow.split(`      - name: ${name}\n`)[1]?.split(
		"\n      - name:",
	)[0];
	assert.ok(step, `Missing workflow step: ${name}`);
	const body = step.split("        run: |\n")[1]?.split(
		/\n        [a-z-]+:/,
	)[0];
	assert.ok(body, `Missing shell script: ${name}`);

	return body.split("\n").map((line) => line.slice(10)).join("\n");
}

test("publisher verifies every asset and the manifest with the expected signer", (t) => {
	const fixture = publisherFixture(t);
	const result = fixture.run("verify sealed release before packaging");
	assert.equal(result.status, 0, result.stderr);
	assert.equal(
		fixture.calls().split("\n").filter((line) =>
			line.startsWith("attestation\tverify")
		).length,
		3,
	);
	assert.match(
		fixture.calls(),
		/--signer-workflow\texample\/project\/\.github\/workflows\/publish.yml/,
	);
	assert.match(
		fixture.calls(),
		/--signer-digest\ta{40}\t--deny-self-hosted-runners/,
	);
});

test("publisher refuses substituted local bytes before calling GitHub", (t) => {
	const fixture = publisherFixture(t);
	writeFileSync(join(fixture.assets, "pina-linux-v1.tar.gz"), "substituted");
	const result = fixture.run("verify sealed release before packaging");
	assert.equal(result.status, 1);
	assert.equal(fixture.calls(), "");
});

for (const failure of ["attestation", "existing"]) {
	test(`publisher fails closed on ${failure} before packaging`, (t) => {
		const fixture = publisherFixture(t);
		const result = fixture.run(
			"verify sealed release before packaging",
			failure,
		);
		assert.equal(result.status, 1);
		assert.doesNotMatch(fixture.calls(), /release\t(upload|edit)/);
	});
}

for (
	const failure of ["", "replace", "missing", "extra", "manifest", "existing"]
) {
	test(`release promotion ${failure ? `rejects ${failure}` : "accepts the sealed inventory"}`, (t) => {
		const fixture = publisherFixture(t);
		const result = fixture.run("publish draft release", failure);
		assert.equal(result.status, failure ? 1 : 0, result.stderr);
		assert.equal(fixture.calls().includes("release\tedit"), failure === "");
		assert.doesNotMatch(fixture.calls(), /--clobber/);
	});
}
