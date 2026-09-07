import assert from "node:assert/strict";
import {
	chmodSync,
	mkdirSync,
	mkdtempSync,
	rmSync,
	writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { delimiter, join } from "node:path";
import test from "node:test";

import { findExecutable } from "../find-executable.ts";

test("returns undefined when PATH is unavailable", () => {
	assert.equal(findExecutable("cargo-build-sbf", {}), undefined);
});

test("finds an executable without relying on an external which command", () => {
	const root = mkdtempSync(join(tmpdir(), "pina-find-executable-"));
	try {
		const missing = join(root, "missing");
		const bin = join(root, "bin");
		mkdirSync(bin);
		const extension = process.platform === "win32" ? ".CMD" : "";
		const executable = join(bin, `cargo-build-sbf${extension}`);
		writeFileSync(executable, "#!/bin/sh\nexit 0\n");
		chmodSync(executable, 0o755);

		assert.equal(
			findExecutable("cargo-build-sbf", {
				PATH: `${missing}${delimiter}${bin}`,
				PATHEXT: process.platform === "win32" ? ".CMD;.EXE" : undefined,
			}),
			executable,
		);
	} finally {
		rmSync(root, { force: true, recursive: true });
	}
});
