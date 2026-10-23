import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";

import {
	buildLanding,
	parseBenchmarks,
	parseCapabilities,
	renderBenchmarks,
	renderLanding,
	renderStats,
	workspaceVersion,
} from "../docs/build-landing.ts";

const root = resolve(import.meta.dirname, "../..");
const read = (path: string) => readFileSync(join(root, path), "utf8");

const comparison = `# Framework comparison

<!-- BEGIN GENERATED: framework-comparison -->

### Hello world

| Framework | Version | Size (bytes) | \`hello\` CU | vs Pinocchio size |
| --- | --- | ---: | ---: | ---: |
| Pina | 1.0.0 | 1,000 | 100 | −50% |
| Pinocchio (hand-written) | 0.11.2 | 2,000 | 90 | +0% |
| Anchor v1 | 1.2.1 | 50,000 | 400 | +2400% |

### Counter

| Framework | Version | Size (bytes) | \`initialize\` CU | \`increment\` CU | vs Pinocchio size |
| --- | --- | ---: | ---: | ---: | ---: |
| Pina | 1.0.0 | 5,000 | 1,500 | 300 | +0% |
| Pinocchio (hand-written) | 0.11.2 | 5,000 | 1,400 | 1,500 | +0% |
| Anchor v1 | 1.2.1 | 100,000 | 12,000 | 9,000 | +1900% |

<!-- END GENERATED: framework-comparison -->
`;

const readme = `### Capabilities

| Capability | Pina | Anchor v1 |
| --- | --- | --- |
| **Version compared** | v<!-- generated -->1.0.0<!-- end --> | 1.2.1 |
| \`no_std\` runtime | ✅ allocation-free | ❌ std + heap |
| Zero-copy | ✅ | 🟡 opt-in \`AccountLoader\` |
`;

test("benchmarks parse from the generated comparison tables", () => {
	const benchmarks = parseBenchmarks(comparison);
	assert.deepEqual(benchmarks.hello[0], {
		framework: "Pina",
		version: "1.0.0",
		bytes: 1000,
		cu: { hello: 100 },
	});
	assert.deepEqual(benchmarks.counter[2].cu, {
		initialize: 12000,
		increment: 9000,
	});
});

test("benchmarks without their generated markers are rejected", () => {
	assert.throws(
		() => parseBenchmarks("### Hello world\n"),
		/generated region markers/,
	);
});

test("benchmarks without a Pina row are rejected", () => {
	assert.throws(
		() => parseBenchmarks(comparison.replace(/^\| Pina \|.*\n/gm, "")),
		/no Pina measurement/,
	);
});

test("capabilities keep statuses, notes, and the compared versions", () => {
	const matrix = parseCapabilities(readme);
	assert.deepEqual(matrix.frameworks, ["Pina", "Anchor v1"]);
	assert.deepEqual(matrix.versions, ["v1.0.0", "1.2.1"]);
	assert.equal(matrix.rows[0].name, "<code>no_std</code> runtime");
	assert.deepEqual(matrix.rows[0].cells, [
		{ status: "yes", note: "allocation-free" },
		{ status: "no", note: "std + heap" },
	]);
	assert.deepEqual(matrix.rows[1].cells[1], {
		status: "partial",
		note: "opt-in <code>AccountLoader</code>",
	});
});

test("capability cells without a status mark are rejected", () => {
	assert.throws(
		() => parseCapabilities(readme.replace("❌ std + heap", "Planned")),
		/"`no_std` runtime" has a cell without a status mark: "Planned"/,
	);
});

test("the workspace version comes from [workspace.package]", () => {
	const manifest = `[package]
version = "0.0.0"

[workspace.package]
version = "1.2.3"
`;
	assert.equal(workspaceVersion(manifest), "1.2.3");
	assert.throws(() => workspaceVersion('[package]\nversion = "1"\n'));
});

test("stat tiles are computed from the measurements", () => {
	const stats = renderStats(parseBenchmarks(comparison));
	assert.match(stats, /1,000<span class="stat-unit">bytes/);
	assert.match(stats, /50% smaller than hand-written Pinocchio/);
	assert.match(stats, /50×/);
	assert.match(stats, /30× less compute than Anchor v1/);
});

test("an outlier bar is drawn past an axis break", () => {
	const html = renderBenchmarks(parseBenchmarks(comparison));
	assert.match(html, /class="bar is-broken"/);
	assert.match(html, /50× Pina/);
});

test("unknown template values and blocks fail the render", () => {
	const sources = { manifest: read("Cargo.toml"), comparison, readme };
	assert.throws(
		() => renderLanding("{{missing}}", sources),
		/unknown landing value/,
	);
	assert.throws(
		() => renderLanding("<!-- pina:missing -->", sources),
		/unknown landing block/,
	);
});

test("the committed landing page renders from the repository's sources", () => {
	const book = mkdtempSync(join(tmpdir(), "pina-landing-"));
	assert.throws(() => buildLanding(book), /does not look like a built book/);

	writeFileSync(join(book, "introduction.html"), "");
	mkdirSync(join(book, "landing"));
	buildLanding(book);

	const html = readFileSync(join(book, "index.html"), "utf8");
	assert.doesNotMatch(html, /\{\{|<!-- pina:/);
	assert.match(html, new RegExp(`v${workspaceVersion(read("Cargo.toml"))}`));
	assert.match(html, /class="matrix"/);
	assert.ok(
		readFileSync(join(book, "landing/fonts/inter-latin-wght-normal.woff2"))
			.length > 0,
	);
});
