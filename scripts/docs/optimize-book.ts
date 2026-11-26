#!/usr/bin/env node

// Post-processes a built mdBook directory to minimize transfer bytes, HTTP
// requests, main-thread JavaScript work, and layout shift while preserving
// every interactive feature (theme switching, sidebar, keyboard navigation,
// code copy, hidden-line toggles, and full-text search).
//
// Usage:
//
//   node scripts/docs/optimize-book.ts <bookDir>

import { createHash } from "node:crypto";
import {
	existsSync,
	mkdirSync,
	readdirSync,
	readFileSync,
	rmSync,
	unlinkSync,
	writeFileSync,
} from "node:fs";
import { join, posix, relative, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import vm from "node:vm";

const PINA_LOGO_SVG =
	'<svg viewBox="0 0 320 320" aria-hidden="true" focusable="false"><g fill="currentColor" fill-rule="evenodd" transform="translate(-492.8 -69.9) scale(0.85 0.62)"><path d="M768 152 L793 205 Q795 210 792 217 L768 276 L744 217 Q741 210 744 204 Z"/><path d="M677 209 L721 223 Q730 226 734 235 L752 281 L708 267 Q698 264 694 254 Z"/><path d="M859 209 L815 223 Q806 226 802 235 L784 281 L828 267 Q838 264 842 254 Z"/><path d="M782 294 L724 294 Q706 292 693 306 L662 338 Q641 360 657 384 L676 404 Z"/><path d="M687 415 L793 304 Q816 281 837 307 L875 344 Q896 367 878 389 L862 405 L802 344 Q790 339 778 342 L875 439 Q897 463 878 494 L873 504 L768 397 L720 447 Z"/><path d="M669 418 L751 501 L716 537 L662 484 Q635 456 658 428 Z"/><path d="M760 581 L727 548 L810 464 L844 498 Z"/><path d="M661 505 L742 585 L719 585 Q690 584 670 563 Q645 545 655 521 Z"/><path d="M854 509 L869 524 Q880 546 864 565 Q850 585 822 586 Q803 588 787 579 Z"/></g></svg>';

const PINA_FOOTER_HTML =
	'<footer class="pina-footer"><span>Pina · Apache-2.0 · Built with mdBook</span><nav aria-label="Project links"><a href="https://github.com/pina-rs/pina">GitHub</a><a href="https://crates.io/crates/pina">crates.io</a><a href="https://docs.rs/pina/">API reference</a><a href="https://github.com/pina-rs/pina/blob/main/SECURITY.md">Security</a></nav></footer>';

const COPY_BUTTON_HTML =
	'<button class="clip-button" type="button" title="Copy to clipboard" aria-label="Copy to clipboard"><i class="tooltiptext"></i></button>';

const FA_EYE_SVG =
	'<span class="fa-svg"><svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 576 512"><path d="M288 32c-80.8 0-145.5 36.8-192.6 80.6C48.6 156 17.3 208 2.5 243.7c-3.3 7.9-3.3 16.7 0 24.6C17.3 304 48.6 356 95.4 399.4C142.5 443.2 207.2 480 288 480s145.5-36.8 192.6-80.6c46.8-43.5 78.1-95.4 93-131.1c3.3-7.9 3.3-16.7 0-24.6c-14.9-35.7-46.2-87.7-93-131.1C433.5 68.8 368.8 32 288 32zM432 256c0 79.5-64.5 144-144 144s-144-64.5-144-144s64.5-144 144-144s144 64.5 144 144zM288 192c0 35.3-28.7 64-64 64c-11.5 0-22.3-3-31.6-8.4c-.2 2.8-.4 5.5-.4 8.4c0 53 43 96 96 96s96-43 96-96s-43-96-96-96c-2.8 0-5.6 .1-8.4 .4c5.3 9.3 8.4 20.1 8.4 31.6z"/></svg></span>';

const EYE_BUTTON_HTML =
	`<button class="boring-toggle" type="button" title="Show hidden lines" aria-label="Show hidden lines">${FA_EYE_SVG}</button>`;

function shortHash(content: string): string {
	return createHash("sha256").update(content).digest("hex").slice(0, 8);
}

function isCssHexDigit(ch: string | undefined): boolean {
	return ch !== undefined && /^[0-9a-fA-F]$/.test(ch);
}

function isCssWhitespace(ch: string | undefined): boolean {
	return ch === " " || ch === "\t" || ch === "\n" || ch === "\r" || ch === "\f";
}

function isValidJsScript(code: string): boolean {
	try {
		new vm.Script(code);
		return true;
	} catch {
		return false;
	}
}

// ---------------------------------------------------------------------------
// Lossless minifiers (CSS, JS, HTML)
// ---------------------------------------------------------------------------

/**
 * Minifies CSS while preserving string literals, CSS backslash-newline string
 * continuations (used in mdBook's inline SVG data URIs), hexadecimal escape
 * terminators (e.g. `.\31  .item`), `calc()` spacing, and descendant
 * combinators before pseudo-classes (e.g. `.buttons :hover`).
 */
export function minifyCss(input: string): string {
	const out: string[] = [];
	const len = input.length;
	let i = 0;
	let pendingSpace = false;

	const emitSpaceIfNeeded = (nextChar: string): void => {
		if (!pendingSpace) {
			return;
		}
		pendingSpace = false;
		const prev = out.length > 0 ? out[out.length - 1] : "";
		if (!prev) {
			return;
		}
		// Never strip space before `:` (preserves descendant `:hover` selectors),
		// nor around `+`/`-` inside `calc(...)`, nor before `(` after keywords.
		if (
			prev === "{" ||
			prev === "}" ||
			prev === ";" ||
			prev === "," ||
			prev === ">" ||
			prev === "~" ||
			prev === ":" ||
			prev === "("
		) {
			return;
		}
		if (
			nextChar === "{" ||
			nextChar === "}" ||
			nextChar === ";" ||
			nextChar === "," ||
			nextChar === ">" ||
			nextChar === "~" ||
			nextChar === ")"
		) {
			return;
		}
		out.push(" ");
	};

	while (i < len) {
		const ch = input[i];

		// Block comments
		if (ch === "/" && input[i + 1] === "*") {
			i += 2;
			while (i < len && !(input[i] === "*" && input[i + 1] === "/")) {
				i += 1;
			}
			i += 2;
			continue;
		}

		// Quoted strings (including CSS `\<newline>` continuations)
		if (ch === '"' || ch === "'") {
			emitSpaceIfNeeded(ch);
			const quote = ch;
			out.push(ch);
			i += 1;
			while (i < len) {
				const c = input[i];
				if (c === "\\") {
					if (input[i + 1] === "\r" && input[i + 2] === "\n") {
						i += 3;
						continue;
					}
					if (input[i + 1] === "\n") {
						i += 2;
						continue;
					}
					out.push(c);
					if (i + 1 < len) {
						out.push(input[i + 1]);
					}
					i += 2;
					continue;
				}
				out.push(c);
				i += 1;
				if (c === quote) {
					break;
				}
			}
			continue;
		}

		// CSS escapes outside strings: preserve the single whitespace consumed by
		// a hex escape (`\31 `) separately from any following selector whitespace
		// so `.\31  .item` retains its descendant combinator.
		if (ch === "\\") {
			emitSpaceIfNeeded(ch);
			out.push(ch);
			i += 1;
			if (i < len && isCssHexDigit(input[i])) {
				let hexCount = 0;
				while (i < len && hexCount < 6 && isCssHexDigit(input[i])) {
					out.push(input[i]);
					i += 1;
					hexCount += 1;
				}
				if (i < len && isCssWhitespace(input[i])) {
					if (input[i] === "\r" && input[i + 1] === "\n") {
						i += 2;
					} else {
						i += 1;
					}
					out.push(" ");
				}
			} else if (i < len) {
				out.push(input[i]);
				i += 1;
			}
			continue;
		}

		// Whitespace
		if (isCssWhitespace(ch)) {
			pendingSpace = true;
			i += 1;
			continue;
		}

		// Drop redundant semicolon before closing brace
		if (ch === "}" && out.length > 0 && out[out.length - 1] === ";") {
			out.pop();
		}

		emitSpaceIfNeeded(ch);
		out.push(ch);
		i += 1;
	}

	return out.join("").trim();
}

/**
 * Conservatively minifies JavaScript: strips comments outside string, template,
 * and regex literals, collapses indentation/blank lines, preserves newlines
 * where ASI could otherwise change meaning, and falls back to the original
 * source if the minified output ever fails syntax validation.
 */
export function minifyJs(input: string): string {
	const lines: string[] = [];
	let cur: string[] = [];
	const len = input.length;
	let i = 0;
	let lastNonWs = "";
	let lastWord = "";
	let afterControlParen = false;
	const parenStack: boolean[] = [];

	const flushLine = (): void => {
		const trimmed = cur.join("").trim();
		if (trimmed.length > 0) {
			lines.push(trimmed);
		}
		cur = [];
	};

	const REGEX_PREV_WORDS = new Set([
		"return",
		"case",
		"throw",
		"in",
		"instanceof",
		"typeof",
		"void",
		"delete",
		"new",
		"do",
		"else",
		"yield",
		"await",
		"of",
	]);

	const CONTROL_FLOW_WORDS = new Set(["if", "while", "for", "with"]);

	const canStartRegex = (): boolean => {
		if (afterControlParen || !lastNonWs) {
			return true;
		}
		if ("([{:;,=!?&|+-*%^~<>".includes(lastNonWs)) {
			return true;
		}
		if (lastNonWs === "\n") {
			return true;
		}
		return REGEX_PREV_WORDS.has(lastWord);
	};

	while (i < len) {
		const ch = input[i];

		// Line comment
		if (ch === "/" && input[i + 1] === "/") {
			i += 2;
			while (i < len && input[i] !== "\n") {
				i += 1;
			}
			continue;
		}

		// Block comment
		if (ch === "/" && input[i + 1] === "*") {
			let j = i + 2;
			let hadNewline = false;
			while (j < len && !(input[j] === "*" && input[j + 1] === "/")) {
				if (input[j] === "\n") {
					hadNewline = true;
				}
				j += 1;
			}
			i = j + 2;
			if (hadNewline) {
				flushLine();
				lastNonWs = "\n";
			} else {
				cur.push(" ");
			}
			continue;
		}

		// Single or double quoted string
		if (ch === '"' || ch === "'") {
			const quote = ch;
			cur.push(ch);
			i += 1;
			while (i < len) {
				const c = input[i];
				cur.push(c);
				i += 1;
				if (c === "\\" && i < len) {
					cur.push(input[i]);
					i += 1;
					continue;
				}
				if (c === quote) {
					break;
				}
			}
			lastNonWs = quote;
			lastWord = "";
			afterControlParen = false;
			continue;
		}

		// Template literal (preserving embedded newlines and nested `${...}` expressions)
		if (ch === "`") {
			cur.push(ch);
			i += 1;
			let exprDepth = 0;
			let exprQuote = "";
			while (i < len) {
				const c = input[i];
				cur.push(c);
				i += 1;
				if (c === "\\" && i < len) {
					cur.push(input[i]);
					i += 1;
					continue;
				}
				if (exprDepth > 0) {
					if (exprQuote) {
						if (c === exprQuote) {
							exprQuote = "";
						}
						continue;
					}
					if (c === '"' || c === "'" || c === "`") {
						exprQuote = c;
						continue;
					}
					if (c === "{") {
						exprDepth += 1;
					} else if (c === "}") {
						exprDepth -= 1;
					}
					continue;
				}
				if (c === "$" && input[i] === "{") {
					cur.push("{");
					i += 1;
					exprDepth += 1;
					continue;
				}
				if (c === "`") {
					break;
				}
			}
			lastNonWs = "`";
			lastWord = "";
			afterControlParen = false;
			continue;
		}

		// Regex literal
		if (ch === "/" && canStartRegex()) {
			cur.push(ch);
			i += 1;
			let inClass = false;
			while (i < len) {
				const c = input[i];
				cur.push(c);
				i += 1;
				if (c === "\\" && i < len) {
					cur.push(input[i]);
					i += 1;
					continue;
				}
				if (c === "[") {
					inClass = true;
				} else if (c === "]") {
					inClass = false;
				} else if (c === "/" && !inClass) {
					break;
				} else if (c === "\n") {
					break;
				}
			}
			while (i < len && /[a-z]/i.test(input[i])) {
				cur.push(input[i]);
				i += 1;
			}
			lastNonWs = "/";
			lastWord = "";
			afterControlParen = false;
			continue;
		}

		// Identifier / keyword tracking for regex and control-flow disambiguation
		if (/[A-Za-z_$]/.test(ch)) {
			let j = i + 1;
			while (j < len && /[A-Za-z0-9_$]/.test(input[j])) {
				j += 1;
			}
			const word = input.slice(i, j);
			cur.push(word);
			lastWord = word;
			lastNonWs = word[word.length - 1];
			afterControlParen = false;
			i = j;
			continue;
		}

		if (ch === "\n") {
			flushLine();
			lastNonWs = "\n";
			i += 1;
			continue;
		}

		if (ch === "(") {
			parenStack.push(CONTROL_FLOW_WORDS.has(lastWord));
			afterControlParen = false;
			cur.push(ch);
			lastNonWs = ch;
			lastWord = "";
			i += 1;
			continue;
		}

		if (ch === ")") {
			afterControlParen = parenStack.pop() ?? false;
			cur.push(ch);
			lastNonWs = ch;
			lastWord = "";
			i += 1;
			continue;
		}

		cur.push(ch);
		if (ch !== " " && ch !== "\t" && ch !== "\r") {
			lastNonWs = ch;
			lastWord = "";
			afterControlParen = false;
		}
		i += 1;
	}

	flushLine();

	// Join lines cleanly when the previous line ends with a token that cannot trigger ASI
	const joined: string[] = [];
	for (const line of lines) {
		if (joined.length === 0) {
			joined.push(line);
			continue;
		}
		const prev = joined[joined.length - 1];
		const last = prev[prev.length - 1];
		if (last === ";" || last === "{" || last === "," || last === "(") {
			joined[joined.length - 1] = `${prev}${line}`;
		} else {
			joined.push(line);
		}
	}

	const result = joined.join("\n");
	if (isValidJsScript(input) && !isValidJsScript(result)) {
		return input;
	}
	return result;
}

const STRUCTURAL_TAG_RE =
	/^\/?(?:html|head|body|meta|link|title|base|style|script|header|nav|main|section|article|aside|footer|div|ul|ol|li|dl|dt|dd|table|thead|tbody|tfoot|tr|th|td|caption|colgroup|col|h[1-6]|p|blockquote|figure|figcaption|details|summary|form|fieldset|legend|hr|template|noscript|iframe|svg|g|defs|symbol|path|circle|rect|line|polygon|polyline|mdbook-sidebar-scrollbox)\b/i;

/**
 * Collapses whitespace in text nodes and between tag attributes without ever
 * modifying whitespace inside quoted HTML attribute values.
 */
function collapseHtmlWhitespaceOutsideAttributes(html: string): string {
	const out: string[] = [];
	const len = html.length;
	let i = 0;

	while (i < len) {
		if (html[i] === "<") {
			let quote = "";
			let tagPendingWs = false;
			out.push("<");
			i += 1;
			while (i < len) {
				const ch = html[i];
				if (quote) {
					out.push(ch);
					i += 1;
					if (ch === quote) {
						quote = "";
					}
					continue;
				}
				if (ch === '"' || ch === "'") {
					if (tagPendingWs) {
						out.push(" ");
						tagPendingWs = false;
					}
					quote = ch;
					out.push(ch);
					i += 1;
					continue;
				}
				if (ch === ">") {
					tagPendingWs = false;
					out.push(ch);
					i += 1;
					break;
				}
				if (ch === " " || ch === "\t" || ch === "\r" || ch === "\n") {
					tagPendingWs = true;
					i += 1;
					continue;
				}
				if (tagPendingWs) {
					out.push(" ");
					tagPendingWs = false;
				}
				out.push(ch);
				i += 1;
			}
			continue;
		}

		const nextTag = html.indexOf("<", i);
		const end = nextTag === -1 ? len : nextTag;
		const text = html
			.slice(i, end)
			.replace(/[ \t]*[\r\n]+[ \t]*/g, " ")
			.replace(/[ \t]{2,}/g, " ");
		out.push(text);
		i = end;
	}

	return out.join("");
}

function extractAndMinifyRawBlocks(input: string): {
	withPlaceholders: string;
	rawBlocks: string[];
} {
	const rawBlocks: string[] = [];
	const placeholder = (index: number) => `\u0000RAW${index}\u0000`;

	const withPlaceholders = input.replace(
		/<(pre|code|textarea|script|style)(\b[^>]*)>([\s\S]*?)<\/\1>/gi,
		(full, tag: string, attrs: string, body: string) => {
			const lower = tag.toLowerCase();
			let replacement = full;
			if (lower === "style") {
				replacement = `<style${attrs}>${minifyCss(body)}</style>`;
			} else if (
				lower === "script" &&
				!/\btype\s*=\s*["']?(?:application\/(?:ld\+)?json|module)/i.test(
					attrs,
				) &&
				!/\bsrc\s*=/i.test(attrs) &&
				body.trim().length > 0
			) {
				replacement = `<script${attrs}>${minifyJs(body)}</script>`;
			}
			const idx = rawBlocks.length;
			rawBlocks.push(replacement);
			return placeholder(idx);
		},
	);

	return { withPlaceholders, rawBlocks };
}

function restoreRawBlocks(html: string, rawBlocks: readonly string[]): string {
	return html.replace(/\u0000RAW(\d+)\u0000/g, (_m, idx: string) => {
		return rawBlocks[Number(idx)] ?? "";
	});
}

/**
 * Minifies standalone non-mdBook HTML pages conservatively: minifies inline
 * `<style>`/`<script>` blocks and strips HTML comments while preserving body
 * whitespace so custom `white-space: pre` containers (such as `.cmd` and
 * `.tree` in `flow-interactive.html`) stay intact.
 */
export function minifyStandaloneHtml(input: string): string {
	const { withPlaceholders, rawBlocks } = extractAndMinifyRawBlocks(input);
	const out = withPlaceholders.replace(/<!--(?!\[if)[\s\S]*?-->/g, "");
	return restoreRawBlocks(out, rawBlocks).trim();
}

/**
 * Minifies HTML while preserving whitespace inside `<pre>`, `<code>`,
 * `<textarea>`, `<script>`, `<style>`, and quoted HTML attributes, and
 * minifying inline CSS/JS.
 */
export function minifyHtml(input: string): string {
	const { withPlaceholders, rawBlocks } = extractAndMinifyRawBlocks(input);

	// Strip HTML comments (keep conditional/IE comments if any exist).
	let out = withPlaceholders.replace(/<!--(?!\[if)[\s\S]*?-->/g, "");

	// Collapse whitespace between tags when at least one side is structural.
	out = out.replace(
		/>[ \t\r\n]+<(\/?[a-zA-Z0-9-]+)/g,
		(_match, nextTag: string) => {
			if (STRUCTURAL_TAG_RE.test(nextTag)) {
				return `><${nextTag}`;
			}
			return `> <${nextTag}`;
		},
	);

	// Also collapse whitespace directly after closing structural tags before text or inline tags.
	out = out.replace(
		/<(\/?(?:html|head|body|header|nav|main|section|article|aside|footer|div|ul|ol|li|table|thead|tbody|tr|th|td|h[1-6]|p|blockquote|details|summary|form|template|noscript|svg|g|defs|symbol)\b[^>]*)>[ \t\r\n]+/gi,
		"<$1>",
	);
	out = out.replace(
		/[ \t\r\n]+<(\/(?:html|head|body|header|nav|main|section|article|aside|footer|div|ul|ol|li|table|thead|tbody|tr|th|td|h[1-6]|p|blockquote|details|summary|form|template|noscript|svg|g|defs|symbol))>/gi,
		"<$1>",
	);

	// Collapse runs of whitespace in text nodes and tag delimiters without touching quoted attributes.
	out = collapseHtmlWhitespaceOutsideAttributes(out);

	return restoreRawBlocks(out, rawBlocks).trim();
}

// ---------------------------------------------------------------------------
// Build-time syntax highlighting (using mdBook's bundled highlight.js)
// ---------------------------------------------------------------------------

interface HljsLike {
	configure?: (opts: Record<string, unknown>) => void;
	getLanguage: (name: string) => unknown;
	highlight: (
		lang: string,
		code: string,
		ignoreIllegals?: boolean,
	) => { value: string };
}

function loadHljs(bookDir: string, highlightFile: string): HljsLike {
	const src = readFileSync(join(bookDir, highlightFile), "utf8");
	const sandbox: Record<string, unknown> = { console };
	sandbox.window = sandbox;
	sandbox.self = sandbox;
	vm.runInNewContext(src, sandbox);
	const hljs = sandbox.hljs as HljsLike | undefined;
	if (!hljs || typeof hljs.highlight !== "function") {
		throw new Error(`failed to initialize highlight.js from ${highlightFile}`);
	}
	hljs.configure?.({ tabReplace: "    ", languages: [] });
	return hljs;
}

function decodeHtmlEntities(text: string): string {
	return text.replace(
		/&(lt|gt|amp|quot|#39|#x27|#x2F|#(\d+)|#x([0-9a-fA-F]+));/g,
		(match, named: string, dec?: string, hex?: string) => {
			switch (named) {
				case "lt":
					return "<";
				case "gt":
					return ">";
				case "amp":
					return "&";
				case "quot":
					return '"';
				case "#39":
				case "#x27":
					return "'";
				case "#x2F":
					return "/";
				default:
					if (dec) {
						const cp = Number.parseInt(dec, 10);
						return Number.isFinite(cp) && cp >= 0 && cp <= 0x10ffff
							? String.fromCodePoint(cp)
							: match;
					}
					if (hex) {
						const cp = Number.parseInt(hex, 16);
						return Number.isFinite(cp) && cp >= 0 && cp <= 0x10ffff
							? String.fromCodePoint(cp)
							: match;
					}
					return match;
			}
		},
	);
}

interface TextSegment {
	boring: boolean;
	start: number;
	end: number;
}

/**
 * Splits a code block's inner HTML into plain text and `.boring` character
 * ranges (preserving mdBook's hidden `# ` lines).
 */
function extractCodeAndBoringRanges(innerHtml: string): {
	code: string;
	boringRanges: TextSegment[];
} {
	const segments: TextSegment[] = [];
	const parts: string[] = [];
	let offset = 0;
	let cursor = 0;

	const boringRe = /<span class="boring">([\s\S]*?)<\/span>/g;
	for (const match of innerHtml.matchAll(boringRe)) {
		const idx = match.index ?? 0;
		if (idx > cursor) {
			const plain = decodeHtmlEntities(innerHtml.slice(cursor, idx)).replace(
				/\t/g,
				"    ",
			);
			parts.push(plain);
			offset += plain.length;
		}
		const boringText = decodeHtmlEntities(match[1]).replace(/\t/g, "    ");
		segments.push({
			boring: true,
			start: offset,
			end: offset + boringText.length,
		});
		parts.push(boringText);
		offset += boringText.length;
		cursor = idx + match[0].length;
	}

	if (cursor < innerHtml.length) {
		const tail = decodeHtmlEntities(innerHtml.slice(cursor)).replace(
			/\t/g,
			"    ",
		);
		parts.push(tail);
	}

	return { code: parts.join(""), boringRanges: segments };
}

/**
 * Wraps the given plain-text character ranges in `<span class="boring">...</span>`
 * inside already-highlighted HTML without producing overlapping/unbalanced tags.
 */
function applyBoringRanges(
	highlightedHtml: string,
	ranges: readonly TextSegment[],
): string {
	if (ranges.length === 0) {
		return highlightedHtml;
	}

	const out: string[] = [];
	const openSpans: string[] = [];
	let rangeIdx = 0;
	let inBoring = false;
	let charPos = 0;
	let i = 0;
	const len = highlightedHtml.length;

	const openBoring = () => {
		if (inBoring) {
			return;
		}
		for (let s = openSpans.length - 1; s >= 0; s -= 1) {
			out.push("</span>");
		}
		out.push('<span class="boring">');
		for (const tag of openSpans) {
			out.push(tag);
		}
		inBoring = true;
	};

	const closeBoring = () => {
		if (!inBoring) {
			return;
		}
		for (let s = openSpans.length - 1; s >= 0; s -= 1) {
			out.push("</span>");
		}
		out.push("</span>");
		for (const tag of openSpans) {
			out.push(tag);
		}
		inBoring = false;
	};

	while (i < len) {
		if (highlightedHtml[i] === "<") {
			const end = highlightedHtml.indexOf(">", i);
			if (end === -1) {
				out.push(highlightedHtml.slice(i));
				break;
			}
			const tag = highlightedHtml.slice(i, end + 1);
			if (tag.startsWith("</")) {
				openSpans.pop();
			} else if (!tag.endsWith("/>")) {
				openSpans.push(tag);
			}
			out.push(tag);
			i = end + 1;
			continue;
		}

		// Advance range cursor past finished ranges.
		while (rangeIdx < ranges.length && charPos >= ranges[rangeIdx].end) {
			if (inBoring) {
				closeBoring();
			}
			rangeIdx += 1;
		}

		const curRange = ranges[rangeIdx];
		if (curRange && charPos >= curRange.start && charPos < curRange.end) {
			if (!inBoring) {
				openBoring();
			}
		} else if (inBoring) {
			closeBoring();
		}

		// Consume either one HTML entity (counts as 1 logical character) or 1 char.
		if (highlightedHtml[i] === "&") {
			const semi = highlightedHtml.indexOf(";", i);
			if (semi !== -1 && semi - i <= 12) {
				out.push(highlightedHtml.slice(i, semi + 1));
				charPos += 1;
				i = semi + 1;
				continue;
			}
		}

		out.push(highlightedHtml[i]);
		charPos += 1;
		i += 1;
	}

	if (inBoring) {
		closeBoring();
	}

	return out.join("");
}

/**
 * Pre-highlights all `<pre><code>` blocks in an HTML document and bakes in
 * copy-to-clipboard (and hidden-line toggle) buttons statically so zero runtime
 * highlight/DOM-mutation work happens on page load.
 */
export function highlightAndEnhanceCodeBlocks(
	html: string,
	hljs: HljsLike | null,
	addCopyButtons: boolean,
): { html: string; hasBoring: boolean } {
	let hasBoring = false;

	const transformed = html.replace(
		/<pre(\b[^>]*)><code(\b[^>]*)>([\s\S]*?)<\/code><\/pre>/g,
		(_full, preAttrs: string, codeAttrs: string, inner: string) => {
			const classMatch = /\bclass="([^"]*)"/.exec(codeAttrs);
			const rawClasses = classMatch
				? classMatch[1].split(/\s+/).filter(Boolean)
				: [];
			const classSet = new Set(rawClasses);
			classSet.add("hljs");

			const { code, boringRanges } = extractCodeAndBoringRanges(inner);
			const blockHasBoring = boringRanges.length > 0;
			if (blockHasBoring) {
				hasBoring = true;
				classSet.add("hide-boring");
			}

			let renderedInner = inner;
			const langClass = rawClasses.find((c) => c.startsWith("language-"));
			const skipHighlight = classSet.has("nohighlight") ||
				classSet.has("no-highlight") ||
				langClass === "language-text" ||
				langClass === "language-plaintext" ||
				langClass === "language-txt";

			if (hljs && langClass && !skipHighlight) {
				const lang = langClass.slice("language-".length);
				if (hljs.getLanguage(lang)) {
					try {
						const highlighted = hljs.highlight(lang, code, true).value;
						renderedInner = applyBoringRanges(highlighted, boringRanges);
					} catch {
						// Keep original escaped markup on unexpected highlight.js error.
					}
				}
			}

			const newClassAttr = `class="${Array.from(classSet).join(" ")}"`;
			const newCodeAttrs = classMatch
				? codeAttrs.replace(/\bclass="[^"]*"/, newClassAttr)
				: `${codeAttrs} ${newClassAttr}`;

			let buttonsHtml = "";
			if (addCopyButtons || blockHasBoring) {
				const parts: string[] = [];
				if (addCopyButtons) {
					parts.push(COPY_BUTTON_HTML);
				}
				if (blockHasBoring) {
					parts.push(EYE_BUTTON_HTML);
				}
				buttonsHtml = `<div class="buttons">${parts.join("")}</div>`;
			}

			// Drop `playground` class since runnable playgrounds are disabled.
			const cleanPreAttrs = preAttrs.replace(/\s*class="playground"/, "");
			return `<pre${cleanPreAttrs}>${buttonsHtml}<code${newCodeAttrs}>${renderedInner}</code></pre>`;
		},
	);

	return { html: transformed, hasBoring };
}

// ---------------------------------------------------------------------------
// Compact search index + tiny on-demand searcher
// ---------------------------------------------------------------------------

interface MdBookSearchPayload {
	doc_urls: string[];
	index: {
		documentStore: {
			docs: Record<
				string,
				{
					id?: string;
					title?: string;
					breadcrumbs?: string;
					body?: string;
				}
			>;
		};
	};
}

export interface CompactSearchData {
	/** Document URLs (with `#anchor`), indexed by doc ID. */
	u: string[];
	/** `[breadcrumbs, body]` tuples matching `u` order. */
	d: Array<[string, string]>;
}

/**
 * Extracts a compact JSON search payload from mdBook's generated
 * `searchindex-*.js` (dropping the multi-megabyte elasticlunr trie).
 */
export function buildCompactSearchData(
	searchIndexJs: string,
): CompactSearchData {
	const sandbox: { window: { search: Record<string, unknown> } } = {
		window: { search: {} },
	};
	vm.runInNewContext(searchIndexJs, sandbox);
	const payload = sandbox.window.search as unknown as MdBookSearchPayload;
	const urls = Array.from(payload.doc_urls ?? [], String);
	const docsMap = payload.index?.documentStore?.docs ?? {};
	const docs: Array<[string, string]> = urls.map((_url, idx) => {
		const doc = docsMap[String(idx)];
		const breadcrumbs = (doc?.breadcrumbs ?? doc?.title ?? "").trim();
		const body = (doc?.body ?? "").replace(/\s+/g, " ").trim();
		return [breadcrumbs, body];
	});
	return { u: urls, d: docs };
}

function renderOnDemandSearchScript(searchDataRelPath: string): string {
	// Self-contained ~2.8 KB searcher + DOM text-node highlighter (`?highlight=`).
	// Preserves all mdBook DOM IDs, URL params (`?search=`, `?highlight=`),
	// teaser markup (`<em>`), and keyboard navigation without using `innerHTML`.
	return `(function(){
"use strict";
const root=typeof path_to_root==="string"?path_to_root:"";
const wrap=document.getElementById("mdbook-search-wrapper");
const outer=document.getElementById("mdbook-searchbar-outer");
const bar=document.getElementById("mdbook-searchbar");
const list=document.getElementById("mdbook-searchresults");
const listOuter=document.getElementById("mdbook-searchresults-outer");
const header=document.getElementById("mdbook-searchresults-header");
const toggle=document.getElementById("mdbook-search-toggle");
const content=document.getElementById("mdbook-content");
if(!wrap||!outer||!bar||!list||!listOuter||!header||!toggle){return;}

let data=null;
let loading=null;
let lastQuery="";
let teaserId=0;

const hasFocus=()=>bar===document.activeElement;
window.search=window.search||{};
window.search.hasFocus=hasFocus;

function unmark(){
if(!content){return;}
for(const m of Array.from(content.querySelectorAll("mark.pina-hl"))){
const p=m.parentNode;
if(p){p.replaceChild(document.createTextNode(m.textContent||""),m);p.normalize();}
}
}

function markTerms(terms){
unmark();
if(!content||!terms.length){return;}
const clean=terms.map(t=>t.trim()).filter(Boolean).map(t=>t.replace(/[.*+?^$()|[\\]\\\\]/g,"\\\\$&"));
if(!clean.length){return;}
const pat="("+clean.join("|")+")";
const testRe=new RegExp(pat,"i");
const re=new RegExp(pat,"gi");
const walker=document.createTreeWalker(content,NodeFilter.SHOW_TEXT,{
acceptNode(n){
const p=n.parentElement;
if(!p||p.closest("script,style,svg,mark,button")){return NodeFilter.FILTER_REJECT;}
return testRe.test(n.nodeValue||"")?NodeFilter.FILTER_ACCEPT:NodeFilter.FILTER_SKIP;
}
});
const nodes=[];
while(walker.nextNode()){nodes.push(walker.currentNode);}
for(const node of nodes){
const text=node.nodeValue||"";
re.lastIndex=0;
let last=0,m;
const frag=document.createDocumentFragment();
while((m=re.exec(text))!==null){
if(m.index>last){frag.appendChild(document.createTextNode(text.slice(last,m.index)));}
const mk=document.createElement("mark");
mk.className="pina-hl";
mk.textContent=m[0];
frag.appendChild(mk);
last=m.index+m[0].length;
}
if(last<text.length){frag.appendChild(document.createTextNode(text.slice(last)));}
if(node.parentNode){node.parentNode.replaceChild(frag,node);}
}
const marks=content.querySelectorAll("mark.pina-hl");
const hide=()=>{
for(const m of marks){m.classList.add("fade-out");}
setTimeout(unmark,300);
updateUrl("","replace");
};
for(const m of marks){m.addEventListener("click",hide);}
}

function updateUrl(query,mode){
const u=new URL(window.location.href);
const had=u.searchParams.has("search");
if(query||mode==="push_if_new"){
u.searchParams.set("search",query);
u.searchParams.delete("highlight");
u.hash="";
}else{
u.searchParams.delete("search");
u.searchParams.delete("highlight");
}
if(mode==="push"||(mode==="push_if_new"&&!had)){history.pushState({},document.title,u);}
else{history.replaceState({},document.title,u);}
}

function syncCurrentQuery(){
const cur=bar.value.trim();
if(cur){
bar.classList.add("active");
renderResults(cur);
}else{
lastQuery="";
bar.classList.remove("active");
listOuter.classList.add("hidden");
list.textContent="";
}
}

function showLoadError(){
header.textContent="Failed to load search index.";
list.textContent="";
const li=document.createElement("li");
const btn=document.createElement("button");
btn.type="button";
btn.textContent="Retry search";
btn.addEventListener("click",()=>{
loadData().then(loaded=>{if(loaded){syncCurrentQuery();}});
});
li.appendChild(btn);
list.appendChild(li);
listOuter.classList.remove("hidden");
}

function loadData(){
if(data){return Promise.resolve(data);}
if(!loading){
outer.classList.add("searching");
loading=fetch(root+${JSON.stringify(searchDataRelPath)})
.then(r=>{if(!r.ok){throw new Error("HTTP "+r.status);}return r.json();})
.then(json=>{
data=json.u.map((u,i)=>{
const entry=json.d[i]||["",""];
const bc=entry[0]||"";
const body=entry[1]||"";
const sep=bc.lastIndexOf(" » ");
const title=sep>=0?bc.slice(sep+3):bc;
return {u,bc,body,tl:title.toLowerCase(),bl:bc.toLowerCase(),dl:body.toLowerCase()};
});
loading=null;
outer.classList.remove("searching");
return data;
})
.catch(err=>{
loading=null;
lastQuery="";
outer.classList.remove("searching");
console.error(err);
showLoadError();
return null;
});
}
return loading;
}

function appendTeaser(container,body,terms){
if(!body){return;}
const lower=body.toLowerCase();
let pos=-1;
for(const t of terms){
const idx=lower.indexOf(t);
if(idx>=0&&(pos===-1||idx<pos)){pos=idx;}
}
const start=pos>60?Math.max(0,body.lastIndexOf(" ",pos-60)+1):0;
const end=Math.min(body.length,start+220);
let slice=body.slice(start,end).trim();
if(start>0){slice="…"+slice;}
if(end<body.length){slice+="…";}
const pat=terms.map(t=>t.replace(/[.*+?^$()|[\\]\\\\]/g,"\\\\$&")).filter(Boolean);
if(!pat.length){container.textContent=slice;return;}
const re=new RegExp("("+pat.join("|")+")","gi");
let last=0,m;
while((m=re.exec(slice))!==null){
if(m.index>last){container.appendChild(document.createTextNode(slice.slice(last,m.index)));}
const em=document.createElement("em");
em.textContent=m[0];
container.appendChild(em);
last=m.index+m[0].length;
}
if(last<slice.length){container.appendChild(document.createTextNode(slice.slice(last)));}
}

function renderResults(query){
if(!data||lastQuery===query){return;}
lastQuery=query;
const terms=query.toLowerCase().split(/\\s+/).filter(Boolean);
if(!terms.length){
listOuter.classList.add("hidden");
list.textContent="";
return;
}
const scored=[];
for(let i=0;i<data.length;i+=1){
const doc=data[i];
let score=0,matched=0;
for(const t of terms){
let s=0;
const ti=doc.tl.indexOf(t);
if(ti>=0){s+=ti===0?12:8;}
else if(doc.bl.indexOf(t)>=0){s+=5;}
const di=doc.dl.indexOf(t);
if(di>=0){s+=2+(di<120?1:0);}
if(s>0){matched+=1;score+=s;}
}
if(matched>0){
score+=matched===terms.length?20:0;
scored.push([score,doc]);
}
}
scored.sort((a,b)=>b[0]-a[0]);
const top=scored.slice(0,30);
header.textContent=top.length===0?"No search results for '"+query+"'.":top.length+(top.length===1?" search result":" search results")+" for '"+query+"':";
list.textContent="";
const enc=encodeURIComponent(terms.join(" ")).replace(/'/g,"%27");
const frag=document.createDocumentFragment();
for(const [,doc] of top){
const parts=doc.u.split("#");
const page=parts[0]||"";
if(/^(?:[a-z][a-z0-9+.-]*:|\\/\\/)/i.test(page)){continue;}
teaserId+=1;
const href=root+page+"?highlight="+enc+(parts[1]?"#"+parts[1]:"");
const li=document.createElement("li");
const a=document.createElement("a");
a.setAttribute("href",href);
a.setAttribute("aria-details","mdbook-teaser_"+teaserId);
a.textContent=doc.bc;
const sp=document.createElement("span");
sp.className="teaser";
sp.id="mdbook-teaser_"+teaserId;
sp.setAttribute("aria-label","Search Result Teaser");
appendTeaser(sp,doc.body,terms);
li.appendChild(a);
li.appendChild(sp);
frag.appendChild(li);
}
list.appendChild(frag);
listOuter.classList.remove("hidden");
}

function show(yes){
if(yes){
wrap.classList.remove("hidden");
toggle.setAttribute("aria-expanded","true");
loadData().then(loaded=>{if(loaded){syncCurrentQuery();}});
}else{
wrap.classList.add("hidden");
toggle.setAttribute("aria-expanded","false");
for(const li of list.children){li.classList.remove("focus");}
}
}

function onInput(){
const q=bar.value.trim();
if(q){
bar.classList.add("active");
loadData().then(loaded=>{if(loaded){syncCurrentQuery();}});
}else{
lastQuery="";
bar.classList.remove("active");
listOuter.classList.add("hidden");
list.textContent="";
}
updateUrl(q,"push_if_new");
unmark();
}

function fromUrl(){
const u=new URL(window.location.href);
const q=u.searchParams.get("search");
if(q){
show(true);
bar.value=q;
onInput();
}else{
show(false);
}
const hl=u.searchParams.get("highlight");
if(hl){markTerms(hl.split(/\\s+/));}
}

toggle.addEventListener("click",()=>{
if(wrap.classList.contains("hidden")){show(true);window.scrollTo(0,0);bar.select();}
else{show(false);}
});
bar.addEventListener("input",onInput);
outer.addEventListener("submit",e=>e.preventDefault());
window.addEventListener("popstate",fromUrl);

document.addEventListener("keydown",e=>{
if(e.altKey||e.ctrlKey||e.metaKey||e.shiftKey){return;}
const t=e.target;
if(t&&!hasFocus()&&/^(?:INPUT|SELECT|TEXTAREA)$/.test(t.nodeName)){return;}
if(e.key==="Escape"){
if(!wrap.classList.contains("hidden")){
e.preventDefault();
bar.classList.remove("active");
updateUrl("",bar.value.trim()?"push":"replace");
bar.blur();
show(false);
}
unmark();
}else if(!hasFocus()&&(e.key==="/"||e.key==="s")){
e.preventDefault();
show(true);
window.scrollTo(0,0);
bar.select();
}else if(hasFocus()&&(e.key==="ArrowDown"||e.key==="Enter")){
const first=list.firstElementChild;
if(first){
e.preventDefault();
bar.blur();
first.classList.add("focus");
if(e.key==="Enter"){const a=first.querySelector("a");if(a){window.location.assign(a.href);}}
}
}else if(!hasFocus()&&(e.key==="ArrowDown"||e.key==="ArrowUp"||e.key==="Enter")){
const cur=list.querySelector("li.focus");
if(!cur){return;}
e.preventDefault();
if(e.key==="ArrowDown"&&cur.nextElementSibling){cur.classList.remove("focus");cur.nextElementSibling.classList.add("focus");}
else if(e.key==="ArrowUp"){cur.classList.remove("focus");if(cur.previousElementSibling){cur.previousElementSibling.classList.add("focus");}else{bar.select();}}
else if(e.key==="Enter"){const a=cur.querySelector("a");if(a){window.location.assign(a.href);}}
}
});

window.search.open=()=>{show(true);window.scrollTo(0,0);bar.select();};
window.search.fromUrl=fromUrl;
fromUrl();
})();`;
}

// ---------------------------------------------------------------------------
// Lean book runtime (replaces mdBook's 31 KB book.js + 11 KB clipboard.min.js)
// ---------------------------------------------------------------------------

function renderLeanBookScript(
	searchScriptRelPath: string | null,
	themeColors: Record<string, string>,
): string {
	return `(function(){
"use strict";
const root=typeof path_to_root==="string"?path_to_root:"";
const html=document.documentElement;

function otherFocus(e){
const t=(e.composedPath&&e.composedPath()[0])||e.target;
if(!t||!t.nodeName){return false;}
if(t.classList&&t.classList.contains("checkbox-img")){return false;}
return /^(?:INPUT|SELECT|TEXTAREA)$/.test(t.nodeName);
}

// Theme switcher (zero layout-thrashing getComputedStyle call)
(function themes(){
const toggle=document.getElementById("mdbook-theme-toggle");
const popup=document.getElementById("mdbook-theme-list");
const meta=document.querySelector('meta[name="theme-color"]');
if(!toggle||!popup){return;}
const colors=${JSON.stringify(themeColors)};
const ids=Array.from(popup.querySelectorAll("button.theme"),el=>el.id);
const findThemeBtn=name=>{const el=document.getElementById("mdbook-theme-"+name);return el&&popup.contains(el)?el:null;};
const saved=()=>{try{return localStorage.getItem("mdbook-theme");}catch{return null;}};
const pick=()=>{
const t=saved();
if(!t||!ids.includes("mdbook-theme-"+t)){
if(typeof default_dark_theme==="undefined"){return default_theme;}
return matchMedia("(prefers-color-scheme: dark)").matches?default_dark_theme:default_light_theme;
}
return t;
};
let prev=typeof default_theme==="string"?default_theme:"light";
function apply(t,store){
if(meta&&colors[t]){meta.content=colors[t];}
if(store){try{localStorage.setItem("mdbook-theme",t);}catch{}}
html.classList.remove(prev);
html.classList.add(t);
prev=t;
popup.querySelectorAll(".theme-selected").forEach(el=>el.classList.remove("theme-selected"));
const sel=saved()??"default_theme";
const btn=findThemeBtn(sel)||findThemeBtn(pick());
if(btn){btn.classList.add("theme-selected");}
}
const show=()=>{popup.style.display="block";toggle.setAttribute("aria-expanded","true");const b=findThemeBtn(pick());if(b){b.focus();}};
const hide=()=>{popup.style.display="none";toggle.setAttribute("aria-expanded","false");toggle.focus();};
const mq=matchMedia("(prefers-color-scheme: dark)");
mq.onchange=()=>apply(pick(),false);
apply(pick(),false);
toggle.addEventListener("click",()=>popup.style.display==="block"?hide():show());
popup.addEventListener("click",e=>{
const b=e.target.closest&&e.target.closest("button.theme");
if(!b){return;}
const t=b.id.replace(/^mdbook-theme-/,"");
if(t==="default_theme"){try{localStorage.removeItem("mdbook-theme");}catch{}apply(pick(),false);}
else{apply(t,true);}
});
popup.addEventListener("focusout",e=>{
if(e.relatedTarget&&!toggle.contains(e.relatedTarget)&&!popup.contains(e.relatedTarget)){hide();}
});
document.addEventListener("click",e=>{
if(popup.style.display==="block"&&!toggle.contains(e.target)&&!popup.contains(e.target)){hide();}
});
document.addEventListener("keydown",e=>{
if(e.altKey||e.ctrlKey||e.metaKey||e.shiftKey||!popup.contains(e.target)){return;}
const li=document.activeElement&&document.activeElement.parentElement;
if(e.key==="Escape"){e.preventDefault();hide();}
else if(e.key==="ArrowUp"&&li&&li.previousElementSibling){e.preventDefault();li.previousElementSibling.querySelector("button").focus();}
else if(e.key==="ArrowDown"&&li&&li.nextElementSibling){e.preventDefault();li.nextElementSibling.querySelector("button").focus();}
else if(e.key==="Home"){e.preventDefault();const f=popup.querySelector("li:first-child button");if(f){f.focus();}}
else if(e.key==="End"){e.preventDefault();const l=popup.querySelector("li:last-child button");if(l){l.focus();}}
});
})();

// Sidebar toggle, drag-resize, and touch swipe
(function sidebar(){
const el=document.getElementById("mdbook-sidebar");
const toggle=document.getElementById("mdbook-sidebar-toggle");
const handle=document.getElementById("mdbook-sidebar-resize-handle");
const box=document.getElementById("mdbook-sidebar-toggle-anchor");
if(!el||!toggle||!box){return;}
const setTab=v=>{for(const a of el.querySelectorAll("a")){a.tabIndex=v;}};
const visible=html.classList.contains("sidebar-visible");
setTab(visible?0:-1);
if(!visible){el.style.display="none";}
el.addEventListener("transitionend",()=>{if(!box.checked){el.style.display="none";}});
toggle.addEventListener("click",()=>{if(!box.checked){el.style.display="";el.offsetHeight;}});
function show(){
html.classList.add("sidebar-visible");
setTab(0);
toggle.setAttribute("aria-expanded","true");
el.setAttribute("aria-hidden","false");
try{localStorage.setItem("mdbook-sidebar","visible");}catch{}
}
function hide(){
html.classList.remove("sidebar-visible");
setTab(-1);
toggle.setAttribute("aria-expanded","false");
el.setAttribute("aria-hidden","true");
try{localStorage.setItem("mdbook-sidebar","hidden");}catch{}
}
box.addEventListener("change",()=>{
if(box.checked){
const w=parseInt(html.style.getPropertyValue("--sidebar-target-width"),10);
if(w<150){html.style.setProperty("--sidebar-target-width","150px");}
show();
}else{hide();}
});
if(handle){
const resize=e=>{
let pos=e.clientX-el.offsetLeft;
if(pos<20){hide();}
else{if(!html.classList.contains("sidebar-visible")){show();}html.style.setProperty("--sidebar-target-width",Math.min(pos,innerWidth-100)+"px");}
};
const stop=()=>{html.classList.remove("sidebar-resizing");removeEventListener("mousemove",resize);removeEventListener("mouseup",stop);};
handle.addEventListener("mousedown",()=>{addEventListener("mousemove",resize);addEventListener("mouseup",stop);html.classList.add("sidebar-resizing");});
}
let touch=null;
document.addEventListener("touchstart",e=>{touch={x:e.touches[0].clientX,t:Date.now()};},{passive:true});
document.addEventListener("touchmove",e=>{
if(!touch){return;}
const cx=e.touches[0].clientX,dx=cx-touch.x,dt=Date.now()-touch.t;
if(dt<250&&Math.abs(dx)>=150){
if(dx>=0&&touch.x<Math.min(document.body.clientWidth*0.25,300)){show();}
else if(dx<0&&cx<300){hide();}
touch=null;
}
},{passive:true});
})();

// Chapter keyboard navigation + help popup
(function nav(){
const help=document.getElementById("mdbook-help-container");
const popup=document.getElementById("mdbook-help-popup");
document.addEventListener("keydown",e=>{
if(e.altKey||e.ctrlKey||e.metaKey||(window.search&&window.search.hasFocus&&window.search.hasFocus())||otherFocus(e)){return;}
if(e.key==="?"&&help&&popup){
e.preventDefault();
help.style.display="flex";
const close=()=>{help.style.display="none";document.removeEventListener("keydown",onEsc,true);help.removeEventListener("mousedown",onDown);};
const onDown=ev=>{if(!popup.contains(ev.target)&&ev.button===0){ev.preventDefault();ev.stopPropagation();close();}};
const onEsc=ev=>{if(ev.key==="Escape"){ev.preventDefault();ev.stopPropagation();close();}};
document.addEventListener("keydown",onEsc,true);
help.addEventListener("mousedown",onDown);
return;
}
if(e.key==="Escape"){for(const c of document.querySelectorAll("input.checkbox-img")){c.checked=false;}return;}
if(e.shiftKey){return;}
if(e.key==="ArrowRight"||e.key==="ArrowLeft"){
e.preventDefault();
const goNext=(e.key==="ArrowRight")!==(html.dir==="rtl");
const a=document.querySelector(goNext?".nav-chapters.next":".nav-chapters.previous");
if(a){location.href=a.href;}
}
});
})();

// Delegated copy-to-clipboard and hidden-line toggle (zero per-block setup cost)
(function codeActions(){
async function copyText(text){
if(navigator.clipboard&&navigator.clipboard.writeText){await navigator.clipboard.writeText(text);return;}
const ta=document.createElement("textarea");
ta.value=text;ta.style.position="fixed";ta.style.opacity="0";
document.body.appendChild(ta);
try{
ta.select();
if(!document.execCommand("copy")){throw new Error("copy failed");}
}finally{
ta.remove();
}
}
document.addEventListener("click",e=>{
const clip=e.target.closest&&e.target.closest("button.clip-button");
if(clip){
const pre=clip.closest("pre");
const code=pre&&pre.querySelector("code");
if(!code){return;}
copyText(code.innerText).then(()=>{
if(clip.firstChild){clip.firstChild.textContent="Copied!";}
clip.className="clip-button tooltipped";
}).catch(()=>{
if(clip.firstChild){clip.firstChild.textContent="Clipboard error!";}
clip.className="clip-button tooltipped";
});
return;
}
const eye=e.target.closest&&e.target.closest("button.boring-toggle");
if(eye){
const pre=eye.closest("pre");
const code=pre&&pre.querySelector("code");
if(!code){return;}
const showing=code.classList.toggle("hide-boring");
const label=showing?"Show hidden lines":"Hide lines";
eye.title=label;
eye.setAttribute("aria-label",label);
const tpl=document.getElementById(showing?"fa-eye":"fa-eye-slash");
if(tpl){
if(tpl.content){eye.replaceChildren(tpl.content.cloneNode(true));}
else{eye.innerHTML=tpl.innerHTML;}
}
}
});
document.addEventListener("mouseout",e=>{
const clip=e.target.closest&&e.target.closest("button.clip-button.tooltipped");
if(clip&&!clip.contains(e.relatedTarget)){
if(clip.firstChild){clip.firstChild.textContent="";}
clip.className="clip-button";
}
});
})();

// Menu bar scroll tracking (zero forced-layout menu.offsetTop reads)
(function menuBar(){
const menu=document.getElementById("mdbook-menu-bar");
if(!menu){return;}
const title=document.querySelector(".menu-title");
if(title){title.addEventListener("click",()=>document.scrollingElement.scrollTo({top:0,behavior:"smooth"}));}
let prevY=0,topY=0,sticky=true,bordered=false,minY=-110,raf=0;
function update(){
raf=0;
const y=Math.max(document.scrollingElement.scrollTop,0);
let nextSticky=null,nextTop=null;
if(y>prevY){
nextSticky=false;
if(topY-y>0){nextTop=prevY;}
}else{
if(topY-y>0){nextSticky=true;}
else if(topY-y<minY){nextTop=prevY+minY;}
}
if(nextSticky===true&&!sticky){menu.classList.add("sticky");sticky=true;}
else if(nextSticky===false&&sticky){menu.classList.remove("sticky");sticky=false;}
if(nextTop!==null){menu.style.top=nextTop+"px";topY=nextTop;}
const wantBorder=y>0;
if(wantBorder!==bordered){menu.classList.toggle("bordered",wantBorder);bordered=wantBorder;}
prevY=y;
}
document.addEventListener("scroll",()=>{if(!raf){raf=requestAnimationFrame(update);}},{passive:true});
})();

// On-demand search loader (0 bytes until clicked, pressed / or s, or URL param)
${
		searchScriptRelPath
			? `(function lazySearch(){
const toggle=document.getElementById("mdbook-search-toggle");
if(!toggle){return;}
let loading=false,ready=false,pendingOpen=false;
function onClick(){
if(ready){return;}
boot(true);
}
function onKey(e){
if(ready||e.altKey||e.ctrlKey||e.metaKey||e.shiftKey||otherFocus(e)){return;}
if(e.key==="/"||e.key==="s"){e.preventDefault();boot(true);}
}
function boot(openNow){
if(openNow){pendingOpen=true;}
if(ready){
if(pendingOpen&&window.search&&window.search.open){pendingOpen=false;window.search.open();}
return;
}
if(loading){return;}
loading=true;
const s=document.createElement("script");
s.src=root+${JSON.stringify(searchScriptRelPath)};
s.onload=()=>{
loading=false;
ready=true;
toggle.removeEventListener("click",onClick);
document.removeEventListener("keydown",onKey);
if(pendingOpen&&window.search&&window.search.open){pendingOpen=false;window.search.open();}
};
s.onerror=()=>{
loading=false;
pendingOpen=false;
s.remove();
};
document.head.appendChild(s);
}
toggle.addEventListener("click",onClick);
document.addEventListener("keydown",onKey);
const p=new URLSearchParams(location.search);
if(p.has("search")||p.has("highlight")){boot(false);}
})();`
			: ""
	}

// Hover prefetch for same-origin doc links (instant chapter switches)
(function prefetch(){
const conn=navigator.connection;
if(conn&&(conn.saveData||/2g/.test(conn.effectiveType||""))){return;}
const seen=new Set([location.href.split("#")[0]]);
document.addEventListener("pointerover",e=>{
const a=e.target&&e.target.closest&&e.target.closest("a[href]");
if(!a||a.origin!==location.origin||a.target==="_blank"||a.hasAttribute("download")){return;}
const url=a.href.split("#")[0];
if(seen.has(url)||!/\\.html$|\\/$/.test(new URL(url).pathname)){return;}
seen.add(url);
const link=document.createElement("link");
link.rel="prefetch";
link.as="document";
link.href=url;
document.head.appendChild(link);
},{passive:true});
})();
})();`;
}

// ---------------------------------------------------------------------------
// CSS bundling & URL rebasing
// ---------------------------------------------------------------------------

/**
 * Rebases relative `url(...)` paths in `css` from `sourceRel` to `targetRel`
 * (both POSIX paths relative to the book root). Parses quoted and unquoted
 * URL values separately so parentheses inside quoted paths rebase cleanly.
 */
export function rebaseCssUrls(
	css: string,
	sourceRel: string,
	targetRel: string,
): string {
	const srcDir = posix.dirname(sourceRel);
	const dstDir = posix.dirname(targetRel);
	if (srcDir === dstDir) {
		return css;
	}
	return css.replace(
		/url\(\s*(?:"((?:\\.|[^"\\])*)"|'((?:\\.|[^'\\])*)'|([^)'"\s][^)]*))\s*\)/g,
		(full, dq?: string, sq?: string, unquoted?: string) => {
			const quote = dq !== undefined ? '"' : sq !== undefined ? "'" : "";
			const rawUrl = dq ?? sq ?? unquoted ?? "";
			const trimmed = rawUrl.trim();
			if (
				!trimmed ||
				trimmed.startsWith("data:") ||
				trimmed.startsWith("http:") ||
				trimmed.startsWith("https:") ||
				trimmed.startsWith("//") ||
				trimmed.startsWith("/") ||
				trimmed.startsWith("#")
			) {
				return full;
			}
			const suffixMatch = /[?#][\s\S]*$/.exec(trimmed);
			const suffix = suffixMatch ? suffixMatch[0] : "";
			const pathPart = suffix ? trimmed.slice(0, suffixMatch!.index) : trimmed;
			if (!pathPart) {
				return full;
			}
			const resolved = posix.normalize(posix.join(srcDir, pathPart));
			const rebased = posix.relative(dstDir, resolved);
			return `url(${quote}${rebased}${suffix}${quote})`;
		},
	);
}

