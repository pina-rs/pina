import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, join, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";

export interface CargoDependency {
	name: string;
	features: string[];
}

export interface CargoTarget {
	crate_types: string[];
	kind: string[];
	name: string;
}

export interface CargoPackage {
	name: string;
	dependencies: CargoDependency[];
	features: Record<string, string[]>;
	manifest_path: string;
	targets: CargoTarget[];
}

export interface CargoMetadata {
	packages: CargoPackage[];
}

export interface ExampleProgram {
	artifactName: string;
	directory: string;
	manifest: string;
	name: string;
	package: CargoPackage;
}

interface InventoryPolicy {
	excludedPrograms?: string[];
}

interface InventoryOptions {
	env?: NodeJS.ProcessEnv;
	policyFile?: string;
}

const scriptDirectory = dirname(fileURLToPath(import.meta.url));

export const DEFAULT_COMPUTE_UNIT_POLICY = join(
	scriptDirectory,
	"compute-unit-policy.json",
);

export function loadCargoMetadata(
	workspace: string,
	env: NodeJS.ProcessEnv = process.env,
): CargoMetadata {
	const result = spawnSync(
		"cargo",
		["metadata", "--format-version", "1", "--no-deps", "--locked"],
		{
			cwd: workspace,
			env,
			encoding: "utf8",
			stdio: ["ignore", "pipe", "inherit"],
		},
	);

	if (result.error !== undefined) {
		throw result.error;
	}

	if (result.status !== 0) {
		throw new Error(`cargo metadata failed with status ${result.status ?? 1}`);
	}

	return JSON.parse(result.stdout ?? "") as CargoMetadata;
}

export function discoverExamplePrograms(
	metadata: CargoMetadata,
	workspace: string,
	excludedPrograms: readonly string[] = [],
): ExampleProgram[] {
	const excluded = new Set(excludedPrograms);
	const programs: ExampleProgram[] = [];

	for (const package_ of metadata.packages) {
		const manifestParts = relative(workspace, package_.manifest_path).split(
			sep,
		);
		const isExample = manifestParts.length === 3 &&
			manifestParts[0] === "examples" &&
			manifestParts[2] === "Cargo.toml" &&
			Object.hasOwn(package_.features, "bpf-entrypoint") &&
			!excluded.has(package_.name);

		if (!isExample) {
			continue;
		}

		const cdylibTarget = package_.targets.find((target) =>
			target.crate_types.includes("cdylib") || target.kind.includes("cdylib")
		);

		if (cdylibTarget === undefined) {
			throw new Error(`${package_.name} has no cdylib Cargo target`);
		}

		programs.push({
			artifactName: cdylibTarget.name,
			directory: manifestParts[1] ?? package_.name,
			manifest: package_.manifest_path,
			name: package_.name,
			package: package_,
		});
	}

	return programs.toSorted((left, right) =>
		left.name.localeCompare(right.name)
	);
}

export function loadExampleInventory(
	workspace: string,
	options: InventoryOptions = {},
): { metadata: CargoMetadata; programs: ExampleProgram[] } {
	const policyFile = options.policyFile ?? DEFAULT_COMPUTE_UNIT_POLICY;
	const policy = JSON.parse(
		readFileSync(policyFile, "utf8"),
	) as InventoryPolicy;
	const metadata = loadCargoMetadata(workspace, options.env);
	const programs = discoverExamplePrograms(
		metadata,
		workspace,
		policy.excludedPrograms,
	);

	return { metadata, programs };
}
