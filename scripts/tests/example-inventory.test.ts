import assert from "node:assert/strict";
import test from "node:test";

import {
	type CargoMetadata,
	discoverExamplePrograms,
} from "../example-inventory.ts";

const metadata: CargoMetadata = {
	packages: [
		{
			name: "included-program",
			dependencies: [],
			features: { "bpf-entrypoint": [] },
			manifest_path: "/workspace/examples/included_program/Cargo.toml",
			targets: [{
				crate_types: ["cdylib", "lib"],
				kind: ["cdylib", "lib"],
				name: "custom_artifact",
			}],
		},
		{
			name: "excluded-program",
			dependencies: [],
			features: { "bpf-entrypoint": [] },
			manifest_path: "/workspace/examples/excluded_program/Cargo.toml",
			targets: [{
				crate_types: ["cdylib"],
				kind: ["cdylib"],
				name: "excluded_program",
			}],
		},
		{
			name: "workspace-library",
			dependencies: [],
			features: { "bpf-entrypoint": [] },
			manifest_path: "/workspace/crates/workspace-library/Cargo.toml",
			targets: [{
				crate_types: ["lib"],
				kind: ["lib"],
				name: "workspace_library",
			}],
		},
	],
};

test("example inventory applies exclusions and uses the cdylib target name", () => {
	const programs = discoverExamplePrograms(
		metadata,
		"/workspace",
		["excluded-program"],
	);

	assert.equal(programs.length, 1);
	assert.equal(programs[0]?.name, "included-program");
	assert.equal(programs[0]?.directory, "included_program");
	assert.equal(programs[0]?.artifactName, "custom_artifact");
});