/**
 * Prefixes every selector in a simple highlight.js stylesheet with `scope`
 * (e.g. `.light,.rust` or `.navy,.coal`) so theme-specific syntax palettes can
 * live inside one shared stylesheet without toggling `<link disabled>`.
 */
function scopeCssSelectors(css: string, scope: string): string {
	const scopes = scope.split(",").map((s) => s.trim()).filter(Boolean);
	return minifyCss(css).replace(
		/(^|\})([^{}@]+)\{/g,
		(_m, prefix: string, selectorList: string) => {
			const scoped = selectorList
				.split(",")
				.map((sel) => sel.trim())
				.filter(Boolean)
				.flatMap((sel) => scopes.map((sc) => `${sc} ${sel}`))
				.join(",");
			return `${prefix}${scoped}{`;
		},
	);
}

// ---------------------------------------------------------------------------
// Directory traversal helpers
// ---------------------------------------------------------------------------

function walkFiles(dir: string, root = dir): string[] {
	const results: string[] = [];
	for (const entry of readdirSync(dir, { withFileTypes: true })) {
		const full = join(dir, entry.name);
		if (entry.isDirectory()) {
			results.push(...walkFiles(full, root));
		} else if (entry.isFile()) {
			results.push(relative(root, full).split("\\").join("/"));
		}
	}
	return results;
}

