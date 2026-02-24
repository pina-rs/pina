import { renderVisitor as renderJsVisitor } from "@codama/renderers-js";
import { renderVisitor as renderRustVisitor } from "@codama/renderers-rust";
import { createFromJson } from "codama";
import { readdirSync, readFileSync } from "node:fs";
import { basename, join } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = fileURLToPath(new URL(".", import.meta.url));
const idlsDir = join(__dirname, "idls");
const clientsDir = join(__dirname, "clients");

const programs = readdirSync(idlsDir)
	.filter((f) => f.endsWith(".json"))
	.map((f) => ({
		name: basename(f, ".json"),
		idlPath: join(idlsDir, f),
	}));

console.log(`Found ${programs.length} IDL files to process.\n`);

let hasErrors = false;

for (const program of programs) {
	console.log(`--- ${program.name} ---`);

	const json = readFileSync(program.idlPath, "utf-8");

	let codama;
	try {
		codama = createFromJson(json);
		console.log(`  [ok] IDL parsed successfully`);
	} catch (err) {
		console.error(`  [FAIL] IDL parse error: ${err.message}`);
		hasErrors = true;
		continue;
	}

	// Generate Rust client
	try {
		const rustDir = join(clientsDir, "rust", program.name);
		codama.accept(
			renderRustVisitor(rustDir, {
				formatCode: false,
				deleteFolderBeforeRendering: true,
				anchorTraits: false,
				traitOptions: {
					baseDefaults: [
						"borsh::BorshSerialize",
						"borsh::BorshDeserialize",
						"Clone",
						"Debug",
						"Eq",
						"PartialEq",
					],
					scalarEnumDefaults: [
						"Copy",
						"PartialOrd",
						"Hash",
						"num_derive::FromPrimitive",
					],
				},
			}),
		);
		console.log(
			`  [ok] Rust client generated at clients/rust/${program.name}/`,
		);
	} catch (err) {
		console.error(`  [FAIL] Rust generation error: ${err.message}`);
		hasErrors = true;
	}

	// Generate JavaScript client
	try {
		const jsDir = join(clientsDir, "js", program.name);
		await codama.accept(
			renderJsVisitor(jsDir, {
				formatCode: false,
				deleteFolderBeforeRendering: true,
			}),
		);
		console.log(`  [ok] JS client generated at clients/js/${program.name}/`);
	} catch (err) {
		console.error(`  [FAIL] JS generation error: ${err.message}`);
		hasErrors = true;
	}

	console.log();
}

if (hasErrors) {
	console.error("Some generations failed. See errors above.");
	process.exit(1);
} else {
	console.log("All clients generated successfully.");
}
