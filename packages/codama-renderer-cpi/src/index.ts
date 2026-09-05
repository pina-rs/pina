import { spawnSync } from "node:child_process";
import { createRequire } from "node:module";

import type { RootNode, Visitor } from "codama";
import { rootNodeVisitor } from "codama";

const MAX_OUTPUT_BYTES = 16 * 1024;

export interface RenderOptions {
	/** Override the Pina command and any arguments placed before `cpi`. */
	pinaCommand?: readonly [string, ...string[]];
}

/** Creates a Codama visitor that renders a standalone, `no_std` Pina CPI crate. */
export function renderVisitor(
	outputDir: string,
	options: RenderOptions = {},
): Visitor<void, "rootNode"> {
	return rootNodeVisitor((root: RootNode) => {
		renderRoot(root, outputDir, options);
	});
}

/** Renders one normalized Codama root through the native Pina renderer. */
export function renderRoot(
	root: RootNode,
	outputDir: string,
	options: RenderOptions = {},
): void {
	const [command, ...prefixArgs] = options.pinaCommand ?? defaultPinaCommand();
	const result = spawnSync(
		command,
		[...prefixArgs, "cpi", "--stdin", "--output", outputDir],
		{
			encoding: "utf8",
			input: JSON.stringify(root),
			maxBuffer: MAX_OUTPUT_BYTES,
			windowsHide: true,
		},
	);

	if (result.error) {
		throw new Error(
			`Failed to run the Pina CPI renderer: ${result.error.message}`,
			{
				cause: result.error,
			},
		);
	}

	if (result.status !== 0) {
		const detail = diagnosticText(result.stderr) ||
			diagnosticText(result.stdout);
		const suffix = detail ? `: ${detail}` : "";
		throw new Error(
			`Pina CPI renderer exited with status ${
				result.status ?? "unknown"
			}${suffix}`,
		);
	}
}

function defaultPinaCommand(): readonly [string, ...string[]] {
	const require = createRequire(import.meta.url);
	const launcher = require.resolve("@pina-rs/cli/bin/pina.cjs");

	return [process.execPath, launcher];
}

function diagnosticText(value: string | null): string {
	return (value ?? "")
		.slice(-MAX_OUTPUT_BYTES)
		.trim()
		.replaceAll(
			/[\u0000-\u001f\u007f]/g,
			(character) => JSON.stringify(character).slice(1, -1),
		);
}

export default renderVisitor;
