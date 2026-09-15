import {
	mkdirSync,
	mkdtempSync,
	readdirSync,
	readFileSync,
	rmSync,
	writeFileSync,
} from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { afterAll, describe, expect, it } from "vitest";

import { extractCliModel } from "../src/model.ts";
import { renderTypeScript } from "../src/typescript.ts";

const here = dirname(fileURLToPath(import.meta.url));
const idls = join(here, "../../../codama/idls");
const tempRoots: string[] = [];

function renderCounterProgram(): Map<string, string> {
	const root = JSON.parse(
		readFileSync(join(idls, "counter_program.json"), "utf8"),
	);
	return renderTypeScript(extractCliModel(root), {
		clientImportPath: "../../../js/counter_program/src/generated/index",
	});
}

afterAll(() => {
	for (const root of tempRoots) {
		rmSync(root, { recursive: true, force: true });
	}
});

describe("renderTypeScript", () => {
	it("emits a runnable commander app importing the generated client", () => {
		const files = renderCounterProgram();

		const names = [...files.keys()].sort();
		expect(names).toEqual([
			"README.md",
			"package.json",
			"src/client.ts",
			"src/commands/increment.ts",
			"src/commands/initialize.ts",
			"src/context.ts",
			"src/endpoint.ts",
			"src/fetch.ts",
			"src/main.ts",
			"tsconfig.json",
		]);

		const initialize = files.get("src/commands/initialize.ts")!;
		expect(initialize).toContain("getInitializeInstructionAsync");
		expect(initialize).toContain('.requiredOption("--bump <bump>"');
		expect(initialize).toContain("context.payer");

		const main = files.get("src/main.ts")!;
		expect(main).toContain("program.addCommand(initializeCommand);");
		expect(main).toContain("program.addCommand(fetchCommand);");

		const client = files.get("src/client.ts")!;
		expect(client).toContain(
			'from "../../../js/counter_program/src/generated/index"',
		);
	});

	it("declares every external import in package.json", () => {
		const files = renderCounterProgram();
		const manifest = JSON.parse(files.get("package.json")!) as {
			dependencies: Record<string, string>;
			devDependencies: Record<string, string>;
		};
		expect(manifest.dependencies["@cliffy/command"]).toBeUndefined();
		expect(manifest.dependencies.commander).toBeDefined();

		const declared = new Set([
			...Object.keys(manifest.dependencies),
			...Object.keys(manifest.devDependencies),
		]);
		const imported = new Set<string>();
		for (const [name, source] of files) {
			if (name === "package.json") continue;
			for (
				const match of source.matchAll(/(?:from|import)\s+["']([^"']+)["']/g)
			) {
				const specifier = match[1];
				if (specifier.startsWith(".") || specifier.startsWith("node:")) {
					continue;
				}
				const scope = specifier.startsWith("@")
					? specifier.split("/").slice(0, 2).join("/")
					: specifier.split("/")[0];
				imported.add(scope);
			}
		}
		for (const scope of imported) {
			expect(declared.has(scope), `undeclared import: ${scope}`).toBe(true);
		}
	});

	it("treats only exact loopback hosts as local for plaintext http", async () => {
		const files = renderCounterProgram();
		const tempRoot = mkdtempSync(join(here, ".tmp-render-"));
		tempRoots.push(tempRoot);
		mkdirSync(join(tempRoot, "src"), { recursive: true });
		writeFileSync(
			join(tempRoot, "src", "endpoint.ts"),
			files.get("src/endpoint.ts")!,
		);
		const module = await import(
			pathToFileURL(join(tempRoot, "src", "endpoint.ts")).href
		);

		for (
			const endpoint of [
				"https://rpc.example.com",
				"http://localhost",
				"http://localhost:8899",
				"http://127.0.0.1:8899",
				"http://[::1]:8899",
			]
		) {
			expect(module.endpointIsSecure(endpoint), endpoint).toBe(true);
		}
		for (
			const endpoint of [
				"http://localhost.evil.com",
				"http://127.0.0.1.evil.com",
				"http://127.0.0.1@evil.com",
				"http://user:password@localhost",
				"http://evil.com",
				"http://example.com/localhost",
				"http://192.168.0.10",
				"ftp://localhost",
			]
		) {
			expect(module.endpointIsSecure(endpoint), endpoint).toBe(false);
		}
	});

	it("wires ownership and validation into the emitted runtime", () => {
		const files = renderCounterProgram();
		const fetch = files.get("src/fetch.ts")!;
		expect(fetch).toContain(
			"account.programAddress !== context.programAddress",
		);
		expect(fetch).toContain("import { CliContext, CliError,");

		const context = files.get("src/context.ts")!;
		expect(context).toContain("endpointIsSecure");
		expect(context).toContain('pubkey("--program-id", options.programId)');
	});
});
