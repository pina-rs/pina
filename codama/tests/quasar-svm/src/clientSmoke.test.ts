import { readdirSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { describe, expect, test } from "vitest";

const TEST_DIR = dirname(fileURLToPath(import.meta.url));
const CLIENTS_DIR = resolve(TEST_DIR, "../../../clients/js");

async function importModule(path: string) {
	return await import(/* @vite-ignore */ pathToFileURL(path).href);
}

describe("generated Codama JS clients smoke", () => {
	const clients = readdirSync(CLIENTS_DIR, { withFileTypes: true })
		.filter((entry) => entry.isDirectory())
		.filter((entry) => entry.name !== "node_modules")
		.map((entry) => entry.name)
		.sort();

	test.each(clients)(
		"%s generated entrypoints import cleanly",
		async (client) => {
			const generatedDir = resolve(CLIENTS_DIR, client, "src/generated");
			const indexModule = await importModule(resolve(generatedDir, "index.ts"));
			const programsModule = await importModule(
				resolve(generatedDir, "programs/index.ts"),
			);

			expect(Object.keys(indexModule).length).toBeGreaterThan(0);
			expect(Object.keys(programsModule).length).toBeGreaterThan(0);
		},
	);
});
