import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import type { RootNode } from "codama";
import { visit } from "codama";
import { describe, expect, it } from "vitest";

import { renderRoot, renderVisitor } from "../src/index.js";

const root = {
	kind: "rootNode",
	standard: "codama",
	version: "1.8.0",
	program: {
		kind: "programNode",
		name: "counter",
		publicKey: "11111111111111111111111111111111",
		version: "0.0.0",
		accounts: [],
		instructions: [],
		definedTypes: [],
		pdas: [],
		errors: [],
	},
	additionalPrograms: [],
} as unknown as RootNode;

function fixtureScript(contents: string): string {
	const directory = mkdtempSync(join(tmpdir(), "pina-cpi-visitor-"));
	const script = join(directory, "renderer.mjs");
	writeFileSync(script, contents);
	return script;
}

describe("renderVisitor", () => {
	it("passes the current root to the Pina CLI stdin pipeline", () => {
		const output = mkdtempSync(join(tmpdir(), "pina-cpi-output-"));
		const script = fixtureScript(`
			import { mkdirSync, writeFileSync } from "node:fs";
			import { join } from "node:path";
			let input = "";
			for await (const chunk of process.stdin) input += chunk;
			const args = process.argv.slice(2);
			if (args[0] !== "cpi" || args[1] !== "--stdin" || args[2] !== "--output") process.exit(9);
			mkdirSync(args[3], { recursive: true });
			writeFileSync(join(args[3], "root.json"), input);
		`);

		visit(
			root,
			renderVisitor(output, {
				pinaCommand: [process.execPath, script],
			}),
		);

		expect(JSON.parse(readFileSync(join(output, "root.json"), "utf8"))).toEqual(
			root,
		);
	});

	it("reports process launch and renderer failures", () => {
		expect(() =>
			renderRoot(root, "unused", {
				pinaCommand: [join(tmpdir(), "missing-pina-command")],
			})
		).toThrow("Failed to run the Pina CPI renderer");

		const script = fixtureScript(`
			process.stdout.write("fallback output");
			process.stderr.write("bad\\ninput");
			process.exit(7);
		`);
		expect(() =>
			renderRoot(root, "unused", {
				pinaCommand: [process.execPath, script],
			})
		).toThrow("status 7: bad\\ninput");
	});

	it("uses stdout and unknown status fallbacks in diagnostics", () => {
		const stdoutScript = fixtureScript(`
			process.stdout.write("stdout only");
			process.exit(4);
		`);
		expect(() =>
			renderRoot(root, "unused", {
				pinaCommand: [process.execPath, stdoutScript],
			})
		).toThrow("status 4: stdout only");

		const signalScript = fixtureScript(`process.kill(process.pid, "SIGTERM");`);
		expect(() =>
			renderRoot(root, "unused", {
				pinaCommand: [process.execPath, signalScript],
			})
		).toThrow("status unknown");
	});

	it("resolves the bundled Pina CLI by default", () => {
		expect(() => renderRoot(root, "unused")).toThrow("Pina CPI renderer");
	});
});
