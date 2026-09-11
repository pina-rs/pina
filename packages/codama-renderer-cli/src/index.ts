/**
 * `@pina-rs/codama-renderer-cli` — Codama visitors that render CLI
 * applications from program IDLs.
 *
 * TypeScript CLIs are built with commander on top of a generated
 * `@solana/kit` client; Dart CLIs are built with the `args` package on top
 * of a generated solana_kit client.
 */

import {
	mkdirSync,
	readFileSync,
	rmSync,
	statSync,
	writeFileSync,
} from "node:fs";
import { dirname, join } from "node:path";

import type { RootNode, Visitor } from "codama";
import { rootNodeVisitor } from "codama";

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
	if (
		options.deleteFolderBeforeRendering !== false &&
		statSync(packageFolder, { throwIfNoEntry: false })?.isDirectory() &&
		options.language !== "dart"
	) {
		rmSync(packageFolder, { force: true, recursive: true });
	}

	const model = extractCliModel(root as never);
	const files = options.language === "dart"
		? renderDart(model, {
			packageName: options.packageName,
			clientBarrel: options.clientBarrel,
		})
		: renderTypeScript(model, {
			clientImportPath: options.clientImportPath,
			kitVersion: options.kitVersion,
		});

	for (const [relative, contents] of files) {
		const destination = join(packageFolder, relative);
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
	const root = JSON.parse(json) as RootNode;
	await renderRoot(root, packageFolder, options);
}
