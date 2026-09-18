#!/usr/bin/env node
/**
 * Redirects a generated CPI crate's `pina` dependency at a checkout.
 *
 * The SBF fixture gate keeps the manifest the renderer wrote, so a regression
 * in that manifest still fails the gate; this script rewrites only the `pina`
 * dependency to the in-repo crate and isolates the manifest from any parent
 * workspace.
 *
 * Usage: point-pina-dependency.ts <manifest> <crate-path>
 */

import { readFileSync, writeFileSync } from "node:fs";

const [manifest, cratePath] = process.argv.slice(2);

if (!manifest || !cratePath) {
	console.error("usage: point-pina-dependency.ts <manifest> <crate-path>");
	process.exit(2);
}

const original = readFileSync(manifest, "utf8");
let updated = original.replace(
	/^pina\s*=\s*\{.*\}$/m,
	`pina = { path = "${cratePath}", default-features = false }`,
);

if (!/^pina\s*=/m.test(original)) {
	console.error(`${manifest}: generated manifest declares no pina dependency`);
	process.exit(1);
}

if (!updated.includes("[workspace]")) {
	updated = updated.replace(
		"[dependencies]",
		"# Standalone: never adopt a parent manifest as a workspace.\n[workspace]\n\n[dependencies]",
	);
}

writeFileSync(manifest, updated);
