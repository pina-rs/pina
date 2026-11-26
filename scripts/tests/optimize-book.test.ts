import assert from "node:assert/strict";
import {
	existsSync,
	mkdirSync,
	mkdtempSync,
	readFileSync,
	writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";
import vm from "node:vm";

import {
	buildCompactSearchData,
	highlightAndEnhanceCodeBlocks,
	minifyCss,
	minifyHtml,
	minifyJs,
	minifyStandaloneHtml,
	optimizeBook,
	rebaseCssUrls,
} from "../docs/optimize-book.ts";

const REPO_ROOT = resolve(import.meta.dirname, "../..");

test("minifyCss strips comments and preserves strings, hex escapes, escaped punctuation, calc, and descendant pseudo-classes", () => {
	const css = `
		/* comment */
		pre > .buttons :hover {
			width: calc(100% - 12px);
			content: url('data:image/svg+xml,<svg \\
viewBox="0 0 10 10"></svg>');
		}
		.\\31  .item { color: red; }
		.\\31 .same { color: blue; }
		.x\\> .y { color: green; }
		a > b { color: black; }
	`;
	const out = minifyCss(css);
	assert.equal(
		out,
		`pre>.buttons :hover{width:calc(100% - 12px);content:url('data:image/svg+xml,<svg viewBox="0 0 10 10"></svg>')}.\\31  .item{color:red}.\\31 .same{color:blue}.x\\> .y{color:green}a>b{color:black}`,
	);
});

test("minifyJs preserves regex literals after control-flow conditions, operators, strings, template literals, and all ECMAScript line terminators", () => {
	const js = `
		// line comment
		const url = "https://pina.rs/docs";
		/* block
		   comment */
		const re = /https?:\\/\\//g;
		if (ok) /[//]/.test(value);
		while (ok) /foo\\//.test(value);
		const quotient = (a + b) / 2;
		const msg = \`hello
world\`;
		function add(a, b) {
			return a + b;
		}
		function crComment() {
			return/*\r*/ 1;
		}
		function lsComment() {
			return/*\u2028*/ 2;
		}
		function psComment() {
			return/*\u2029*/ 3;
		}
	`;
	const out = minifyJs(js);
	assert.doesNotThrow(() => new vm.Script(out));
	assert.match(out, /"https:\/\/pina\.rs\/docs"/);
	assert.match(out, /\/https\?:\\\/\\\/\/g/);
	assert.match(out, /if \(ok\) \/\[\/\/\]\/\.test\(value\);/);
	assert.match(out, /while \(ok\) \/foo\\\/\/\.test\(value\);/);
	assert.match(out, /const quotient = \(a \+ b\) \/ 2;/);
	assert.match(out, /`hello\nworld`/);
	assert.doesNotMatch(out, /line comment|block/);
	const sandbox: Record<string, unknown> = {
		ok: false,
		value: "/",
		a: 2,
		b: 4,
	};
	vm.runInNewContext(
		`${out}\nglobalThis.__cr = crComment(); globalThis.__ls = lsComment(); globalThis.__ps = psComment(); globalThis.__sum = add(2, 3);`,
		sandbox,
	);
	assert.equal(sandbox.__cr, undefined);
	assert.equal(sandbox.__ls, undefined);
	assert.equal(sandbox.__ps, undefined);
	assert.equal(sandbox.__sum, 5);
});

test("minifyHtml preserves pre/code and quoted attribute whitespace including tag-like text while minifying inline style/script", () => {
	const html = `<!DOCTYPE html>
	<html>
		<head>
			<!-- strip me -->
			<style>
				/* comment */
				body { margin: 0; }
			</style>
		</head>
		<body>
			<input   type="text"   value="a  b"   title="line 1\n  line 2">
			<p title="left>  <div right"   data-plain="hello  world">  Text  </p>
			<p>Hello   <strong>world</strong>  and  <a href="/">links</a></p>
			<pre><code>  keep   spaces\n  here</code></pre>
		</body>
	</html>`;
	const out = minifyHtml(html);
	assert.doesNotMatch(out, /strip me/);
	assert.match(out, /<style>body\{margin:0\}<\/style>/);
	assert.match(
		out,
		/<input type="text" value="a  b" title="line 1\n  line 2">/,
	);
	assert.match(
		out,
		/<p title="left>  <div right" data-plain="hello  world">Text<\/p>/,
	);
	assert.match(out, /<pre><code>  keep   spaces\n  here<\/code><\/pre>/);
	assert.match(out, /<strong>world<\/strong> and <a href="\/">links<\/a>/);
});

test("minifyStandaloneHtml preserves .cmd and .tree line breaks in flow-interactive.html", () => {
	const flowHtml = readFileSync(
		join(REPO_ROOT, "docs/src/migrations/flow-interactive.html"),
		"utf8",
	);
	const out = minifyStandaloneHtml(flowHtml);
	assert.match(
		out,
		/<div class="cmd"><span class="prompt">\$ <\/span>pina build\n<span class="prompt">\$ <\/span>pina deploy/,
	);
	assert.match(
		out,
		/<div class="cmd"><span class="comment">\/\/ TODO\(pina-manual-migration\):[^\n]*\npub\(crate\) const SOURCE_SIZE: usize = 42;/,
	);
	assert.match(out, /<div class="tree"><span class="d">migrations\/<\/span>\n/);
});

test("rebaseCssUrls adjusts relative paths including quoted URLs with parentheses", () => {
	const css =
		`src: url('../fonts/a.woff2'), url("icons/a(b).svg"), url('icons/c(d).svg?v=1#id'), url('data:image/svg+xml,foo'), url('https://x/y');`;
	assert.equal(
		rebaseCssUrls(css, "css/general-12345678.css", "assets/book.css"),
		`src: url('../fonts/a.woff2'), url("../css/icons/a(b).svg"), url('../css/icons/c(d).svg?v=1#id'), url('data:image/svg+xml,foo'), url('https://x/y');`,
	);
	assert.equal(
		rebaseCssUrls(`url('icon.svg')`, "theme/pina.css", "assets/book.css"),
		`url('../theme/icon.svg')`,
	);
});

test("buildCompactSearchData extracts urls and normalized doc tuples", () => {
	const payload = JSON.stringify({
		doc_urls: ["intro.html#top"],
		index: {
			documentStore: {
				docs: {
					"0": {
						breadcrumbs: "Intro » Top",
						body: "Hello   \n  world",
					},
				},
			},
		},
	});
	const js = `window.search = Object.assign(window.search, JSON.parse(${
		JSON.stringify(payload)
	}));`;
	const compact = buildCompactSearchData(js);
	assert.deepEqual(compact, {
		u: ["intro.html#top"],
		d: [["Intro » Top", "Hello world"]],
	});
});

test("highlightAndEnhanceCodeBlocks highlights code, preserves boring spans, and adds static buttons", () => {
	const fakeHljs = {
		getLanguage: (lang: string) => lang === "rust",
		highlight: (_lang: string, code: string) => ({
			value: code.replace(/fn/g, '<span class="hljs-keyword">fn</span>'),
		}),
	};
	const html =
		'<pre class="playground"><code class="language-rust"><span class="boring">fn main() {\n</span>let x = 1;\n<span class="boring">}</span></code></pre>';
	const { html: out, hasBoring } = highlightAndEnhanceCodeBlocks(
		html,
		fakeHljs,
		true,
	);
	assert.equal(hasBoring, true);
	assert.match(out, /class="clip-button"/);
	assert.match(out, /class="boring-toggle"/);
	assert.match(out, /class="language-rust hljs hide-boring"/);
	assert.match(
		out,
		/<span class="boring"><span class="hljs-keyword">fn<\/span> main\(\) \{\n<\/span>/,
	);
});

function createFakeBook(
	options: { hashed?: boolean; extraCss?: string; standaloneHtml?: string } =
		{},
): string {
	const hashed = options.hashed ?? true;
	const suffix = (hex: string) => (hashed ? `-${hex}` : "");
	const dir = mkdtempSync(join(tmpdir(), "pina-opt-book-"));
	mkdirSync(join(dir, "css"));
	mkdirSync(join(dir, "fonts"));
	mkdirSync(join(dir, "theme"));

	const varsCss = `css/variables${suffix("11111111")}.css`;
	const genCss = `css/general${suffix("22222222")}.css`;
	const chromeCss = `css/chrome${suffix("33333333")}.css`;
	const printCss = `css/print${suffix("44444444")}.css`;
	const fontsCss = `fonts/fonts${suffix("55555555")}.css`;
	const hlCss = `highlight${suffix("66666666")}.css`;
	const tnCss = `tomorrow-night${suffix("77777777")}.css`;
	const ayuCss = `ayu-highlight${suffix("88888888")}.css`;
	const pinaCss = `theme/pina${suffix("99999999")}.css`;
	const pinaJs = `theme/pina${suffix("aaaaaaaa")}.js`;
	const hlJs = `highlight${suffix("bbbbbbbb")}.js`;
	const bookJs = `book${suffix("cccccccc")}.js`;
	const clipJs = `clipboard${suffix("dddddddd")}.min.js`;
	const lunrJs = `elasticlunr${suffix("eeeeeeee")}.min.js`;
	const markJs = `mark${suffix("ffffffff")}.min.js`;
	const searcherJs = `searcher${suffix("12345678")}.js`;
	const tocJs = `toc${suffix("87654321")}.js`;
	const searchIdxJs = `searchindex${suffix("abcdef12")}.js`;

	writeFileSync(join(dir, varsCss), ":root { --bg: #fff; }");
	writeFileSync(join(dir, genCss), "body { color: #111; }");
	writeFileSync(join(dir, chromeCss), ".page { padding: 0; }");
	writeFileSync(join(dir, printCss), ".sidebar { display: none; }");
	writeFileSync(
		join(dir, fontsCss),
		"@font-face { src: url('../fonts/a.woff2'); }",
	);
	writeFileSync(join(dir, hlCss), ".hljs { color: #000; }");
	writeFileSync(join(dir, tnCss), ".hljs { color: #fff; }");
	writeFileSync(join(dir, ayuCss), ".hljs { color: #eee; }");
	writeFileSync(join(dir, pinaCss), "html .hljs { color: #e8e4d4; }");
	writeFileSync(join(dir, pinaJs), "(function(){})();");
	writeFileSync(
		join(dir, hlJs),
		`window.hljs = { configure(){}, getLanguage(l){ return l === "rust"; }, highlight(l, c){ return { value: c.replace(/let/g, '<span class="hljs-keyword">let</span>') }; } };`,
	);
	writeFileSync(join(dir, bookJs), "// original book js");
	writeFileSync(join(dir, clipJs), "// clipboard");
	writeFileSync(join(dir, lunrJs), "// elasticlunr");
	writeFileSync(join(dir, markJs), "// mark");
	writeFileSync(join(dir, searcherJs), "// searcher");
	writeFileSync(
		join(dir, tocJs),
		`class T extends HTMLElement { connectedCallback() { this.innerHTML = '<ol></ol>'; // Track and set sidebar scroll position\n } }`,
	);
	if (options.extraCss) {
		writeFileSync(join(dir, options.extraCss), ".custom { color: green; }");
	}
	const payload = JSON.stringify({
		doc_urls: ["introduction.html#intro", "entities.html#amp"],
		index: {
			documentStore: {
				docs: {
					"0": { breadcrumbs: "Intro", body: "Pina framework docs" },
					"1": { breadcrumbs: "Entities", body: "amp & <script>safe</script>" },
				},
			},
		},
	});
	writeFileSync(
		join(dir, searchIdxJs),
		`window.search = Object.assign(window.search, JSON.parse(${
			JSON.stringify(payload)
		}));`,
	);
	writeFileSync(
		join(dir, "introduction.html"),
		`<!DOCTYPE HTML>
<html>
<head>
<link rel="stylesheet" href="${varsCss}">
<link rel="stylesheet" href="${genCss}">
<link rel="stylesheet" href="${chromeCss}">
<link rel="stylesheet" href="${printCss}" media="print">
<link rel="stylesheet" href="${fontsCss}">
<link rel="stylesheet" id="mdbook-highlight-css" href="${hlCss}">
<link rel="stylesheet" id="mdbook-tomorrow-night-css" href="${tnCss}">
<link rel="stylesheet" id="mdbook-ayu-highlight-css" href="${ayuCss}">
<link rel="stylesheet" href="${pinaCss}">
${options.extraCss ? `<link rel="stylesheet" href="${options.extraCss}">` : ""}
<script>const path_to_root = ""; window.path_to_searchindex_js = "${searchIdxJs}";</script>
<script src="${tocJs}"></script>
</head>
<body>
<div id="mdbook-body-container">
<h1 class="menu-title">Pina</h1>
<button id="mdbook-theme-navy">Navy</button>
<main><pre><code class="language-rust">let x = 1;</code></pre></main>
<template id=fa-play>play</template>
<script>window.playground_copyable = true;</script>
<script src="${lunrJs}"></script>
<script src="${markJs}"></script>
<script src="${searcherJs}"></script>
<script src="${clipJs}"></script>
<script src="${hlJs}"></script>
<script src="${bookJs}"></script>
<script src="${pinaJs}"></script>
</div>
</body>
</html>`,
	);
	if (options.standaloneHtml) {
		mkdirSync(join(dir, "migrations"), { recursive: true });
		writeFileSync(
			join(dir, "migrations/flow-interactive.html"),
			options.standaloneHtml,
		);
	}
	return dir;
}

test("optimizeBook bundles CSS, preserves custom external CSS, supports idempotency, and keeps standalone .cmd breaks", () => {
	const standaloneHtml = readFileSync(
		join(REPO_ROOT, "docs/src/migrations/flow-interactive.html"),
		"utf8",
	);
	const dir = createFakeBook({
		hashed: true,
		extraCss: "custom.css",
		standaloneHtml,
	});

	const stats = optimizeBook(dir);
	assert.equal(stats.htmlFilesOptimized, 2);
	assert.ok(stats.searchDataFile);
	assert.ok(stats.searchJsFile);

	const outHtml = readFileSync(join(dir, "introduction.html"), "utf8");
	assert.match(outHtml, new RegExp(stats.bundleCssFile));
	assert.match(outHtml, /<link rel="stylesheet" href="custom\.css">/);
	assert.equal(existsSync(join(dir, "custom.css")), true);
	assert.match(outHtml, new RegExp(`defer src="${stats.bundleJsFile}"`));
	assert.doesNotMatch(outHtml, /toc-87654321\.js/);
	assert.match(outHtml, /<span class="hljs-keyword">let<\/span>/);
	assert.match(outHtml, /class="pina-brand"/);
	assert.match(outHtml, /class="pina-footer"/);
	assert.match(outHtml, /id="mdbook-theme-navy">Dark<\/button>/);
	assert.doesNotMatch(
		outHtml,
		/highlight-bbbbbbbb|elasticlunr|clipboard-dddddddd|fa-play/,
	);

	// Standalone flow-interactive.html preserves `.cmd` multi-line commands.
	const outFlow = readFileSync(
		join(dir, "migrations/flow-interactive.html"),
		"utf8",
	);
	assert.match(
		outFlow,
		/<div class="cmd"><span class="prompt">\$ <\/span>pina build\n<span class="prompt">\$ <\/span>pina deploy/,
	);

	assert.equal(existsSync(join(dir, "highlight-bbbbbbbb.js")), false);
	assert.equal(existsSync(join(dir, "toc-87654321.js")), false);
	assert.equal(existsSync(join(dir, "searchindex-abcdef12.js")), false);
	assert.equal(existsSync(join(dir, stats.bundleCssFile)), true);
	assert.equal(existsSync(join(dir, stats.bundleJsFile)), true);
	assert.equal(existsSync(join(dir, stats.searchDataFile)), true);
	assert.equal(existsSync(join(dir, stats.searchJsFile)), true);

	// Second invocation is idempotent and preserves the working bundle.
	const second = optimizeBook(dir);
	assert.equal(second.htmlFilesOptimized, 0);
	assert.equal(second.bundleCssFile, stats.bundleCssFile);
	assert.equal(second.bundleJsFile, stats.bundleJsFile);
	assert.equal(readFileSync(join(dir, "introduction.html"), "utf8"), outHtml);
});

test("optimizeBook handles hash-files = false and preserves stylesheet links when no CSS assets exist", () => {
	const unhashedDir = createFakeBook({ hashed: false });
	const unhashedStats = optimizeBook(unhashedDir);
	assert.equal(unhashedStats.htmlFilesOptimized, 1);
	assert.match(unhashedStats.bundleCssFile, /^assets\/book-[a-f0-9]{8}\.css$/);
	assert.equal(
		existsSync(join(unhashedDir, unhashedStats.bundleCssFile)),
		true,
	);
	assert.match(
		readFileSync(join(unhashedDir, "introduction.html"), "utf8"),
		new RegExp(unhashedStats.bundleCssFile),
	);

	// Book missing CSS files keeps its original stylesheet links untouched.
	const noCssDir = mkdtempSync(join(tmpdir(), "pina-opt-nocss-"));
	writeFileSync(
		join(noCssDir, "chapter.html"),
		`<!DOCTYPE html><html><head><link rel="stylesheet" href="external.css"></head><body><div id="mdbook-body-container"><main>Hello</main></div></body></html>`,
	);
	const noCssStats = optimizeBook(noCssDir);
	assert.equal(noCssStats.bundleCssFile, "");
	assert.match(
		readFileSync(join(noCssDir, "chapter.html"), "utf8"),
		/<link rel="stylesheet" href="external\.css">/,
	);
});

// Minimal DOM stub for executing emitted browser scripts (`assets/search-*.js` & `assets/book-*.js`).
interface StubNode {
	nodeType: number;
	nodeName: string;
	tagName?: string;
	id: string;
	className: string;
	classList: {
		add: (...c: string[]) => void;
		remove: (...c: string[]) => void;
		contains: (c: string) => boolean;
		toggle: (c: string, force?: boolean) => boolean;
	};
	attributes: Map<string, string>;
	children: StubNode[];
	childNodes: StubNode[];
	parentNode: StubNode | null;
	parentElement: StubNode | null;
	nodeValue: string | null;
	value: string;
	style: Record<string, string>;
	listeners: Map<string, Array<(event: unknown) => void>>;
	setAttribute: (k: string, v: string) => void;
	getAttribute: (k: string) => string | null;
	appendChild: (child: StubNode) => StubNode;
	replaceChild: (next: StubNode, old: StubNode) => StubNode;
	remove: () => void;
	normalize: () => void;
	closest: (sel: string) => StubNode | null;
	querySelector: (sel: string) => StubNode | null;
	querySelectorAll: (sel: string) => StubNode[];
	addEventListener: (type: string, fn: (e: unknown) => void) => void;
	removeEventListener: (type: string, fn: (e: unknown) => void) => void;
	dispatch: (type: string, event?: Record<string, unknown>) => void;
	select: () => void;
	blur: () => void;
	focus: () => void;
	contains: (other: StubNode | null) => boolean;
	textContent: string;
	firstChild: StubNode | null;
	firstElementChild: StubNode | null;
	nextElementSibling: StubNode | null;
	previousElementSibling: StubNode | null;
	src?: string;
	onload?: () => void;
	onerror?: () => void;
	type?: string;
	innerText?: string;
}

function makeStubNode(
	tag: string,
	opts: { id?: string; classes?: string[]; nodeType?: number; text?: string } =
		{},
): StubNode {
	const classSet = new Set(opts.classes ?? []);
	const listeners = new Map<string, Array<(event: unknown) => void>>();
	const childNodes: StubNode[] = [];
	const attributes = new Map<string, string>();
	let textVal = opts.text ?? "";
	const nodeType = opts.nodeType ?? 1;

	const node: StubNode = {
		nodeType,
		nodeName: nodeType === 3 ? "#text" : tag.toUpperCase(),
		tagName: nodeType === 3 ? undefined : tag.toUpperCase(),
		id: opts.id ?? "",
		get className() {
			return Array.from(classSet).join(" ");
		},
		set className(val: string) {
			classSet.clear();
			for (const cls of val.split(/\s+/).filter(Boolean)) {
				classSet.add(cls);
			}
		},
		classList: {
			add: (...cs: string[]) => {
				cs.forEach((c) => classSet.add(c));
			},
			remove: (...cs: string[]) => {
				cs.forEach((c) => classSet.delete(c));
			},
			contains: (c: string) => classSet.has(c),
			toggle: (c: string, force?: boolean) => {
				const on = force === undefined ? !classSet.has(c) : Boolean(force);
				if (on) {
					classSet.add(c);
				} else {
					classSet.delete(c);
				}
				return on;
			},
		},
		attributes,
		get children() {
			return childNodes.filter((c) => c.nodeType === 1);
		},
		childNodes,
		parentNode: null,
		parentElement: null,
		get nodeValue() {
			return nodeType === 3 ? textVal : null;
		},
		set nodeValue(v: string | null) {
			textVal = v ?? "";
		},
		value: "",
		style: {},
		listeners,
		setAttribute(k: string, v: string) {
			attributes.set(k, v);
		},
		getAttribute(k: string) {
			return attributes.get(k) ?? null;
		},
		appendChild(child: StubNode) {
			if (child.nodeType === 11) {
				for (const nested of [...child.childNodes]) {
					node.appendChild(nested);
				}
				child.childNodes.length = 0;
				return child;
			}
			child.parentNode = node;
			child.parentElement = node.nodeType === 1 ? node : null;
			childNodes.push(child);
			return child;
		},
		replaceChild(next: StubNode, old: StubNode) {
			const idx = childNodes.indexOf(old);
			if (idx === -1) {
				return old;
			}
			const insertNodes = next.nodeType === 11 ? [...next.childNodes] : [next];
			for (const ins of insertNodes) {
				ins.parentNode = node;
				ins.parentElement = node.nodeType === 1 ? node : null;
			}
			childNodes.splice(idx, 1, ...insertNodes);
			old.parentNode = null;
			old.parentElement = null;
			return old;
		},
		remove() {
			if (!node.parentNode) {
				return;
			}
			const idx = node.parentNode.childNodes.indexOf(node);
			if (idx >= 0) {
				node.parentNode.childNodes.splice(idx, 1);
			}
			node.parentNode = null;
			node.parentElement = null;
		},
		normalize() {},
		closest(sel: string) {
			const parts = sel.split(",").map((s) => s.trim());
			let cur: StubNode | null = node;
			while (cur) {
				for (const part of parts) {
					if (part === "button.clip-button") {
						if (
							cur.tagName === "BUTTON" &&
							cur.classList.contains("clip-button")
						) {
							return cur;
						}
					} else if (cur.tagName?.toLowerCase() === part.toLowerCase()) {
						return cur;
					}
				}
				cur = cur.parentElement;
			}
			return null;
		},
		querySelector(sel: string) {
			return node.querySelectorAll(sel)[0] ?? null;
		},
		querySelectorAll(sel: string) {
			const out: StubNode[] = [];
			const walk = (n: StubNode) => {
				for (const child of n.childNodes) {
					if (child.nodeType === 1) {
						if (
							(sel === "mark.pina-hl" &&
								child.tagName === "MARK" &&
								child.classList.contains("pina-hl")) ||
							(sel === "code" && child.tagName === "CODE") ||
							(sel === "em" && child.tagName === "EM") ||
							(sel === "a" && child.tagName === "A") ||
							(sel === "button" && child.tagName === "BUTTON")
						) {
							out.push(child);
						}
						walk(child);
					}
				}
			};
			walk(node);
			return out;
		},
		addEventListener(type: string, fn: (e: unknown) => void) {
			const arr = listeners.get(type) ?? [];
			arr.push(fn);
			listeners.set(type, arr);
		},
		removeEventListener(type: string, fn: (e: unknown) => void) {
			const arr = listeners.get(type);
			if (!arr) {
				return;
			}
			const idx = arr.indexOf(fn);
			if (idx >= 0) {
				arr.splice(idx, 1);
			}
		},
		dispatch(type: string, event: Record<string, unknown> = {}) {
			for (const fn of [...(listeners.get(type) ?? [])]) {
				fn({
					preventDefault() {},
					stopPropagation() {},
					target: node,
					...event,
				});
			}
		},
		select() {},
		blur() {},
		focus() {},
		contains(other: StubNode | null) {
			let cur = other;
			while (cur) {
				if (cur === node) {
					return true;
				}
				cur = cur.parentNode;
			}
			return false;
		},
		get textContent() {
			if (nodeType === 3) {
				return textVal;
			}
			return childNodes.map((c) => c.textContent).join("");
		},
		set textContent(val: string) {
			if (nodeType === 3) {
				textVal = val;
				return;
			}
			childNodes.length = 0;
			if (val !== "") {
				node.appendChild(makeStubNode("#text", { nodeType: 3, text: val }));
			}
		},
		get firstChild() {
			return childNodes[0] ?? null;
		},
		get firstElementChild() {
			return node.children[0] ?? null;
		},
		get nextElementSibling() {
			if (!node.parentNode) {
				return null;
			}
			const sibs = node.parentNode.children;
			const idx = sibs.indexOf(node);
			return idx >= 0 ? (sibs[idx + 1] ?? null) : null;
		},
		get previousElementSibling() {
			if (!node.parentNode) {
				return null;
			}
			const sibs = node.parentNode.children;
			const idx = sibs.indexOf(node);
			return idx > 0 ? (sibs[idx - 1] ?? null) : null;
		},
	};
	return node;
}

const flushMicrotasks = () =>
	new Promise<void>((resolvePromise) => setImmediate(resolvePromise));

test("emitted lazy-search script retries failed fetches, discards cleared queries, renders safe DOM teasers, and highlights consecutive text nodes", async () => {
	const dir = createFakeBook({ hashed: true });
	const stats = optimizeBook(dir);
	assert.ok(stats.searchJsFile && stats.searchDataFile);

	const searchScript = readFileSync(join(dir, stats.searchJsFile), "utf8");
	const searchDataJson = JSON.parse(
		readFileSync(join(dir, stats.searchDataFile), "utf8"),
	);

	const wrap = makeStubNode("div", {
		id: "mdbook-search-wrapper",
		classes: ["hidden"],
	});
	const outer = makeStubNode("form", { id: "mdbook-searchbar-outer" });
	const bar = makeStubNode("input", { id: "mdbook-searchbar" });
	const list = makeStubNode("ul", { id: "mdbook-searchresults" });
	const listOuter = makeStubNode("div", {
		id: "mdbook-searchresults-outer",
		classes: ["hidden"],
	});
	const header = makeStubNode("div", { id: "mdbook-searchresults-header" });
	const toggle = makeStubNode("button", { id: "mdbook-search-toggle" });
	const content = makeStubNode("main", { id: "mdbook-content" });
	const p1 = makeStubNode("p");
	p1.appendChild(makeStubNode("#text", { nodeType: 3, text: "Pina" }));
	const p2 = makeStubNode("p");
	p2.appendChild(makeStubNode("#text", { nodeType: 3, text: "Pina" }));
	content.appendChild(p1);
	content.appendChild(p2);

	const byId = new Map<string, StubNode>([
		[wrap.id, wrap],
		[outer.id, outer],
		[bar.id, bar],
		[list.id, list],
		[listOuter.id, listOuter],
		[header.id, header],
		[toggle.id, toggle],
		[content.id, content],
	]);
	const docListeners = new Map<string, Array<(e: unknown) => void>>();

	let fetchCalls = 0;
	let deferredReject = null as ((err: Error) => void) | null;
	let deferredResolve = null as
		| ((val: { ok: boolean; json: () => Promise<unknown> }) => void)
		| null;

	const fetchStub = () => {
		fetchCalls += 1;
		if (fetchCalls === 1) {
			return Promise.reject(new Error("offline"));
		}
		if (fetchCalls === 2) {
			return new Promise((_resolvePromise, rejectPromise) => {
				deferredReject = rejectPromise;
			});
		}
		if (fetchCalls === 3) {
			return new Promise((resolvePromise) => {
				deferredResolve = resolvePromise;
			});
		}
		return Promise.resolve({
			ok: true,
			json: () => Promise.resolve(searchDataJson),
		});
	};

	const documentStub = {
		title: "Docs",
		activeElement: null,
		getElementById: (id: string) => byId.get(id) ?? null,
		createElement: (tag: string) => makeStubNode(tag),
		createTextNode: (text: string) =>
			makeStubNode("#text", { nodeType: 3, text }),
		createDocumentFragment: () => makeStubNode("#fragment", { nodeType: 11 }),
		createTreeWalker(
			rootNode: StubNode,
			_whatToShow: number,
			filter: { acceptNode: (n: StubNode) => number },
		) {
			const accepted: StubNode[] = [];
			const walk = (n: StubNode) => {
				for (const c of n.childNodes) {
					if (c.nodeType === 3) {
						if (filter.acceptNode(c) === 1) {
							accepted.push(c);
						}
					} else {
						walk(c);
					}
				}
			};
			walk(rootNode);
			let cursor = -1;
			return {
				get currentNode() {
					return accepted[cursor] ?? null;
				},
				nextNode() {
					cursor += 1;
					return cursor < accepted.length ? accepted[cursor] : null;
				},
			};
		},
		addEventListener: (type: string, fn: (e: unknown) => void) => {
			const arr = docListeners.get(type) ?? [];
			arr.push(fn);
			docListeners.set(type, arr);
		},
	};

	const windowLocation = {
		href: "https://pina.rs/introduction.html?highlight=Pina",
		assign: () => {},
	};
	const sandbox: Record<string, unknown> = {
		path_to_root: "",
		document: documentStub,
		NodeFilter: {
			SHOW_TEXT: 4,
			FILTER_ACCEPT: 1,
			FILTER_REJECT: 2,
			FILTER_SKIP: 3,
		},
		URL,
		URLSearchParams,
		fetch: fetchStub,
		history: {
			pushState: (_s: unknown, _t: unknown, u: URL) => {
				windowLocation.href = u.href;
			},
			replaceState: (_s: unknown, _t: unknown, u: URL) => {
				windowLocation.href = u.href;
			},
		},
		location: windowLocation,
		scrollTo: () => {},
		setTimeout: () => 0,
		addEventListener: () => {},
		console: { error: () => {} },
	};
	sandbox.window = sandbox;

	vm.runInNewContext(searchScript, sandbox);

	// 1. Both consecutive text nodes containing "Pina" were highlighted via ?highlight=Pina.
	assert.equal(content.querySelectorAll("mark.pina-hl").length, 2);

	// 2. First search fails while query stays active -> shows retryable error, does not permanently poison `loading`.
	bar.value = "pina";
	bar.dispatch("input");
	await flushMicrotasks();
	assert.equal(fetchCalls, 1);
	assert.equal(listOuter.classList.contains("hidden"), false);
	assert.match(header.textContent, /Failed to load search index/);
	assert.equal(list.querySelectorAll("button").length, 1);

	// 3. Second search fails AFTER reader clears the field -> resets `loading` and keeps results hidden.
	bar.value = "pina";
	bar.dispatch("input");
	assert.equal(fetchCalls, 2);
	bar.value = "";
	bar.dispatch("input");
	assert.equal(listOuter.classList.contains("hidden"), true);
	deferredReject!(new Error("late failure"));
	await flushMicrotasks();
	assert.equal(listOuter.classList.contains("hidden"), true);
	assert.equal(list.children.length, 0);

	// 4. Third search starts in-flight fetch; clearing the field before resolution keeps results empty.
	bar.value = "pina";
	bar.dispatch("input");
	assert.equal(fetchCalls, 3);
	bar.value = "";
	bar.dispatch("input");
	assert.equal(listOuter.classList.contains("hidden"), true);
	deferredResolve!({
		ok: true,
		json: () => Promise.resolve(searchDataJson),
	});
	await flushMicrotasks();
	assert.equal(listOuter.classList.contains("hidden"), true);
	assert.equal(list.children.length, 0);

	// 5. Subsequent search reuses cached data (no 4th fetch) and renders results cleanly.
	bar.value = "pina";
	bar.dispatch("input");
	await flushMicrotasks();
	assert.equal(fetchCalls, 3);
	assert.equal(listOuter.classList.contains("hidden"), false);
	assert.match(header.textContent, /1 search result for 'pina':/);
	assert.equal(list.querySelectorAll("a")[0]?.textContent, "Intro");

	// 6. Teaser matches raw text ("amp &") before HTML escaping without corrupting `&amp;`.
	bar.value = "amp";
	bar.dispatch("input");
	await flushMicrotasks();
	const ems = list.querySelectorAll("em");
	assert.equal(ems.length, 1);
	assert.equal(ems[0].textContent, "amp");
	assert.match(list.textContent, /amp & <script>safe<\/script>/);
});

test("emitted lean book script retries failed search-script loads, keeps pending clicks on ?highlight=, and handles copyText fallback failures", async () => {
	const dir = createFakeBook({ hashed: true });
	const stats = optimizeBook(dir);
	const bookScript = readFileSync(join(dir, stats.bundleJsFile), "utf8");

	const makeBookEnv = (
		searchUrl: string,
		execCommandImpl: () => boolean = () => true,
	) => {
		const toggle = makeStubNode("button", { id: "mdbook-search-toggle" });
		const head = makeStubNode("head");
		const body = makeStubNode("body");
		const html = makeStubNode("html");
		const docListeners = new Map<string, Array<(e: unknown) => void>>();
		let openCount = 0;

		const documentStub = {
			documentElement: html,
			head,
			body,
			scrollingElement: { scrollTop: 0, scrollTo: () => {} },
			getElementById: (id: string) =>
				id === "mdbook-search-toggle" ? toggle : null,
			createElement: (tag: string) => makeStubNode(tag),
			querySelector: () => null,
			querySelectorAll: () => [],
			execCommand: execCommandImpl,
			addEventListener: (type: string, fn: (e: unknown) => void) => {
				const arr = docListeners.get(type) ?? [];
				arr.push(fn);
				docListeners.set(type, arr);
			},
			removeEventListener: (type: string, fn: (e: unknown) => void) => {
				const arr = docListeners.get(type);
				if (!arr) {
					return;
				}
				const idx = arr.indexOf(fn);
				if (idx >= 0) {
					arr.splice(idx, 1);
				}
			},
		};

		const sandbox: Record<string, unknown> = {
			path_to_root: "",
			HTMLElement: class {},
			customElements: { define: () => {} },
			document: documentStub,
			location: {
				origin: "https://pina.rs",
				href: `https://pina.rs/introduction.html${searchUrl}`,
				search: searchUrl,
			},
			navigator: {},
			URL,
			URLSearchParams,
			requestAnimationFrame: () => 0,
			window: null,
		};
		sandbox.window = sandbox;
		sandbox.search = {
			open: () => {
				openCount += 1;
			},
		};

		vm.runInNewContext(bookScript, sandbox);
		return {
			toggle,
			head,
			body,
			docListeners,
			getOpenCount: () => openCount,
			dispatchDoc: (type: string, event: Record<string, unknown>) => {
				for (const fn of [...(docListeners.get(type) ?? [])]) {
					fn({ preventDefault() {}, stopPropagation() {}, ...event });
				}
			},
		};
	};

	// 1. Failed search script load resets state so a second click retries and opens search.
	const retryEnv = makeBookEnv("");
	retryEnv.toggle.dispatch("click");
	assert.equal(retryEnv.head.children.length, 1);
	retryEnv.head.children[0].onerror!();
	assert.equal(retryEnv.head.children.length, 0);

	retryEnv.toggle.dispatch("click");
	assert.equal(retryEnv.head.children.length, 1);
	retryEnv.head.children[0].onload!();
	assert.equal(retryEnv.getOpenCount(), 1);

	// 2. On `?highlight=` page, a Search click while script is loading opens panel on load.
	const hlClickEnv = makeBookEnv("?highlight=pina");
	assert.equal(hlClickEnv.head.children.length, 1);
	hlClickEnv.toggle.dispatch("click");
	assert.equal(hlClickEnv.head.children.length, 1);
	hlClickEnv.head.children[0].onload!();
	assert.equal(hlClickEnv.getOpenCount(), 1);

	// 3. On `?highlight=` page with no click, panel remains closed on load.
	const hlIdleEnv = makeBookEnv("?highlight=pina");
	assert.equal(hlIdleEnv.head.children.length, 1);
	hlIdleEnv.head.children[0].onload!();
	assert.equal(hlIdleEnv.getOpenCount(), 0);

	// 4. copyText fallback treats execCommand("copy") === false as error and cleans up textarea on throw.
	for (
		const impl of [
			() => false,
			() => {
				throw new Error("boom");
			},
		]
	) {
		const copyEnv = makeBookEnv("", impl);
		const pre = makeStubNode("pre");
		const code = makeStubNode("code");
		code.innerText = "let x = 1;";
		const clip = makeStubNode("button", { classes: ["clip-button"] });
		const tooltip = makeStubNode("i", { classes: ["tooltiptext"] });
		clip.appendChild(tooltip);
		pre.appendChild(clip);
		pre.appendChild(code);

		copyEnv.dispatchDoc("click", { target: clip });
		await flushMicrotasks();
		assert.equal(tooltip.textContent, "Clipboard error!");
		assert.equal(copyEnv.body.children.length, 0);
	}
});

test("optimizeBook preserves cascade order for mixed additional-css = [custom.css, theme/override.css] and excludes @import theme sheets", () => {
	const dir = mkdtempSync(join(tmpdir(), "pina-opt-cascade-"));
	mkdirSync(join(dir, "css"));
	mkdirSync(join(dir, "theme"));

	writeFileSync(join(dir, "css/variables.css"), ":root { --bg: #fff; }");
	writeFileSync(join(dir, "css/general.css"), "body { color: #111; }");
	writeFileSync(join(dir, "theme/base.css"), ".banner { color: gray; }");
	writeFileSync(
		join(dir, "theme/with-import.css"),
		`@import url("colors.css");\n.imported { color: purple; }`,
	);
	writeFileSync(join(dir, "custom.css"), ".banner { color: red; }");
	writeFileSync(join(dir, "theme/override.css"), ".banner { color: blue; }");
	writeFileSync(join(dir, "book.js"), "// book");
	writeFileSync(
		join(dir, "index.html"),
		`<!DOCTYPE html><html><head>
<link rel="stylesheet" href="css/variables.css">
<link rel="stylesheet" href="css/general.css">
<link rel="stylesheet" href="theme/base.css">
<link rel="stylesheet" href="custom.css">
<link rel="stylesheet" href="theme/override.css">
<link rel="stylesheet" href="theme/with-import.css">
</head><body><div id="mdbook-body-container"><main><div class="banner">Hi</div></main><script src="book.js"></script></div></body></html>`,
	);

	const stats = optimizeBook(dir);
	const html = readFileSync(join(dir, "index.html"), "utf8");

	// Extract every linked stylesheet in final HTML document order and compute
	// the last equal-specificity `.banner { color: ... }` rule to verify the
	// later `theme/override.css` sheet still wins over `custom.css`.
	const linkedHrefs = Array.from(
		html.matchAll(/<link\b[^>]*rel="stylesheet"[^>]*href="([^"]+)"/gi),
		(m) => m[1],
	);
	assert.deepEqual(linkedHrefs, [
		stats.bundleCssFile,
		"custom.css",
		"theme/override.css",
		"theme/with-import.css",
	]);

	let winningBannerColor = "";
	for (const href of linkedHrefs) {
		const css = readFileSync(join(dir, href), "utf8");
		for (const m of css.matchAll(/\.banner\s*\{\s*color\s*:\s*([^;}\s]+)/g)) {
			winningBannerColor = m[1];
		}
	}
	assert.equal(winningBannerColor, "blue");

	// Contiguous ordinary `theme/base.css` was bundled and removed; `@import` and
	// post-`custom.css` sheets stayed linked and on disk.
	assert.equal(existsSync(join(dir, "theme/base.css")), false);
	assert.equal(existsSync(join(dir, "custom.css")), true);
	assert.equal(existsSync(join(dir, "theme/override.css")), true);
	assert.equal(existsSync(join(dir, "theme/with-import.css")), true);
	assert.doesNotMatch(
		readFileSync(join(dir, stats.bundleCssFile), "utf8"),
		/@import/,
	);
});

test("optimizeBook rewrites unquoted stylesheet links and keeps superseded assets still referenced by any page", () => {
	const dir = mkdtempSync(join(tmpdir(), "pina-opt-unquoted-"));
	mkdirSync(join(dir, "css"));

	writeFileSync(join(dir, "css/variables.css"), ":root { --bg: #fff; }");
	writeFileSync(join(dir, "css/general.css"), "body { color: #111; }");
	writeFileSync(join(dir, "book.js"), "// book");

	// Chapter uses unquoted `<link rel=stylesheet href="css/general.css">` and `<link rel=stylesheet href=css/variables.css>`.
	writeFileSync(
		join(dir, "index.html"),
		`<!DOCTYPE html><html><head>
<link rel=stylesheet href=css/variables.css>
<link rel=stylesheet href="css/general.css">
</head><body><div id="mdbook-body-container"><main>Hello</main><script defer src=book.js></script></div></body></html>`,
	);
	// Standalone page still links directly to `css/general.css`.
	writeFileSync(
		join(dir, "standalone.html"),
		`<!DOCTYPE html><html><head><link rel=stylesheet href="css/general.css"></head><body><main>Standalone</main></body></html>`,
	);

	const first = optimizeBook(dir);
	const chapterHtml = readFileSync(join(dir, "index.html"), "utf8");
	assert.match(chapterHtml, new RegExp(first.bundleCssFile));
	assert.doesNotMatch(chapterHtml, /css\/variables\.css|css\/general\.css/);
	assert.match(chapterHtml, new RegExp(`defer src="${first.bundleJsFile}"`));

	// `css/variables.css` and `book.js` have zero remaining references -> deleted.
	assert.equal(existsSync(join(dir, "css/variables.css")), false);
	assert.equal(existsSync(join(dir, "book.js")), false);
	// `css/general.css` is still referenced by `standalone.html` -> preserved!
	assert.equal(existsSync(join(dir, "css/general.css")), true);

	// Second run remains idempotent even with `css/general.css` kept on disk.
	const second = optimizeBook(dir);
	assert.equal(second.htmlFilesOptimized, 0);
	assert.equal(second.bundleCssFile, first.bundleCssFile);
	assert.equal(readFileSync(join(dir, "index.html"), "utf8"), chapterHtml);
});
