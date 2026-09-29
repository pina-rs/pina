import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

import { dartString, renderDart } from "../src/dart.ts";
import { extractCliModel } from "../src/model.ts";

const here = dirname(fileURLToPath(import.meta.url));
const idls = join(here, "../../../codama/idls");

describe("dartString", () => {
	it("escapes interpolation alongside the JSON escapes Dart shares", () => {
		expect(dartString("plain")).toBe('"plain"');
		expect(dartString("cost $5 ${x}")).toBe('"cost \\$5 \\${x}"');
		expect(dartString('quote " slash \\ line\nend')).toBe(
			'"quote \\" slash \\\\ line\\nend"',
		);
	});
});

describe("renderDart", () => {
	it("keeps IDL docs inert inside generated string literals", () => {
		const root = JSON.parse(
			readFileSync(join(idls, "counter_program.json"), "utf8"),
		);
		const payload = '${(() { throw "PWN"; })()} $pwnVar';
		root.program.docs = [payload];
		for (const instruction of root.program.instructions) {
			instruction.docs = [payload];
		}

		const files = renderDart(extractCliModel(root), {
			packageName: "pina_cli_apps",
			clientBarrel: "package:pina_codama_clients/counter_program.dart",
		});
		const sources = [...files.values()].join("\n");

		expect(sources).toContain('\\${(() { throw \\"PWN\\"; })()} \\$pwnVar');
		expect(sources).not.toMatch(/[^\\]\$\{\(\(\) \{ throw/u);
		expect(sources).not.toMatch(/[^\\]\$pwnVar/u);
	});
});
