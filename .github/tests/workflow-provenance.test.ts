import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
	existsSync,
	mkdtempSync,
	readFileSync,
	rmSync,
	writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

const MAIN = "a".repeat(40);
const RELEASE = "b".repeat(40);
const UNMERGED = "c".repeat(40);

/** Run the actual checkout-free workflow scripts with only GitHub's API replaced. */
function runStep(
	workflow: string,
	step: string,
	env: NodeJS.ProcessEnv = {},
) {
	const source = readFileSync(
		new URL(`../workflows/${workflow}.yml`, import.meta.url),
		"utf8",
	);
	const section = source.split(`- name: ${step}\n`)[1];
	assert.ok(section, `Missing workflow step: ${step}`);
	const script = section.match(/run: \|\n((?: {10}.*\n|\n)+)/)?.[1];
	assert.ok(script, `Missing shell script: ${step}`);
	const root = mkdtempSync(join(tmpdir(), "pina-workflow-provenance-"));
	const api = `#!${process.execPath}
import { appendFileSync } from "node:fs";
const endpoint = process.argv[3];
appendFileSync(process.env.API_LOG, endpoint + "\\n");
if (process.env.API_FAILURE === "true") process.exit(1);
if (endpoint.endsWith("/branches/main")) {
  if (process.env.UNPROTECTED !== "true") console.log(process.env.MAIN_SHA);
} else if (endpoint.includes("/compare/")) {
  const [base, head] = endpoint.split("/compare/")[1].split("...");
  if (head !== process.env.MAIN_SHA) throw new Error("Comparison must pin main");
  console.log(base === head ? "identical" : base === "${RELEASE}" ? "ahead" : process.env.UNMERGED_STATUS || "behind");
} else if (endpoint.includes("/commits/")) {
  const ref = endpoint.split("/commits/")[1];
  if (ref === "refs/tags/v1.0.0" && process.env.MISSING_TAG === "true") process.exit(1);
  console.log(JSON.stringify({ sha: ref === "refs/tags/v1.0.0" ? process.env.TAG_SHA : process.env.REQUESTED_SHA }));
} else {
  throw new Error("Unexpected API request: " + endpoint);
}
`;

	try {
		writeFileSync(join(root, "gh"), api, { mode: 0o755 });
		writeFileSync(join(root, "output"), "");
		writeFileSync(join(root, "calls"), "");
		const result = spawnSync("bash", [
			"-euo",
			"pipefail",
			"-c",
			script.replace(/^ {10}/gm, ""),
		], {
			encoding: "utf8",
			env: {
				...process.env,
				PATH: `${root}:${process.env.PATH}`,
				GITHUB_REPOSITORY: "pina-rs/pina",
				GITHUB_SERVER_URL: "https://github.com",
				GITHUB_RUN_ID: "123",
				GITHUB_RUN_ATTEMPT: "2",
				GITHUB_EVENT_NAME: "workflow_dispatch",
				GITHUB_REF: "refs/heads/main",
				GITHUB_SHA: MAIN,
				WORKFLOW_SHA: MAIN,
				WORKFLOW_REF:
					"pina-rs/pina/.github/workflows/release-pr.yml@refs/heads/main",
				MAIN_SHA: MAIN,
				APPROVED_MAIN_SHA: MAIN,
				RELEASE_TAG: "v1.0.0",
				DRY_RUN: "false",
				TAG_SHA: RELEASE,
				REQUESTED_SHA: RELEASE,
				REQUESTED_REF: RELEASE,
				...env,
				RUNNER_TEMP: root,
				GITHUB_OUTPUT: join(root, "output"),
				API_LOG: join(root, "calls"),
			},
		});

		return {
			...result,
			output: readFileSync(join(root, "output"), "utf8"),
			calls: readFileSync(join(root, "calls"), "utf8"),
			predicate: existsSync(join(root, "release-provenance.json"))
				? readFileSync(join(root, "release-provenance.json"), "utf8")
				: undefined,
		};
	} finally {
		rmSync(root, { recursive: true, force: true });
	}
}

