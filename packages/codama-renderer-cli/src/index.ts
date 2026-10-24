/**
 * `@pina-rs/codama-renderer-cli` — Codama visitors that render CLI
 * applications from program IDLs.
 *
 * TypeScript CLIs are built with commander on top of a generated
 * `@solana/kit` client; Dart CLIs are built with the `args` package on top
 * of a generated solana_kit client.
 */

import {
	lstatSync,
	mkdirSync,
	readFileSync,
	rmSync,
	statSync,
	writeFileSync,
} from "node:fs";
import { dirname, isAbsolute, relative, resolve, sep } from "node:path";

import type { RootNode, Visitor } from "codama";
import { createFromJson, rootNodeVisitor } from "codama";

import { type DartOptions, renderDart } from "./dart.ts";
import { extractCliModel } from "./model.ts";
import { renderTypeScript, type TypeScriptOptions } from "./typescript.ts";

export { renderDart } from "./dart.ts";
export type { DartOptions } from "./dart.ts";
export { extractCliModel } from "./model.ts";
export type * from "./model.ts";
export { renderTypeScript } from "./typescript.ts";
export type { TypeScriptOptions } from "./typescript.ts";

export interface RenderOptions extends TypeScriptOptions, DartOptions {
	/** Output language of the generated CLI application. */
	language?: "typescript" | "dart";
	/** Remove the destination package before rendering. Defaults to `true`. */
	deleteFolderBeforeRendering?: boolean;
}

/** Preflight every destination before any output is deleted or written. */
function outputDestination(packageFolder: string, file: string): string {
	const root = resolve(packageFolder);
	const destination = resolve(root, file);
	const path = relative(root, destination);

	if (
		isAbsolute(file) || !path || path === ".." || path.startsWith(`..${sep}`) ||
		isAbsolute(path)
	) {
		throw new Error(
			`CLI output path escapes its package: ${JSON.stringify(file)}`,
		);
	}

	// Lexical containment alone does not stop existing symlinks redirecting a
	// write. Check the root and every descendant, including the final file.
	let current = root;

	for (const segment of ["", ...path.split(sep)]) {
		current = resolve(current, segment);

		if (lstatSync(current, { throwIfNoEntry: false })?.isSymbolicLink()) {
			throw new Error(
				`CLI output path contains a symbolic link: ${JSON.stringify(file)}`,
			);
		}
	}

	return destination;
}

/** Creates a Codama visitor that renders a CLI application. */
export function renderVisitor(
	packageFolder: string,
	options: RenderOptions,
): Visitor<Promise<void>, "rootNode"> {
	return rootNodeVisitor(async (root: RootNode) => {
		await renderRoot(root, packageFolder, options);
	});
}

/** Renders one normalized Codama root as a CLI application. */
export async function renderRoot(
	root: RootNode,
	packageFolder: string,
	options: RenderOptions,
): Promise<void> {
	const model = extractCliModel(root);
	const files = options.language === "dart"
		? renderDart(model, {
			packageName: options.packageName,
			clientBarrel: options.clientBarrel,
		})
		: renderTypeScript(model, {
			clientImportPath: options.clientImportPath,
			kitVersion: options.kitVersion,
		});

	const output = [...files].map((
		[file, contents],
	): [string, string] => [outputDestination(packageFolder, file), contents]);

	if (
		options.deleteFolderBeforeRendering !== false &&
		statSync(packageFolder, { throwIfNoEntry: false })?.isDirectory() &&
		options.language !== "dart"
	) {
		rmSync(packageFolder, { force: true, recursive: true });
	}

	for (const [destination, contents] of output) {
		mkdirSync(dirname(destination), { recursive: true });
		writeFileSync(destination, contents, "utf8");
	}
}

/** Renders a CLI directly from a Codama IDL JSON file on disk. */
export async function renderIdlFile(
	idlPath: string,
	packageFolder: string,
	options: RenderOptions,
): Promise<void> {
	const json = readFileSync(idlPath, "utf8");
	const root = createFromJson(json).getRoot();
	await renderRoot(root, packageFolder, options);
}