function findSingle(files: readonly string[], pattern: RegExp): string | null {
	return files.find((f) => pattern.test(f)) ?? null;
}

function removeIfExists(filePath: string): void {
	if (existsSync(filePath)) {
		unlinkSync(filePath);
	}
}

function removeEmptyDir(dirPath: string): void {
	if (existsSync(dirPath) && readdirSync(dirPath).length === 0) {
		rmSync(dirPath, { recursive: true, force: true });
	}
}

function resolveBookRelativeHref(href: string, prefix: string): string | null {
	const cleaned = href.replace(/[?#][\s\S]*$/, "");
	if (
		!cleaned ||
		cleaned.startsWith("http:") ||
		cleaned.startsWith("https:") ||
		cleaned.startsWith("//") ||
		cleaned.startsWith("/") ||
		cleaned.startsWith("data:")
	) {
		return null;
	}
	if (prefix && !cleaned.startsWith(prefix)) {
		return null;
	}
	const withoutPrefix = prefix ? cleaned.slice(prefix.length) : cleaned;
	return posix.normalize(withoutPrefix);
}

// ---------------------------------------------------------------------------
// Main book optimization pipeline
// ---------------------------------------------------------------------------

export interface OptimizeStats {
	htmlFilesOptimized: number;
	bundleCssFile: string;
	bundleJsFile: string;
	searchDataFile: string | null;
	searchJsFile: string | null;
}

/**
 * Optimizes a built mdBook directory in place. Idempotent when run on an
 * already-optimized directory.
 */
export function optimizeBook(bookDir: string): OptimizeStats {
	const root = resolve(bookDir);
	const files = walkFiles(root);

	const highlightJs = findSingle(files, /^highlight(?:-[a-f0-9]+)?\.js$/);
	const bookJs = findSingle(files, /^book(?:-[a-f0-9]+)?\.js$/);
	const tocJs = findSingle(files, /^toc(?:-[a-f0-9]+)?\.js$/);
	const clipboardJs = findSingle(
		files,
		/^clipboard(?:-[a-f0-9]+)?\.min\.js$/,
	);
	const elasticlunrJs = findSingle(
		files,
		/^elasticlunr(?:-[a-f0-9]+)?\.min\.js$/,
	);
	const markJs = findSingle(files, /^mark(?:-[a-f0-9]+)?\.min\.js$/);
	const searcherJs = findSingle(files, /^searcher(?:-[a-f0-9]+)?\.js$/);
	const searchIndexJs = findSingle(files, /^searchindex(?:-[a-f0-9]+)?\.js$/);

	const variablesCss = findSingle(
		files,
		/^css\/variables(?:-[a-f0-9]+)?\.css$/,
	);
	const generalCss = findSingle(files, /^css\/general(?:-[a-f0-9]+)?\.css$/);
	const chromeCss = findSingle(files, /^css\/chrome(?:-[a-f0-9]+)?\.css$/);
	const printCss = findSingle(files, /^css\/print(?:-[a-f0-9]+)?\.css$/);
	const fontsCss = findSingle(files, /^fonts\/fonts(?:-[a-f0-9]+)?\.css$/);
	const hlLightCss = findSingle(files, /^highlight(?:-[a-f0-9]+)?\.css$/);
	const hlDarkCss = findSingle(
		files,
		/^tomorrow-night(?:-[a-f0-9]+)?\.css$/,
	);
	const hlAyuCss = findSingle(
		files,
		/^ayu-highlight(?:-[a-f0-9]+)?\.css$/,
	);

	const pinaCss = findSingle(files, /^theme\/pina(?:-[a-f0-9]+)?\.css$/);
	const pinaJs = findSingle(files, /^theme\/pina(?:-[a-f0-9]+)?\.js$/);
	const isPinaBranded = pinaCss !== null && pinaJs !== null;

	// Detect already-optimized output so a second invocation is a safe no-op.
	const existingBundleCss = findSingle(
		files,
		/^assets\/book-[a-f0-9]{8}\.css$/,
	);
	const existingBundleJs = findSingle(files, /^assets\/book-[a-f0-9]{8}\.js$/);
	const hasUnoptimizedSources = Boolean(
		variablesCss ||
			generalCss ||
			chromeCss ||
			bookJs ||
			tocJs ||
			highlightJs ||
			searchIndexJs,
	);
	if ((existingBundleCss || existingBundleJs) && !hasUnoptimizedSources) {
		return {
			htmlFilesOptimized: 0,
			bundleCssFile: existingBundleCss ?? "",
			bundleJsFile: existingBundleJs ?? "",
			searchDataFile: findSingle(
				files,
				/^assets\/search-data-[a-f0-9]{8}\.json$/,
			),
			searchJsFile: findSingle(files, /^assets\/search-[a-f0-9]{8}\.js$/),
		};
	}

	const assetsDir = join(root, "assets");
	mkdirSync(assetsDir, { recursive: true });

	// 1. Bundle & minify discovered CSS into a single hashed stylesheet,
	// tracking every file that the bundle actually supersedes.
	const cssParts: string[] = [];
	const supersededCssFiles = new Set<string>();
	const bundleCssPlaceholder = "assets/book.css";
	const addCss = (rel: string | null, wrapMedia?: string): void => {
		if (!rel) {
			return;
		}
		supersededCssFiles.add(rel);
		const raw = readFileSync(join(root, rel), "utf8");
		const rebased = rebaseCssUrls(raw, rel, bundleCssPlaceholder);
		const min = minifyCss(rebased);
		if (!min) {
			return;
		}
		cssParts.push(wrapMedia ? `@media ${wrapMedia}{${min}}` : min);
	};

	addCss(variablesCss);
	addCss(generalCss);
	addCss(chromeCss);
	addCss(fontsCss);

	// For Pina's branded theme, `theme/pina.css` defines all `.hljs-*` syntax
	// colors uniformly across themes. For standard mdBook themes (e.g. PinaPod),
	// scope each highlight theme stylesheet under its theme class so all syntax
	// highlighting stays inside the single CSS file with zero `<link disabled>`.
	if (isPinaBranded) {
		if (hlLightCss) {
			supersededCssFiles.add(hlLightCss);
		}
		if (hlDarkCss) {
			supersededCssFiles.add(hlDarkCss);
		}
		if (hlAyuCss) {
			supersededCssFiles.add(hlAyuCss);
		}
	} else {
		if (hlLightCss) {
			supersededCssFiles.add(hlLightCss);
			cssParts.push(
				scopeCssSelectors(
					readFileSync(join(root, hlLightCss), "utf8"),
					".light,.rust",
				),
			);
		}
		if (hlDarkCss) {
			supersededCssFiles.add(hlDarkCss);
			cssParts.push(
				scopeCssSelectors(
					readFileSync(join(root, hlDarkCss), "utf8"),
					".navy,.coal",
				),
			);
		}
		if (hlAyuCss) {
			supersededCssFiles.add(hlAyuCss);
			cssParts.push(
				scopeCssSelectors(readFileSync(join(root, hlAyuCss), "utf8"), ".ayu"),
			);
		}
	}

	// Include any custom theme stylesheets (e.g. `theme/pina-*.css`).
	const themeCssFiles = files.filter((f) => /^theme\/.*\.css$/.test(f));
	for (const cssFile of themeCssFiles) {
		addCss(cssFile);
	}
	addCss(printCss, "print");

	const bundledCss = cssParts.join("\n");
	let bundleCssRel = "";
	if (bundledCss.length > 0 && supersededCssFiles.size > 0) {
		bundleCssRel = `assets/book-${shortHash(bundledCss)}.css`;
		writeFileSync(join(root, bundleCssRel), bundledCss);
	}

	// 2. Replace mdBook's massive elasticlunr search index with compact JSON + tiny searcher.
	let searchDataRel: string | null = null;
	let searchJsRel: string | null = null;
	if (searchIndexJs) {
		const compact = buildCompactSearchData(
			readFileSync(join(root, searchIndexJs), "utf8"),
		);
		const jsonStr = JSON.stringify(compact);
		searchDataRel = `assets/search-data-${shortHash(jsonStr)}.json`;
		writeFileSync(join(root, searchDataRel), jsonStr);

		const searcherMin = minifyJs(renderOnDemandSearchScript(searchDataRel));
		searchJsRel = `assets/search-${shortHash(searcherMin)}.js`;
		writeFileSync(join(root, searchJsRel), searcherMin);
	}

	// 3. Optimize `toc-*.js` and merge it directly into the single deferred
	// `assets/book-<hash>.js` bundle so every docs page fetches one JS file.
	let tocBundlePart = "";
	if (tocJs) {
		let tocSource = readFileSync(join(root, tocJs), "utf8");
		if (isPinaBranded) {
			const sidebarHeadExpr =
				`'<div class="pina-sidebar-head"><a class="pina-brand" href="'+(path_to_root===""?"./":path_to_root)+'" aria-label="Pina home">${PINA_LOGO_SVG}<span class="pina-brand-name">pina</span></a><a class="pina-sidebar-link" href="https://github.com/pina-rs/pina">GitHub ↗</a></div>'`;
			// Insert sidebar head right after the active-link loop finishes so `links[0]`
			// still points at the first chapter and the header renders before first paint.
			tocSource = tocSource.replace(
				/(\/\/ Track and set sidebar scroll position)/,
				`if(!this.querySelector('.pina-sidebar-head')){this.insertAdjacentHTML('afterbegin',${sidebarHeadExpr});}\n$1`,
			);
		}
		tocBundlePart = `${minifyJs(tocSource)}\n`;
	}

	const themeColors: Record<string, string> = isPinaBranded
		? {
			light: "#faf8ef",
			rust: "#faf8ef",
			navy: "#121611",
			coal: "#121611",
			ayu: "#121611",
		}
		: {
			light: "#ffffff",
			rust: "#e1e1db",
			coal: "#141617",
			navy: "#161923",
			ayu: "#0f1419",
		};

	const leanBookJs = `${tocBundlePart}${
		minifyJs(renderLeanBookScript(searchJsRel, themeColors))
	}`;
	const bundleJsRel = `assets/book-${shortHash(leanBookJs)}.js`;
	writeFileSync(join(root, bundleJsRel), leanBookJs);

	// 4. Load highlight.js once into a VM sandbox for build-time syntax highlighting.
	const hljs = highlightJs ? loadHljs(root, highlightJs) : null;

	// 5. Rewrite every HTML file in the built book.
	const htmlFiles = files.filter((f) => f.endsWith(".html"));
	let htmlFilesOptimized = 0;

	for (const relHtml of htmlFiles) {
		const absHtml = join(root, relHtml);
		let html = readFileSync(absHtml, "utf8");

		// Standalone non-mdBook pages (e.g. `migrations/flow-interactive.html`)
		// only get conservative inline CSS/JS/comment minification so custom
		// `white-space: pre` blocks remain untouched.
		const isMdBookPage = html.includes("mdbook-body-container") ||
			relHtml === "toc.html";
		if (!isMdBookPage) {
			writeFileSync(absHtml, minifyStandaloneHtml(html));
			htmlFilesOptimized += 1;
			continue;
		}

		const dirRel = posix.dirname(relHtml);
		const prefix = dirRel === "."
			? ""
			: `${"../".repeat(dirRel.split("/").length)}`;
		const homeHref = prefix === "" ? "./" : prefix;

		// Replace only stylesheet `<link>` tags that the bundle actually supersedes,
		// preserving custom `additional-css` outside `theme/` and external sheets.
		if (bundleCssRel) {
			let insertedBundle = false;
			html = html.replace(
				/<link\b[^>]*\brel=["']stylesheet["'][^>]*>\s*/gi,
				(fullTag) => {
					const hrefMatch = /\bhref=["']([^"']+)["']/i.exec(fullTag);
					if (!hrefMatch) {
						return fullTag;
					}
					const relTarget = resolveBookRelativeHref(hrefMatch[1], prefix);
					if (!relTarget || !supersededCssFiles.has(relTarget)) {
						return fullTag;
					}
					if (insertedBundle) {
						return "";
					}
					insertedBundle = true;
					return `<link rel="stylesheet" href="${prefix}${bundleCssRel}">\n`;
				},
			);
		}

		if (relHtml === "toc.html") {
			writeFileSync(absHtml, minifyHtml(html));
			htmlFilesOptimized += 1;
			continue;
		}

		// Replace `<script src="...toc-*.js">` in `<head>` with the single
		// deferred `assets/book-<hash>.js` bundle so download starts immediately
		// without blocking HTML parsing or First Contentful Paint.
		let insertedBookScript = false;
		if (tocJs) {
			html = html.replace(
				new RegExp(
					`<script\\s+src="${escapeRegex(prefix + tocJs)}"\\s*><\\/script>`,
				),
				() => {
					insertedBookScript = true;
					return `<script defer src="${prefix}${bundleJsRel}"></script>`;
				},
			);
		}

		// Drop `window.path_to_searchindex_js` (handled by compact JSON searcher).
		html = html.replace(
			/\s*window\.path_to_searchindex_js\s*=\s*"[^"]*";/g,
			"",
		);

		// Build-time syntax highlight + static copy / boring-toggle buttons.
		const isPrint = relHtml === "print.html";
		const highlighted = highlightAndEnhanceCodeBlocks(html, hljs, !isPrint);
		html = highlighted.html;

		// Remove unused SVG `<template>` elements (play/reset/copy are never cloned at runtime;
		// eye/eye-slash are only needed on pages containing `.boring` code blocks).
		html = html.replace(
			/<template id=(?:"?fa-play"?|"?fa-clock-rotate-left"?|"?fa-copy"?)>[\s\S]*?<\/template>\s*/g,
			"",
		);
		if (!highlighted.hasBoring) {
			html = html.replace(
				/<template id=(?:"?fa-eye"?|"?fa-eye-slash"?)>[\s\S]*?<\/template>\s*/g,
				"",
			);
		}

		// Remove inline `window.playground_copyable = true;` script.
		html = html.replace(
			/<script>\s*window\.playground_copyable\s*=\s*true;\s*<\/script>\s*/g,
			"",
		);

		// Statically SSR `pina.js` chrome so zero layout shift occurs on load.
		if (isPinaBranded) {
			const brandHtml =
				`<a class="pina-brand" href="${homeHref}" aria-label="Pina home">${PINA_LOGO_SVG}<span class="pina-brand-name">pina</span><span class="pina-brand-tag">docs</span></a>`;
			html = html.replace(
				/<h1 class="menu-title">[\s\S]*?<\/h1>/,
				`<h1 class="menu-title">${brandHtml}</h1>`,
			);
			html = html.replace(
				/(<button[^>]*id="mdbook-theme-default_theme"[^>]*>)[^<]*(<\/button>)/,
				"$1Auto$2",
			);
			html = html.replace(
				/(<button[^>]*id="mdbook-theme-light"[^>]*>)[^<]*(<\/button>)/,
				"$1Light$2",
			);
			html = html.replace(
				/(<button[^>]*id="mdbook-theme-navy"[^>]*>)[^<]*(<\/button>)/,
				"$1Dark$2",
			);
			if (!isPrint && !html.includes('class="pina-footer"')) {
				html = html.replace("</main>", `</main>${PINA_FOOTER_HTML}`);
			}
		}

		// Remove obsolete synchronous `<script src="...">` tags and replace `book-*.js`
		// with the single deferred lean runtime bundle.
		const obsoleteScripts = [
			elasticlunrJs,
			markJs,
			searcherJs,
			clipboardJs,
			highlightJs,
			pinaJs,
		].filter((x): x is string => x !== null);

		for (const scriptRel of obsoleteScripts) {
			html = html.replace(
				new RegExp(
					`<script\\s+src="${
						escapeRegex(prefix + scriptRel)
					}"\\s*><\\/script>\\s*`,
				),
				"",
			);
		}
		if (bookJs) {
			html = html.replace(
				new RegExp(
					`<script\\s+src="${
						escapeRegex(prefix + bookJs)
					}"\\s*><\\/script>\\s*`,
				),
				insertedBookScript
					? ""
					: `<script defer src="${prefix}${bundleJsRel}"></script>`,
			);
		}

		writeFileSync(absHtml, minifyHtml(html));
		htmlFilesOptimized += 1;
	}

	// 6. Delete only assets actually superseded by the bundles.
	const toDelete = [
		highlightJs,
		bookJs,
		tocJs,
		clipboardJs,
		elasticlunrJs,
		markJs,
		searcherJs,
		searchIndexJs,
		pinaJs,
		...(bundleCssRel ? Array.from(supersededCssFiles) : []),
	];
	for (const rel of toDelete) {
		if (rel) {
			removeIfExists(join(root, rel));
		}
	}
	removeEmptyDir(join(root, "css"));
	removeEmptyDir(join(root, "theme"));

	return {
		htmlFilesOptimized,
		bundleCssFile: bundleCssRel,
		bundleJsFile: bundleJsRel,
		searchDataFile: searchDataRel,
		searchJsFile: searchJsRel,
	};
}

function escapeRegex(value: string): string {
	return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function main(): number {
	const [bookArgument] = process.argv.slice(2);
	if (!bookArgument) {
		process.stderr.write("usage: optimize-book.ts <bookDir>\n");
		return 2;
	}
	try {
		const stats = optimizeBook(bookArgument);
		process.stdout.write(
			`optimize-book: ${stats.htmlFilesOptimized} HTML pages -> ${stats.bundleCssFile}, ${stats.bundleJsFile}\n`,
		);
		return 0;
	} catch (error) {
		process.stderr.write(
			`optimize-book: ${
				error instanceof Error ? error.message : String(error)
			}\n`,
		);
		return 1;
	}
}

if (
	process.argv[1] !== undefined &&
	import.meta.url === pathToFileURL(resolve(process.argv[1])).href
) {
	process.exitCode = main();
}