/** Authorize the workflow and event independently before any checkout. */
function authorize(env: NodeJS.ProcessEnv = {}) {
	return runStep(
		"authorize-main",
		"authorize workflow and event revisions",
		env,
	);
}

test("authorizes main and propagates its immutable SHA", () => {
	const result = authorize();
	assert.equal(result.status, 0, result.stderr);
	assert.equal(result.output, `sha=${MAIN}\nmain_sha=${MAIN}\n`);
});

test("rejects a manual branch or tag dispatch before making API requests", () => {
	for (const ref of ["refs/heads/unmerged", "refs/tags/v1.0.0"]) {
		const result = authorize({ GITHUB_REF: ref });
		assert.equal(result.status, 1, result.stderr);
		assert.equal(result.calls, "");
		assert.equal(result.output, "");
	}
});

test("accepts release tags already merged into main, even after main advances", () => {
	for (const event of ["release", "push"]) {
		const result = authorize({
			GITHUB_EVENT_NAME: event,
			GITHUB_REF: "refs/tags/v1.0.0",
			GITHUB_SHA: RELEASE,
			WORKFLOW_SHA: RELEASE,
			WORKFLOW_REF:
				"pina-rs/pina/.github/workflows/publish.yml@refs/tags/v1.0.0",
		});
		assert.equal(result.status, 0, result.stderr);
		assert.equal(result.output, `sha=${RELEASE}\nmain_sha=${MAIN}\n`);
	}
});

test("rejects unmerged and diverged workflow or checkout revisions", () => {
	for (const field of ["GITHUB_SHA", "WORKFLOW_SHA"]) {
		for (const status of ["behind", "diverged", "unknown"]) {
			const result = authorize({ [field]: UNMERGED, UNMERGED_STATUS: status });
			assert.equal(result.status, 1, result.stderr);
			assert.equal(result.output, "");
		}
	}
});

test("fails closed on API errors, unprotected main, invalid SHAs, or unrelated workflows", () => {
	for (
		const env of [
			{ API_FAILURE: "true" },
			{ UNPROTECTED: "true" },
			{ GITHUB_SHA: "main" },
			{ WORKFLOW_SHA: "bad\nsha" },
			{ MAIN_SHA: "" },
			{ GITHUB_REPOSITORY: "other/repository" },
			{
				WORKFLOW_REF:
					"other/repository/.github/workflows/publish.yml@refs/heads/main",
			},
			{ GITHUB_EVENT_NAME: "pull_request" },
		]
	) {
		const result = authorize(env);
		assert.equal(result.status, 1, result.stderr);
		assert.equal(result.output, "");
	}
});

test("publishing requires a tagged commit already merged into main", () => {
	for (const sha of [MAIN, RELEASE, UNMERGED]) {
		const result = runStep(
			"publish",
			"resolve tag and checkout ref to one commit",
			{
				TAG_SHA: sha,
				REQUESTED_SHA: sha,
			},
		);
		assert.equal(result.status, sha === UNMERGED ? 1 : 0, result.stderr);
		assert.equal(result.output, sha === UNMERGED ? "" : `sha=${sha}\n`);
	}
});

test("publishing rejects a mismatching tag and cannot bypass ancestry on API failure", () => {
	for (
		const env of [{ TAG_SHA: MAIN }, { API_FAILURE: "true" }, {
			MISSING_TAG: "true",
		}]
	) {
		const result = runStep(
			"publish",
			"resolve tag and checkout ref to one commit",
			env,
		);
		assert.equal(result.status, 1, result.stderr);
		assert.equal(result.output, "");
	}
});

