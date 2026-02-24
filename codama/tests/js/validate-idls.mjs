import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { getValidationItemsVisitor, visit } from "codama";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, "../../..");
const examplesDir = path.join(rootDir, "examples");
const idlDir = path.join(rootDir, "codama", "idls");

const exampleNames = readdirSync(examplesDir, { withFileTypes: true })
	.filter((entry) => entry.isDirectory())
	.map((entry) => entry.name)
	.sort();

const idlFiles = readdirSync(idlDir)
	.filter((entry) => entry.endsWith(".json"))
	.sort();

assert.ok(exampleNames.length > 0, "Expected at least one example program.");
assert.equal(
	idlFiles.length,
	exampleNames.length,
	`Expected IDL count to match example count (${exampleNames.length}), got ${idlFiles.length}.`,
);

for (const exampleName of exampleNames) {
	const filename = `${exampleName}.json`;
	assert.ok(
		idlFiles.includes(filename),
		`Missing generated IDL fixture for example ${exampleName}: ${filename}`,
	);
}

for (const filename of idlFiles) {
	const idlPath = path.join(idlDir, filename);
	const idl = JSON.parse(readFileSync(idlPath, "utf8"));

	assert.equal(idl.kind, "rootNode", `${filename}: expected kind=rootNode`);
	assert.equal(idl.standard, "codama", `${filename}: expected standard=codama`);
	assert.equal(
		typeof idl.program?.name,
		"string",
		`${filename}: missing program.name`,
	);
	assert.notEqual(
		idl.program.name.length,
		0,
		`${filename}: empty program.name`,
	);

	const validationItems = visit(idl, getValidationItemsVisitor());
	const validationErrors = validationItems.filter((item) =>
		item.level === "error"
	);

	assert.equal(
		validationErrors.length,
		0,
		`${filename}: Codama validation errors\n${
			validationErrors.map((item) => `- ${item.message}`).join("\n")
		}`,
	);

	const instructionNodes = idl.program?.instructions ?? [];

	for (const instruction of instructionNodes) {
		assert.ok(
			Array.isArray(instruction.discriminators) &&
				instruction.discriminators.length > 0,
			`${filename}: instruction ${instruction.name} is missing discriminator metadata`,
		);
	}

	for (const account of idl.program?.accounts ?? []) {
		assert.ok(
			Array.isArray(account.discriminators) &&
				account.discriminators.length > 0,
			`${filename}: account ${account.name} is missing discriminator metadata`,
		);
	}
}

console.log(
	`Validated ${idlFiles.length} example IDL fixture(s) with Codama JS.`,
);
