import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

import { extractCliModel } from "../src/model.ts";
import { renderTypeScript } from "../src/typescript.ts";

const here = dirname(fileURLToPath(import.meta.url));
const idls = join(here, "../../../codama/idls");

describe("renderTypeScript", () => {
	it("emits a runnable commander app importing the generated client", () => {
		const root = JSON.parse(
			readFileSync(join(idls, "counter_program.json"), "utf8"),
		);
		const files = renderTypeScript(extractCliModel(root), {
			clientImportPath: "../../../js/counter_program/src/generated/index",
		});

		const names = [...files.keys()].sort();
		expect(names).toEqual([
			"README.md",
			"package.json",
			"src/client.ts",
			"src/commands/increment.ts",
			"src/commands/initialize.ts",
			"src/context.ts",
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
});