test("pre-tag previews can resolve unmerged commits without authorizing a real publish", () => {
	const result = runStep(
		"publish",
		"resolve tag and checkout ref to one commit",
		{
			DRY_RUN: "true",
			MISSING_TAG: "true",
			REQUESTED_SHA: UNMERGED,
		},
	);
	assert.equal(result.status, 0, result.stderr);
	assert.equal(result.output, `sha=${UNMERGED}\n`);
	assert.doesNotMatch(result.calls, /\/compare\//);
});

test("provenance identifies the release source separately from the workflow commit", () => {
	for (const event of ["workflow_dispatch", "push"]) {
		const ref = event === "push" ? "refs/tags/v1.0.0" : "refs/heads/main";
		const workflowSha = event === "push" ? RELEASE : MAIN;
		const workflowRef = `pina-rs/pina/.github/workflows/publish.yml@${ref}`;
		const inputs = event === "push" ? null : {
			tag: "v1.0.0",
			checkout_ref: 'release/"quoted"',
		};
		const result = runStep("publish", "record release source provenance", {
			GITHUB_EVENT_NAME: event,
			GITHUB_REF: ref,
			WORKFLOW_REF: workflowRef,
			WORKFLOW_SHA: workflowSha,
			SOURCE_SHA: RELEASE,
			DISPATCH_INPUTS: JSON.stringify(inputs),
		});
		assert.equal(result.status, 0, result.stderr);
		assert.ok(result.predicate);
		const predicate: unknown = JSON.parse(result.predicate);
		assert.deepEqual(predicate, {
			buildDefinition: {
				buildType: "https://actions.github.io/buildtypes/workflow/v1",
				externalParameters: {
					workflow: {
						repository: "https://github.com/pina-rs/pina",
						ref,
						path: ".github/workflows/publish.yml",
					},
					...(inputs === null ? {} : { inputs }),
				},
				resolvedDependencies: [{
					name: "workflow",
					uri: `git+https://github.com/pina-rs/pina@${ref}`,
					digest: { gitCommit: workflowSha },
				}, {
					name: "release-source",
					uri: "git+https://github.com/pina-rs/pina@refs/tags/v1.0.0",
					digest: { gitCommit: RELEASE },
				}],
			},
			runDetails: {
				builder: { id: `https://github.com/${workflowRef}` },
				metadata: {
					invocationId:
						"https://github.com/pina-rs/pina/actions/runs/123/attempts/2",
				},
			},
		});
	}
});

test("builders have only read access and all entry points depend on authorization", () => {
	const publish = readFileSync(
		new URL("../workflows/publish.yml", import.meta.url),
		"utf8",
	);
	const preview = publish.split("  preview_assets:\n")[1]?.split(
		"  upload_assets:\n",
	)[0];
	assert.ok(preview);
	assert.match(preview, /cache-mode: read/);
	assert.match(preview, /permissions:\n {6}contents: read\n {4}uses:/);
	assert.match(
		preview,
		/sha: \$\{\{ needs\.resolve_revision\.outputs\.sha \}\}/,
	);
	const builder = readFileSync(
		new URL("../workflows/build-release.yml", import.meta.url),
		"utf8",
	);
	assert.match(
		builder,
		/^permissions:\n {2}contents: read\n/m,
		"The builder must cap the caller's token at read access",
	);
	assert.doesNotMatch(
		builder,
		/contents: write|id-token: write|gh release upload/,
	);
	assert.match(builder, /ref: \$\{\{ inputs\.sha \}\}/);
	assert.match(builder, /dry-run: true/);
	assert.match(
		publish,
		/predicate-path: \$\{\{ runner\.temp \}\}\/release-provenance\.json/,
	);

	for (const workflow of ["release-pr", "docs-pages"]) {
		const source = readFileSync(
			new URL(`../workflows/${workflow}.yml`, import.meta.url),
			"utf8",
		);
		assert.match(
			source,
			/authorize:\n {4}uses: \$\/\.github\/workflows\/authorize-main\.yml/,
		);
		const checkouts = source.matchAll(
			/uses: actions\/checkout@[^\n]+\n {8}with:\n([^]*?)(?=\n\n)/g,
		);
		let count = 0;

		for (const checkout of checkouts) {
			assert.match(
				checkout[1] ?? "",
				/ref: \$\{\{ needs\.authorize\.outputs\.sha \}\}/,
			);
			count += 1;
		}

		assert.equal(count, workflow === "release-pr" ? 2 : 1);
	}
});
