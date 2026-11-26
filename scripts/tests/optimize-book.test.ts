import assert from "node:assert/strict";
import {
	existsSync,
	mkdirSync,
	mkdtempSync,
	readFileSync,
	writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import {
	buildCompactSearchData,
	highlightAndEnhanceCodeBlocks,
	minifyCss,
	minifyHtml,
	minifyJs,
	optimizeBook,
	rebaseCssUrls,
} from "../docs/optimize-book.ts";

test("minifyCss strips comments and preserves strings, calc, and descendant pseudo-classes", () => {
	const css = `
		/* comment */
		pre > .buttons :hover {
			width: calc(100% - 12px);
			content: url('data:image/svg+xml,<svg \\
viewBox="0 0 10 10"></svg>');
		}
	`;
	const out = minifyCss(css);
	assert.equal(
		out,
		`pre>.buttons :hover{width:calc(100% - 12px);content:url('data:image/svg+xml,<svg viewBox="0 0 10 10"></svg>')}`,
	);
});

test("minifyJs preserves regex literals, strings, and template literals", () => {
	const js = `
		// line comment
		const url = "https://pina.rs/docs";
		/* block
		   comment */
		const re = /https?:\\/\\//g;
		const msg = \`hello
world\`;
		function add(a, b) {
			return a + b;
		}
	`;
	const out = minifyJs(js);
	assert.match(out, /"https:\/\/pina\.rs\/docs"/);
	assert.match(out, /\/https\?:\\\/\\\/\/g/);
	assert.match(out, /`hello\nworld`/);
	assert.doesNotMatch(out, /line comment|block/);
});

test("minifyHtml preserves pre/code whitespace and minifies inline style/script", () => {
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
			<p>Hello   <strong>world</strong>  and  <a href="/">links</a></p>
			<pre><code>  keep   spaces\n  here</code></pre>
		</body>
	</html>`;
	const out = minifyHtml(html);
	assert.doesNotMatch(out, /strip me/);
	assert.match(out, /<style>body\{margin:0\}<\/style>/);
	assert.match(out, /<pre><code>  keep   spaces\n  here<\/code><\/pre>/);
	assert.match(out, /<strong>world<\/strong> and <a href="\/">links<\/a>/);
});

test("rebaseCssUrls adjusts relative paths without touching data or absolute URLs", () => {
	const css =
		`src: url('../fonts/a.woff2'), url('data:image/svg+xml,foo'), url('https://x/y');`;
	assert.equal(
		rebaseCssUrls(css, "fonts/fonts-12345678.css", "assets/book.css"),
		`src: url('../fonts/a.woff2'), url('data:image/svg+xml,foo'), url('https://x/y');`,
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

test("optimizeBook bundles CSS, removes obsolete scripts, and builds compact search", () => {
	const dir = mkdtempSync(join(tmpdir(), "pina-opt-book-"));
	mkdirSync(join(dir, "css"));
	mkdirSync(join(dir, "fonts"));
	mkdirSync(join(dir, "theme"));

	writeFileSync(
		join(dir, "css/variables-11111111.css"),
		":root { --bg: #fff; }",
	);
	writeFileSync(join(dir, "css/general-22222222.css"), "body { color: #111; }");
	writeFileSync(join(dir, "css/chrome-33333333.css"), ".page { padding: 0; }");
	writeFileSync(
		join(dir, "css/print-44444444.css"),
		".sidebar { display: none; }",
	);
	writeFileSync(
		join(dir, "fonts/fonts-55555555.css"),
		"@font-face { src: url('../fonts/a.woff2'); }",
	);
	writeFileSync(join(dir, "highlight-66666666.css"), ".hljs { color: #000; }");
	writeFileSync(
		join(dir, "tomorrow-night-77777777.css"),
		".hljs { color: #fff; }",
	);
	writeFileSync(
		join(dir, "ayu-highlight-88888888.css"),
		".hljs { color: #eee; }",
	);
	writeFileSync(
		join(dir, "theme/pina-99999999.css"),
		"html .hljs { color: #e8e4d4; }",
	);
	writeFileSync(join(dir, "theme/pina-aaaaaaaa.js"), "(function(){})();");
	writeFileSync(
		join(dir, "highlight-bbbbbbbb.js"),
		`window.hljs = { configure(){}, getLanguage(l){ return l === "rust"; }, highlight(l, c){ return { value: c.replace(/let/g, '<span class="hljs-keyword">let</span>') }; } };`,
	);
	writeFileSync(join(dir, "book-cccccccc.js"), "// original book js");
	writeFileSync(join(dir, "clipboard-dddddddd.min.js"), "// clipboard");
	writeFileSync(join(dir, "elasticlunr-eeeeeeee.min.js"), "// elasticlunr");
	writeFileSync(join(dir, "mark-ffffffff.min.js"), "// mark");
	writeFileSync(join(dir, "searcher-12345678.js"), "// searcher");
	writeFileSync(
		join(dir, "toc-87654321.js"),
		`class T extends HTMLElement { connectedCallback() { this.innerHTML = '<ol></ol>'; // Track and set sidebar scroll position\n } }`,
	);
	const payload = JSON.stringify({
		doc_urls: ["introduction.html#intro"],
		index: {
			documentStore: {
				docs: {
					"0": { breadcrumbs: "Intro", body: "Pina framework docs" },
				},
			},
		},
	});
	writeFileSync(
		join(dir, "searchindex-abcdef12.js"),
		`window.search = Object.assign(window.search, JSON.parse(${
			JSON.stringify(payload)
		}));`,
	);
	writeFileSync(
		join(dir, "introduction.html"),
		`<!DOCTYPE HTML>
<html>
<head>
<link rel="stylesheet" href="css/variables-11111111.css">
<link rel="stylesheet" href="css/general-22222222.css">
<link rel="stylesheet" href="css/chrome-33333333.css">
<link rel="stylesheet" href="css/print-44444444.css" media="print">
<link rel="stylesheet" href="fonts/fonts-55555555.css">
<link rel="stylesheet" id="mdbook-highlight-css" href="highlight-66666666.css">
<link rel="stylesheet" id="mdbook-tomorrow-night-css" href="tomorrow-night-77777777.css">
<link rel="stylesheet" id="mdbook-ayu-highlight-css" href="ayu-highlight-88888888.css">
<link rel="stylesheet" href="theme/pina-99999999.css">
<script>const path_to_root = ""; window.path_to_searchindex_js = "searchindex-abcdef12.js";</script>
<script src="toc-87654321.js"></script>
</head>
<body>
<div id="mdbook-body-container">
<h1 class="menu-title">Pina</h1>
<button id="mdbook-theme-navy">Navy</button>
<main><pre><code class="language-rust">let x = 1;</code></pre></main>
<template id=fa-play>play</template>
<script>window.playground_copyable = true;</script>
<script src="elasticlunr-eeeeeeee.min.js"></script>
<script src="mark-ffffffff.min.js"></script>
<script src="searcher-12345678.js"></script>
<script src="clipboard-dddddddd.min.js"></script>
<script src="highlight-bbbbbbbb.js"></script>
<script src="book-cccccccc.js"></script>
<script src="theme/pina-aaaaaaaa.js"></script>
</div>
</body>
</html>`,
	);

	const stats = optimizeBook(dir);
	assert.equal(stats.htmlFilesOptimized, 1);
	assert.ok(stats.searchDataFile);
	assert.ok(stats.searchJsFile);

	const outHtml = readFileSync(join(dir, "introduction.html"), "utf8");
	assert.match(outHtml, new RegExp(stats.bundleCssFile));
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

	assert.equal(existsSync(join(dir, "highlight-bbbbbbbb.js")), false);
	assert.equal(existsSync(join(dir, "toc-87654321.js")), false);
	assert.equal(existsSync(join(dir, "searchindex-abcdef12.js")), false);
	assert.equal(existsSync(join(dir, stats.bundleCssFile)), true);
	assert.equal(existsSync(join(dir, stats.bundleJsFile)), true);
	assert.equal(existsSync(join(dir, stats.searchDataFile)), true);
	assert.equal(existsSync(join(dir, stats.searchJsFile)), true);
});
