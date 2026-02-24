import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { expect, test } from "vitest";

import {
	rootNodeFromPina,
	rootNodeFromPinaWithoutDefaultVisitor,
} from "../src";

function loadTodoFixture(): string {
	const testDir = fileURLToPath(new URL(".", import.meta.url));
	return readFileSync(
		resolve(testDir, "../../../idls/todo_program.json"),
		"utf8",
	);
}

test("it creates root nodes from a Pina IDL JSON string", () => {
	const root = rootNodeFromPina(loadTodoFixture());

	expect(root.kind).toBe("rootNode");
	expect(root.program.name).toBe("todoProgram");
	expect(root.program.publicKey).toBe(
		"Fc5A5xvNQ6w7kn2P7FpC18JNpDutLCRa14Q6gttxyPjd",
	);
	expect(root.program.accounts[0]?.size).toBeDefined();
});

test("it creates root nodes from parsed Pina IDL objects", () => {
	const parsed = JSON.parse(loadTodoFixture()) as unknown;
	const root = rootNodeFromPina(
		parsed as Parameters<typeof rootNodeFromPina>[0],
	);

	expect(root.kind).toBe("rootNode");
	expect(root.program.name).toBe("todoProgram");
	expect(root.program.accounts[0]?.size).toBeDefined();
});

test("it can skip the default visitor", () => {
	const parsed = JSON.parse(loadTodoFixture()) as Record<string, unknown>;

	const root = rootNodeFromPinaWithoutDefaultVisitor(
		parsed as Parameters<typeof rootNodeFromPinaWithoutDefaultVisitor>[0],
	);

	expect(root.kind).toBe("rootNode");
	expect(root.program.accounts[0]?.size).toBeUndefined();
});

test("it throws on invalid input", () => {
	expect(() =>
		rootNodeFromPinaWithoutDefaultVisitor('{"kind":"notRoot"}' as string)
	).toThrow(
		"Expected node of kind [rootNode]",
	);
});
