import { createHash } from "node:crypto";
import { lstatSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

/** Hash release bytes, including the inventory passed between privileged jobs. */
export function sha256(bytes: Uint8Array): string {
	return createHash("sha256").update(bytes).digest("hex");
}

/** Inventory only regular files with portable, unambiguous release asset names. */
function assetNames(directory: string): string[] {
	const names = readdirSync(directory).sort();

	if (names.length === 0) {
		throw new Error("Release asset inventory is empty");
	}

	for (const name of names) {
		if (
			!/^pina-[A-Za-z0-9._-]+$/.test(name) ||
			!lstatSync(join(directory, name)).isFile()
		) {
			throw new Error(`Invalid release asset: ${name}`);
		}
	}

	return names;
}

/** Seal the complete builder output, with the manifest stored outside the asset directory. */
export function createManifest(directory: string, manifest: string): string {
	const contents = assetNames(directory).map((name) =>
		`${sha256(readFileSync(join(directory, name)))}  ${name}\n`
	).join("");
	writeFileSync(manifest, contents, { flag: "wx" });

	return sha256(Buffer.from(contents));
}

/** Reject replaced inventories, altered bytes, missing assets, and unexpected files. */
export function verifyManifest(
	directory: string,
	manifest: string,
	expectedDigest: string,
): void {
	const bytes = readFileSync(manifest);

	if (
		!/^[a-f0-9]{64}$/.test(expectedDigest) || sha256(bytes) !== expectedDigest
	) {
		throw new Error("Release manifest digest mismatch");
	}

	const contents = assetNames(directory).map((name) =>
		`${sha256(readFileSync(join(directory, name)))}  ${name}\n`
	).join("");

	if (contents !== bytes.toString("utf8")) {
		throw new Error("Release assets differ from the sealed inventory");
	}
}

/** Expose the same inventory checks to attestation and publication jobs. */
function main(): void {
	const [command, directory, manifest, digest] = process.argv.slice(2);

	if (command === "create" && directory && manifest && !digest) {
		process.stdout.write(`${createManifest(directory, manifest)}\n`);
		return;
	}

	if (command === "verify" && directory && manifest && digest) {
		verifyManifest(directory, manifest, digest);
		return;
	}

	throw new Error(
		"Usage: release-assets.ts create|verify ASSET_DIR MANIFEST [EXPECTED_SHA256]",
	);
}

if (
	process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
	main();
}
