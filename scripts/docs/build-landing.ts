#!/usr/bin/env node

// Renders the pina.rs landing page into a built mdBook.
//
// mdBook writes the book's first chapter to `index.html` as well as to its own
// page. This script replaces that copy with the landing page from
// `docs/landing/`, so the site root is the landing page and every chapter keeps
// its URL.
//
// The numbers on the page are never typed by hand. The workspace version comes
// from `Cargo.toml`, the benchmark chart and stat tiles from the generated
// tables in `docs/src/framework-comparison.md`, and the capability matrix from
// the readme. Regenerating those sources and rebuilding the docs refreshes the
// page, and a source that no longer parses fails the build instead of shipping
// stale or empty figures.
//
// Usage:
//
//   node scripts/docs/build-landing.ts <bookDir>

import {
	copyFileSync,
	existsSync,
	mkdirSync,
	readdirSync,
	readFileSync,
	writeFileSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../..");

export interface Measurement {
	framework: string;
	version: string;
	bytes: number;
	/** Compute units per instruction, keyed by instruction name. */
	cu: Record<string, number>;
}

export interface Capability {
	name: string;
	cells: CapabilityCell[];
}

export interface CapabilityCell {
	status: "yes" | "partial" | "no";
	note: string;
}

export interface CapabilityMatrix {
	frameworks: string[];
	versions: string[];
	rows: Capability[];
}

interface Metric {
	id: string;
	tab: string;
	title: string;
	description: string;
	program: string;
	unit: string;
	value: (entry: Measurement) => number;
}

const PINA = "Pina";
const PINOCCHIO = "Pinocchio (hand-written)";
const ANCHOR_V1 = "Anchor v1";

/**
 * A bar is drawn past an axis break when it is this many times the next
 * largest, so a single outlier cannot flatten every other bar to a sliver.
 */
const BREAK_RATIO = 3;

/** The readme's capability marks; a cell without one fails the build. */
const STATUS_MARKS: Record<string, CapabilityCell["status"]> = {
	"✅": "yes",
	"🟡": "partial",
	"❌": "no",
};

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/** Splits a markdown table row into trimmed cells. */
function cells(line: string): string[] {
	return line.trim().replace(/^\|/, "").replace(/\|$/, "").split("|").map((
		cell,
	) => cell.trim());
}

/**
 * Reads the first markdown table at or after `start`, skipping the
 * alignment row.
 */
function readTable(
	lines: string[],
	start: number,
): { header: string[]; rows: string[][] } {
	let index = start;
	while (index < lines.length && !lines[index].trim().startsWith("|")) {
		index += 1;
	}
	if (index >= lines.length) {
		throw new Error("expected a markdown table");
	}
	const header = cells(lines[index]);
	const rows: string[][] = [];
	for (index += 2; index < lines.length; index += 1) {
		const line = lines[index].trim();
		if (!line.startsWith("|")) {
			break;
		}
		rows.push(cells(line));
	}
	return { header, rows };
}

function parseNumber(value: string): number {
	const parsed = Number(value.replace(/[,\s]/g, ""));
	if (!Number.isFinite(parsed)) {
		throw new Error(`expected a number, found "${value}"`);
	}
	return parsed;
}

/**
 * Parses the generated region of `docs/src/framework-comparison.md` into
 * measurements keyed by program (`hello`, `counter`).
 */
export function parseBenchmarks(
	markdown: string,
): Record<string, Measurement[]> {
	const begin = markdown.indexOf(
		"<!-- BEGIN GENERATED: framework-comparison -->",
	);
	const end = markdown.indexOf("<!-- END GENERATED: framework-comparison -->");
	if (begin === -1 || end === -1 || end < begin) {
		throw new Error(
			"framework-comparison.md is missing its generated region markers",
		);
	}
	const lines = markdown.slice(begin, end).split("\n");
	const programs: Record<string, string> = {
		"Hello world": "hello",
		Counter: "counter",
	};
	const result: Record<string, Measurement[]> = {};

	lines.forEach((line, index) => {
		const heading = line.match(/^#{2,4}\s+(.+)$/);
		const program = heading ? programs[heading[1].trim()] : undefined;
		if (!program) {
			return;
		}
		const { header, rows } = readTable(lines, index + 1);
		const sizeColumn = header.findIndex((cell) => cell.startsWith("Size"));
		const cuColumns = header.flatMap((cell, column) => {
			const match = cell.match(/^`([^`]+)` CU$/);
			return match ? [{ name: match[1], column }] : [];
		});
		if (sizeColumn === -1 || cuColumns.length === 0) {
			throw new Error(`the ${heading?.[1]} table is missing its columns`);
		}
		result[program] = rows.map((row) => ({
			framework: row[0],
			version: row[1],
			bytes: parseNumber(row[sizeColumn]),
			cu: Object.fromEntries(
				cuColumns.map(({ name, column }) => [name, parseNumber(row[column])]),
			),
		}));
	});

	for (const program of Object.values(programs)) {
		const rows = result[program];
		if (!rows?.some((entry) => entry.framework === PINA)) {
			throw new Error(`no Pina measurement for the ${program} program`);
		}
	}
	return result;
}

/** Parses the capability table under `### Capabilities` in the readme. */
export function parseCapabilities(readme: string): CapabilityMatrix {
	const lines = readme.split("\n");
	const start = lines.findIndex((line) => line.trim() === "### Capabilities");
	if (start === -1) {
		throw new Error("readme.md has no `### Capabilities` section");
	}
	const { header, rows } = readTable(lines, start + 1);
	const frameworks = header.slice(1).map(inline);
	let versions: string[] = frameworks.map(() => "");
	const parsed: Capability[] = [];

	for (const row of rows) {
		const name = row[0].replace(/\*\*/g, "");
		if (name === "Version compared") {
			versions = row.slice(1).map((cell) => inline(stripComments(cell)));
			continue;
		}
		parsed.push({
			name: inline(name),
			cells: row.slice(1).map((cell) => {
				const text = stripComments(cell);
				const mark = /^(✅|🟡|❌)\s*/u.exec(text);
				if (!mark) {
					throw new Error(
						`capability "${name}" has a cell without a status mark: "${text}"`,
					);
				}
				return {
					status: STATUS_MARKS[mark[1]],
					note: inline(text.slice(mark[0].length)),
				};
			}),
		});
	}

	if (frameworks[0] !== PINA || parsed.length === 0) {
		throw new Error("the readme capability table has an unexpected shape");
	}
	return { frameworks, versions, rows: parsed };
}

/** Reads `[workspace.package] version` from the root manifest. */
export function workspaceVersion(manifest: string): string {
	const section = manifest.split(/^\[/m).find((part) =>
		part.startsWith("workspace.package]")
	);
	const version = section?.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
	if (!version) {
		throw new Error("Cargo.toml has no [workspace.package] version");
	}
	return version;
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

function escapeHtml(value: string): string {
	return value.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(
		/>/g,
		"&gt;",
	).replace(/"/g, "&quot;");
}

function stripComments(value: string): string {
	return value.replace(/<!--[\s\S]*?-->/g, "");
}

/** Escapes a markdown cell and renders its inline code spans. */
function inline(value: string): string {
	return escapeHtml(value.trim()).replace(/`([^`]+)`/g, "<code>$1</code>");
}

function formatNumber(value: number): string {
	return value.toLocaleString("en-US");
}

/** How a value compares with Pina's, phrased for a bar's side label. */
function versusPina(value: number, pina: number): string {
	if (value === pina) {
		return "same as Pina";
	}
	const ratio = value / pina;
	if (ratio >= 2) {
		return `${ratio.toFixed(ratio >= 10 ? 0 : 1)}× Pina`;
	}
	const percent = Math.round((ratio - 1) * 100);
	return `${percent > 0 ? "+" : "−"}${Math.abs(percent)}% vs Pina`;
}

/** Signed percentage against the hand-written Pinocchio floor. */
function versusFloor(value: number, floor: number): string {
	const percent = Math.round(((value - floor) / floor) * 100);
	return `${percent >= 0 ? "+" : "−"}${Math.abs(percent)}%`;
}

function find(rows: Measurement[], framework: string): Measurement {
	const entry = rows.find((row) => row.framework === framework);
	if (!entry) {
		throw new Error(`no ${framework} measurement`);
	}
	return entry;
}

function cu(entry: Measurement, instruction: string): number {
	const value = entry.cu[instruction];
	if (value === undefined) {
		throw new Error(`no ${instruction} compute units for ${entry.framework}`);
	}
	return value;
}

/**
 * Renders the hero stat tiles. The size and compute tiles compare Pina's
 * measurements with hand-written Pinocchio and Anchor v1; the last tile states
 * the `no_std` guarantee, which no benchmark measures.
 */
export function renderStats(benchmarks: Record<string, Measurement[]>): string {
	const hello = benchmarks.hello;
	const counter = benchmarks.counter;
	const pina = find(hello, PINA);
	const floor = find(hello, PINOCCHIO);
	const anchor = find(hello, ANCHOR_V1);
	const pinaIncrement = cu(find(counter, PINA), "increment");
	const anchorIncrement = cu(find(counter, ANCHOR_V1), "increment");

	const versusFloorBytes = Math.round(
		(Math.abs(floor.bytes - pina.bytes) / floor.bytes) * 100,
	);
	const sizeRatio = Math.round(anchor.bytes / pina.bytes);
	const computeRatio = Math.round(anchorIncrement / pinaIncrement);

	const tiles = [
		{
			value: formatNumber(pina.bytes),
			unit: "bytes",
			label: "Hello world, deployed",
			sub: pina.bytes <= floor.bytes
				? `${versusFloorBytes}% smaller than hand-written Pinocchio.`
				: `${versusFloorBytes}% over hand-written Pinocchio.`,
		},
		{
			value: `${sizeRatio}×`,
			unit: "",
			label: "Smaller than Anchor v1",
			sub: `The same program is ${
				formatNumber(anchor.bytes)
			} bytes in Anchor v1.`,
		},
		{
			value: formatNumber(pinaIncrement),
			unit: "CU",
			label: "Per counter increment",
			sub: `${computeRatio}× less compute than Anchor v1.`,
		},
		{
			value: "0",
			unit: "",
			label: "Heap allocations",
			sub: "Every on-chain crate is no_std with no allocator.",
		},
	];

	return tiles.map((tile) =>
		`<li class="stat"><p class="stat-value">${escapeHtml(tile.value)}${
			tile.unit ? `<span class="stat-unit">${tile.unit}</span>` : ""
		}</p><p class="stat-label">${
			escapeHtml(tile.label)
		}</p><p class="stat-sub">${escapeHtml(tile.sub)}</p></li>`
	).join("\n\t\t\t\t\t");
}

const METRICS: Metric[] = [
	{
		id: "hello-size",
		tab: "Hello world · size",
		title: "Hello world: deployed program size",
		description: "Bytes of the deployed .so",
		program: "hello",
		unit: "bytes",
		value: (entry) => entry.bytes,
	},
	{
		id: "hello-cu",
		tab: "Hello world · CU",
		title: "Hello world: hello instruction",
		description: "Compute units consumed",
		program: "hello",
		unit: "CU",
		value: (entry) => cu(entry, "hello"),
	},
	{
		id: "counter-size",
		tab: "Counter · size",
		title: "Counter: deployed program size",
		description: "Bytes of the deployed .so",
		program: "counter",
		unit: "bytes",
		value: (entry) => entry.bytes,
	},
	{
		id: "counter-initialize",
		tab: "Counter · initialize",
		title: "Counter: initialize instruction",
		description: "Compute units consumed, including the account-creation CPI",
		program: "counter",
		unit: "CU",
		value: (entry) => cu(entry, "initialize"),
	},
	{
		id: "counter-increment",
		tab: "Counter · increment",
		title: "Counter: increment instruction",
		description: "Compute units consumed",
		program: "counter",
		unit: "CU",
		value: (entry) => cu(entry, "increment"),
	},
];

function renderPanel(
	metric: Metric,
	rows: Measurement[],
	index: number,
): string {
	const values = rows.map((entry) => ({ entry, value: metric.value(entry) }))
		.sort((left, right) => left.value - right.value);
	const pina = metric.value(find(rows, PINA));
	const floor = metric.value(find(rows, PINOCCHIO));
	const largest = values[values.length - 1].value;
	const runnerUp = values.length > 1
		? values[values.length - 2].value
		: largest;
	const broken = largest > runnerUp * BREAK_RATIO;
	const domain = broken ? runnerUp * 1.15 : largest;

	const bars = values.map(({ entry, value }, position) => {
		const isPina = entry.framework === PINA;
		const isBroken = broken && value === largest;
		const width = Math.min(value / domain, 1).toFixed(4);
		const side = isPina ? "" : `<small>${versusPina(value, pina)}</small>`;
		return `<li class="bar-row${isPina ? " is-pina" : ""}" tabindex="0">
						<span class="bar-label"><b>${inline(entry.framework)}</b><small>${
			inline(entry.version)
		}</small></span>
						<span class="bar-track"><span class="bar${
			isBroken ? " is-broken" : ""
		}" style="--w: ${width}; --i: ${position}"></span><span class="bar-value">${
			formatNumber(value)
		}${side}</span></span>
						<span class="tooltip" aria-hidden="true"><b>${
			inline(entry.framework)
		}</b><span>${
			escapeHtml(metric.unit === "CU" ? "Compute units" : "Size")
		}<em>${
			formatNumber(value)
		} ${metric.unit}</em></span><span>vs hand-written Pinocchio<em>${
			versusFloor(value, floor)
		}</em></span>${
			isBroken
				? "<span>Drawn past the axis break<em>not to scale</em></span>"
				: ""
		}</span>
					</li>`;
	}).join("\n\t\t\t\t\t");

	return `<div class="chart-panel" role="tabpanel" id="panel-${metric.id}" aria-labelledby="tab-${metric.id}"${
		index === 0 ? "" : " hidden"
	}>
				<div class="chart-title"><h3>${escapeHtml(metric.title)}</h3><p>${
		escapeHtml(metric.description)
	} · lower is better</p></div>
				<ol class="bars">
					${bars}
				</ol>
			</div>`;
}

function renderTable(
	title: string,
	rows: Measurement[],
): string {
	const instructions = Object.keys(rows[0].cu);
	const floor = find(rows, PINOCCHIO);
	const head = [
		"Framework",
		"Version",
		"Size (bytes)",
		...instructions.map((name) => `<code>${escapeHtml(name)}</code> CU`),
		"vs Pinocchio size",
	].map((cell) => `<th scope="col">${cell}</th>`).join("");
	const body = rows.map((entry) =>
		`<tr><th scope="row">${inline(entry.framework)}</th><td>${
			inline(entry.version)
		}</td><td>${formatNumber(entry.bytes)}</td>${
			instructions.map((name) => `<td>${formatNumber(cu(entry, name))}</td>`)
				.join("")
		}<td>${versusFloor(entry.bytes, floor.bytes)}</td></tr>`
	).join("");
	return `<div class="table-scroll"><table class="data-table"><caption>${
		escapeHtml(title)
	}</caption><thead><tr>${head}</tr></thead><tbody>${body}</tbody></table></div>`;
}

/**
 * Renders the benchmark chart: one tab and bar panel per metric, each with a
 * table view of the same measurements.
 */
export function renderBenchmarks(
	benchmarks: Record<string, Measurement[]>,
): string {
	const tabs = METRICS.map((metric, index) =>
		`<button role="tab" type="button" id="tab-${metric.id}" aria-controls="panel-${metric.id}" aria-selected="${
			index === 0
		}"${index === 0 ? "" : ' tabindex="-1"'}>${escapeHtml(metric.tab)}</button>`
	).join("");
	const panels = METRICS.map((metric, index) =>
		renderPanel(metric, benchmarks[metric.program], index)
	).join("\n\t\t\t");

	return `<div class="chart">
			<div class="chart-tabs" role="tablist" aria-label="Benchmark">${tabs}</div>
			${panels}
			<div class="chart-foot">
				<span>Bars past a break are drawn to a shortened scale; their full value is printed.</span>
				<a href="framework-comparison.html">Methodology and caveats →</a>
			</div>
			<details class="chart-table">
				<summary>View the measurements as a table</summary>
				${renderTable("Hello world", benchmarks.hello)}
				${renderTable("Counter", benchmarks.counter)}
			</details>
		</div>`;
}

const MARKS: Record<CapabilityCell["status"], string> = {
	yes:
		'<span class="mark mark-yes" role="img" aria-label="First-class"><svg viewBox="0 0 12 12"><path d="M2.5 6.2l2.3 2.3 4.7-5"/></svg></span>',
	partial:
		'<span class="mark mark-partial" role="img" aria-label="Partial"><svg viewBox="0 0 12 12"><path d="M3 6h6"/></svg></span>',
	no:
		'<span class="mark mark-no" role="img" aria-label="Not available"><svg viewBox="0 0 12 12"><path d="M3.5 3.5l5 5M8.5 3.5l-5 5"/></svg></span>',
};

/** Renders the capability matrix, with Pina's column first and highlighted. */
export function renderCapabilities(matrix: CapabilityMatrix): string {
	const head = matrix.frameworks.map((framework, index) =>
		`<th scope="col"${index === 0 ? ' class="col-pina"' : ""}>${framework}${
			matrix.versions[index] ? `<small>${matrix.versions[index]}</small>` : ""
		}</th>`
	).join("");
	const body = matrix.rows.map((row) =>
		`<tr><th scope="row">${row.name}</th>${
			row.cells.map((cell, index) =>
				`<td${index === 0 ? ' class="col-pina"' : ""}><span class="cell">${
					MARKS[cell.status]
				}<span class="cell-note">${cell.note}</span></span></td>`
			).join("")
		}</tr>`
	).join("\n\t\t\t\t\t\t");

	return `<div class="matrix-scroll">
				<table class="matrix">
					<thead><tr><th scope="col">Capability</th>${head}</tr></thead>
					<tbody>
						${body}
					</tbody>
				</table>
			</div>
			<p class="matrix-legend"><span>${MARKS.yes} First-class</span><span>${MARKS.partial} Partial</span><span>${MARKS.no} Not available</span></p>`;
}

/**
 * Fills the landing template. `{{name}}` takes an escaped value and
 * `<!-- pina:name -->` a rendered block; an unknown name throws so a template
 * typo fails the docs build instead of shipping.
 */
export function renderLanding(
	template: string,
	sources: { manifest: string; comparison: string; readme: string },
): string {
	const version = workspaceVersion(sources.manifest);
	const benchmarks = parseBenchmarks(sources.comparison);
	const capabilities = parseCapabilities(sources.readme);
	const values: Record<string, string> = {
		version: escapeHtml(version),
		counterIncrementCu: formatNumber(
			cu(find(benchmarks.counter, PINA), "increment"),
		),
	};
	const blocks: Record<string, string> = {
		stats: renderStats(benchmarks),
		benchmarks: renderBenchmarks(benchmarks),
		capabilities: renderCapabilities(capabilities),
	};

	const html = template
		.replace(/\{\{(\w+)\}\}/g, (match, name: string) => {
			if (!(name in values)) {
				throw new Error(`unknown landing value {{${name}}}`);
			}
			return values[name];
		})
		.replace(/<!-- pina:(\w+) -->/g, (match, name: string) => {
			if (!(name in blocks)) {
				throw new Error(`unknown landing block <!-- pina:${name} -->`);
			}
			return blocks[name];
		});
	return html;
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

function copyInto(source: string, destination: string): void {
	mkdirSync(dirname(destination), { recursive: true });
	copyFileSync(source, destination);
}

/**
 * Writes the landing page over a built book's `index.html` and copies the
 * page's assets, the favicon, and the theme fonts into `<bookDir>/landing/`.
 */
export function buildLanding(bookDir: string, root = ROOT): void {
	if (!existsSync(join(bookDir, "introduction.html"))) {
		throw new Error(
			`${bookDir} does not look like a built book (no introduction.html)`,
		);
	}
	const landing = join(root, "docs/landing");
	const html = renderLanding(
		readFileSync(join(landing, "index.html"), "utf8"),
		{
			manifest: readFileSync(join(root, "Cargo.toml"), "utf8"),
			comparison: readFileSync(
				join(root, "docs/src/framework-comparison.md"),
				"utf8",
			),
			readme: readFileSync(join(root, "readme.md"), "utf8"),
		},
	);
	writeFileSync(join(bookDir, "index.html"), html);

	const output = join(bookDir, "landing");
	for (const file of readdirSync(landing)) {
		if (file !== "index.html") {
			copyInto(join(landing, file), join(output, file));
		}
	}
	copyInto(
		join(root, "docs/theme/favicon.svg"),
		join(output, "favicon.svg"),
	);
	const fonts = join(root, "docs/theme/fonts");
	for (const file of readdirSync(fonts)) {
		if (file.endsWith(".woff2") || file.endsWith(".txt")) {
			copyInto(join(fonts, file), join(output, "fonts", file));
		}
	}
}

function main(): number {
	const [bookArgument] = process.argv.slice(2);
	if (!bookArgument) {
		process.stderr.write("usage: build-landing.ts <bookDir>\n");
		return 2;
	}
	try {
		buildLanding(resolve(bookArgument));
		return 0;
	} catch (error) {
		process.stderr.write(
			`build-landing: ${
				error instanceof Error ? error.message : String(error)
			}\n`,
		);
		return 1;
	}
}

if (
	process.argv[1] !== undefined &&
	import.meta.url === pathToFileURL(process.argv[1]).href
) {
	process.exitCode = main();
}
