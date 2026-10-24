import {
	mkdirSync,
	mkdtempSync,
	readdirSync,
	readFileSync,
	rmSync,
	symlinkSync,
	writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { createFromJson } from "codama";
import { afterEach, describe, expect, it } from "vitest";

import { renderIdlFile, renderVisitor } from "../src/index.ts";

const fixture = readFileSync(
	join(
		dirname(fileURLToPath(import.meta.url)),
		"../../../codama/idls/counter_program.json",
	),
	"utf8",
);
const temporary: string[] = [];
const options = {
	clientImportPath: "../client/index",
	packageName: "counter_cli",
	clientBarrel: "package:counter_client/counter.dart",
};

/** Keep each attempted write confined to a disposable test directory. */
function workspace(json = fixture) {
	const root = mkdtempSync(join(tmpdir(), "cli-idl-validation-"));
	const output = join(root, "output");
	const idl = join(root, "idl.json");
	temporary.push(root);
	mkdirSync(output);
	writeFileSync(join(output, "sentinel"), "keep");
	writeFileSync(idl, json);

	return { root, output, idl };
}

afterEach(() => {
	for (const root of temporary.splice(0)) {
		rmSync(root, { recursive: true, force: true });
	}
});

describe.each(["typescript", "dart"] as const)(
	"%s IDL boundary",
	(language) => {
		it.each([
			"../../../escaped",
			"dir/name",
			"dir\\name",
			'bad";throw new Error("injected");//',
			"bad';throw 'injected';//",
			"bad\nname",
			"bad*/name",
			"${injected}",
			"",
			"1name",
			"café",
			"class",
			"await",
			"arguments",
			"eval",
			"private",
			"__proto__",
			null,
			123,
			{ identifier: "injected" },
		])("rejects unsafe names %j before changing output", async (name) => {
			for (
				const original of [
					"counterProgram",
					"initialize",
					"authority",
					"bump",
					"counterState",
					"count",
				]
			) {
				const json = fixture.replaceAll(
					`"name": "${original}"`,
					`"name": ${JSON.stringify(name)}`,
				);
				expect(json).not.toBe(fixture);
				const { root, output, idl } = workspace(json);

				await expect(renderIdlFile(idl, output, { ...options, language }))
					.rejects.toThrow(/identifier/i);
				expect(readdirSync(output)).toEqual(["sentinel"]);
				expect(readFileSync(join(output, "sentinel"), "utf8")).toBe("keep");
				expect(readdirSync(root).sort()).toEqual(["idl.json", "output"]);
			}
		});

		it("rejects malformed input before deleting an existing output", async () => {
			const { output, idl } = workspace(
				'{"kind":"rootNode","version":"1.8.0","program":null}',
			);

			await expect(renderIdlFile(idl, output, { ...options, language })).rejects
				.toThrow();
			expect(readdirSync(output)).toEqual(["sentinel"]);
		});

		it("does not follow output symlinks", async () => {
			const { root, output, idl } = workspace();
			const outside = join(root, "outside");
			mkdirSync(outside);
			writeFileSync(join(outside, "sentinel"), "keep");
			symlinkSync(outside, join(output, language === "dart" ? "lib" : "src"));

			await expect(renderIdlFile(idl, output, {
				...options,
				language,
				deleteFolderBeforeRendering: false,
			})).rejects.toThrow(/symbolic link/i);
			expect(readdirSync(outside)).toEqual(["sentinel"]);
			expect(readdirSync(output).sort()).toEqual([
				language === "dart" ? "lib" : "sentinel",
				language === "dart" ? "sentinel" : "src",
			]);
		});

		it("rejects unsafe roots passed through the Codama visitor", async () => {
			const { output } = workspace();
			const codama = createFromJson(
				fixture.replaceAll('"initialize"', '"../../../escaped"'),
			);

			await expect(
				codama.accept(renderVisitor(output, { ...options, language })),
			)
				.rejects.toThrow(/identifier/i);
			expect(readdirSync(output)).toEqual(["sentinel"]);
		});

		it.each(["root", "file"])(
			"rejects a symbolic link at the output %s",
			async (target) => {
				const { root, output, idl } = workspace();
				const outside = join(root, "outside");
				const main = language === "dart"
					? "lib/src/counter_program/main.dart"
					: "src/main.ts";
				writeFileSync(outside, "keep");

				if (target === "root") {
					rmSync(output, { recursive: true });
					const directory = join(root, "directory");
					mkdirSync(directory);
					symlinkSync(directory, output);
				} else {
					mkdirSync(dirname(join(output, main)), { recursive: true });
					symlinkSync(outside, join(output, main));
				}

				await expect(renderIdlFile(idl, output, { ...options, language }))
					.rejects.toThrow(/symbolic link/i);
				expect(readFileSync(outside, "utf8")).toBe("keep");
			},
		);

		it("renders the existing IDL", async () => {
			const { output, idl } = workspace();

			await renderIdlFile(idl, output, { ...options, language });
			const main = language === "dart"
				? "lib/src/counter_program/main.dart"
				: "src/main.ts";
			expect(readFileSync(join(output, main), "utf8")).toContain(
				"counter-program-cli",
			);
		});
	},
);
