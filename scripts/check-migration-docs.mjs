#!/usr/bin/env node
// Parity check between the two migration documentation pages.
//
// `docs/src/migrations/flow-interactive.html` restates the migration system
// in prose that must track `docs/src/migrations/flow.md` and the shipped
// implementation. This script asserts the shared facts so either page
// drifting fails the docs gate with a readable message.

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(
	path.dirname(fileURLToPath(import.meta.url)),
	"..",
);
const flow = readFileSync(
	path.join(root, "docs/src/migrations/flow.md"),
	"utf8",
);
const interactive = readFileSync(
	path.join(root, "docs/src/migrations/flow-interactive.html"),
	"utf8",
);

// Canonical implementation facts. Update these together with the code and
// both pages; a mismatch means the docs are behind the implementation.
const CANONICAL_FACTS = [
	{
		name: "supported migration version widths",
		fragments: ["u8", "u16", "u32"],
	},
	{
		name: "clients with migration envelope enforcement",
		fragments: ["TypeScript", "Dart", "Rust"],
	},
	{
		name: "reserved Migrate instruction discriminator width route",
		fragments: ["0xff", "MigrateContext"],
	},
	{
		name: "inline migration step bound",
		fragments: ["MAX_INLINE_STEPS"],
	},
	{
		name: "lamports per byte rent figure",
		fragments: ["6,960"],
	},
	{
		name: "on-demand account migration executor",
		fragments: ["MigrateAccount"],
	},
	{
		name: "generated client needsMigration helpers",
		fragments: ["needsMigration", "getMigrateInstruction"],
	},
];

const failures = [];
for (const fact of CANONICAL_FACTS) {
	for (
		const page of [
			["flow.md", flow],
			["flow-interactive.html", interactive],
		]
	) {
		const [name, source] = page;
		for (const fragment of fact.fragments) {
			if (!source.includes(fragment)) {
				failures.push(
					`${name} is missing the shared fact "${fact.name}" (expected text: ${fragment})`,
				);
			}
		}
	}
}

// Cross-page consistency: both pages must describe the same version-width
// list. The flow page's encoding line and the interactive page's fact table
// must mention exactly the supported widths and nothing narrower or wider.
const widths = ["u64", "u32", "u16", "u8"];
const flowWidths = widths.filter((width) => flow.includes(width));
const interactiveWidths = widths.filter((width) => interactive.includes(width));
if (
	JSON.stringify(flowWidths) !== JSON.stringify(interactiveWidths)
) {
	failures.push(
		`version width mentions differ: flow.md lists [${
			flowWidths.join(", ")
		}] but flow-interactive.html lists [${interactiveWidths.join(", ")}]`,
	);
}

if (failures.length > 0) {
	console.error("migration docs parity check failed:");
	for (const failure of failures) {
		console.error(`  - ${failure}`);
	}
	process.exit(1);
}
console.log("migration docs parity check passed.");
