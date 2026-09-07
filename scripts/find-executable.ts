import { accessSync, constants, lstatSync } from "node:fs";
import { delimiter, join } from "node:path";

function isMissingOrInaccessible(error: unknown): boolean {
	const code = (error as NodeJS.ErrnoException).code;
	return code === "EACCES" || code === "ENOENT" || code === "ENOTDIR";
}

/** Find an executable using only the supplied environment's PATH. */
export function findExecutable(
	program: string,
	env: NodeJS.ProcessEnv,
): string | undefined {
	if (env.PATH === undefined || env.PATH.length === 0) {
		return undefined;
	}

	const extensions = process.platform === "win32"
		? (env.PATHEXT ?? ".COM;.EXE;.BAT;.CMD").split(";")
		: [""];
	for (const directory of env.PATH.split(delimiter)) {
		if (directory.length === 0) {
			continue;
		}
		for (const extension of extensions) {
			const candidate = join(directory, `${program}${extension}`);
			try {
				const metadata = lstatSync(candidate);
				if (!metadata.isFile() && !metadata.isSymbolicLink()) {
					continue;
				}
				if (process.platform !== "win32") {
					accessSync(candidate, constants.X_OK);
				}
				return candidate;
			} catch (error: unknown) {
				if (!isMissingOrInaccessible(error)) {
					throw error;
				}
			}
		}
	}

	return undefined;
}
