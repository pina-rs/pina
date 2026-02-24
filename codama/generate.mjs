import { renderVisitor as renderJsVisitor } from "@codama/renderers-js";
import { createFromJson } from "codama";
import { execFileSync } from "node:child_process";
import { readdirSync, readFileSync } from "node:fs";
import { basename, join } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = fileURLToPath(new URL(".", import.meta.url));
const workspaceRoot = join(__dirname, "..");
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

console.log("--- rust clients ---");
try {
	const args = [
		"run",
		"-p",
		"pina_codama_renderer",
		"--",
		"--output",
		join(clientsDir, "rust"),
		...programs.flatMap((program) => ["--idl", program.idlPath]),
	];
	execFileSync("cargo", args, {
		cwd: workspaceRoot,
		encoding: "utf-8",
		stdio: "pipe",
	});
	for (const program of programs) {
		console.log(
			`  [ok] Rust client generated at clients/rust/${program.name}/`,
		);
	}
} catch (err) {
	const stderr = typeof err?.stderr === "string" ? err.stderr.trim() : "";
	const stdout = typeof err?.stdout === "string" ? err.stdout.trim() : "";
	const details = stderr || stdout || err.message;
	console.error(`  [FAIL] Rust generation error: ${details}`);
	hasErrors = true;
}
console.log();

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
