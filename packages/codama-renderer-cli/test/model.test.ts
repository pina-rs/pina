import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

import { extractCliModel } from "../src/model.ts";

const here = dirname(fileURLToPath(import.meta.url));
const idls = join(here, "../../../codama/idls");

describe("extractCliModel", () => {
	it("maps the counter program to commands, flags, and fetch seeds", () => {
		const root = JSON.parse(
			readFileSync(join(idls, "counter_program.json"), "utf8"),
		);
		const model = extractCliModel(root);

		expect(model.programSnake).toBe("counter_program");
		expect(model.programKebab).toBe("counter-program");
		expect(model.instructions.map((instruction) => instruction.snake)).toEqual([
			"initialize",
			"increment",
		]);

		const initialize = model.instructions[0]!;
		expect(initialize.args.map((arg) => arg.snake)).toEqual(["bump"]);
		expect(initialize.accounts.map((account) => account.snake)).toEqual([
			"authority",
			"counter",
			"system_program",
		]);

		const counter = initialize.accounts[1]!;
		expect(counter.resolution).toEqual({
			resolution: "pda",
			pascal: "Counter",
			seeds: [{ seed: "account", account: "authority" }],
		});
		const system = initialize.accounts[2]!;
		expect(system.resolution).toEqual({
			resolution: "constant",
			address: "11111111111111111111111111111111",
		});

		expect(model.accounts).toHaveLength(1);
		expect(
			model.accounts[0]!.seeds?.filter((seed) => !seed.constant).map((seed) =>
				seed.snake
			),
		).toEqual(["authority"]);
	});

	it("rejects unsupported argument shapes", () => {
		expect(() =>
			extractCliModel({
				program: {
					name: "broken",
					publicKey: "11111111111111111111111111111111",
					version: "0.0.0",
					instructions: [
						{
							name: "go",
							arguments: [{ name: "amount", type: { kind: "amountTypeNode" } }],
						},
					],
				},
			})
		).toThrow(/not supported/);
	});
});
