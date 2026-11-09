//! Live smoke: prepare + `codama-renderers-rust-solana-kit@0.2.1` on a real
//! Pina IDL. Catches prepare regressions that `fake_npx` cannot see.

use std::fs;
use std::path::Path;
use std::process::Command;

#[test]
fn prepare_and_kit_renderer_accept_counter_program_idl() {
	// Windows CI portability runs this crate with the image Node (currently 22).
	// The Kit renderer package engines field requires >= 24; skip there rather
	// than fail a Rust-portability job that does not install that toolchain.
	// Linux/devenv (Node 24) still executes the smoke.
	let Some(version) = node_major() else {
		eprintln!("skipping live rust-kit render: node is not on PATH");
		return;
	};
	if version < 24 {
		eprintln!("skipping live rust-kit render: Node >= 24 required (found {version})");
		return;
	}

	let temp = tempfile::TempDir::new().expect("temp dir");
	let pkg = temp.path().join("pkg");
	fs::create_dir_all(&pkg).expect("pkg dir");
	write_package_json(&pkg);
	npm_install(&pkg);

	let idl = Path::new(env!("CARGO_MANIFEST_DIR"))
		.join("../../codama/idls/counter_program.json")
		.canonicalize()
		.expect("counter_program.json");
	// Script must live under `pkg` so Node resolves the installed packages.
	let script = pkg.join("render.mjs");
	fs::write(&script, RENDER_SCRIPT).expect("write render script");

	let output = Command::new("node")
		.current_dir(&pkg)
		.arg(&script)
		.arg(&idl)
		.output()
		.expect("run live rust-kit render");
	assert!(
		output.status.success(),
		"live rust-kit render failed:\nstdout:\n{}\nstderr:\n{}",
		String::from_utf8_lossy(&output.stdout),
		String::from_utf8_lossy(&output.stderr)
	);
	let stdout = String::from_utf8_lossy(&output.stdout);
	assert!(
		stdout.contains("OK"),
		"expected OK from renderer, got:\n{stdout}"
	);
}

fn node_major() -> Option<u32> {
	let output = Command::new("node")
		.arg("-p")
		.arg("process.versions.node")
		.output()
		.ok()?;
	if !output.status.success() {
		return None;
	}
	String::from_utf8_lossy(&output.stdout)
		.trim()
		.split('.')
		.next()?
		.parse()
		.ok()
}

fn write_package_json(pkg: &Path) {
	fs::write(
		pkg.join("package.json"),
		r#"{
  "name": "pina-rust-kit-live-render",
  "private": true,
  "type": "module",
  "dependencies": {
    "codama": "1.11.0",
    "codama-renderers-rust-solana-kit": "0.2.1"
  }
}
"#,
	)
	.expect("package.json");
}

fn npm_install(pkg: &Path) {
	let output = Command::new("npm")
		.current_dir(pkg)
		.args(["install", "--ignore-scripts", "--no-fund", "--no-audit"])
		.output()
		.expect("npm install");
	assert!(
		output.status.success(),
		"npm install failed:\n{}",
		String::from_utf8_lossy(&output.stderr)
	);
}

const RENDER_SCRIPT: &str = r#"
import { readFileSync, mkdirSync, rmSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { visit } from "codama";
import renderVisitor from "codama-renderers-rust-solana-kit";

function prepareRustKitRoot(root) {
	if (root && typeof root === "object" && root.kind === "rootNode" && root.version !== "1.0.0") {
		root.version = "1.8.0";
	}
	return transformRustKitNode(root, "root");
}

function transformRustKitNode(node, ctx) {
	if (Array.isArray(node)) {
		return node.map((item) => transformRustKitNode(item, ctx)).filter((item) => item !== undefined);
	}
	if (!node || typeof node !== "object") {
		return node;
	}
	if (node.kind === "pluginNode") {
		return undefined;
	}
	const nextCtx = ["instructionNode", "accountNode", "eventNode"].includes(node.kind)
		? node.kind
		: ctx;
	const out = {};
	for (const [key, value] of Object.entries(node)) {
		if (key === "plugins") {
			continue;
		}
		if (
			node.kind === "structFieldTypeNode"
			&& (key === "defaultValue" || key === "defaultValueStrategy")
		) {
			continue;
		}
		if (key === "discriminators" && Array.isArray(value) && nextCtx === "instructionNode") {
			out[key] = value.map((discriminator) => {
				if (discriminator?.kind === "constantDiscriminatorNode") {
					return {
						kind: "fieldDiscriminatorNode",
						name: "discriminator",
						offset: discriminator.offset ?? 0,
					};
				}
				return transformRustKitNode(discriminator, nextCtx);
			});
			continue;
		}
		if (
			key === "discriminators"
			&& Array.isArray(value)
			&& (nextCtx === "accountNode" || nextCtx === "eventNode")
		) {
			out[key] = value.map((discriminator) =>
				convertNumericAccountDiscriminator(discriminator, nextCtx)
			);
			continue;
		}
		const transformed = transformRustKitNode(value, nextCtx);
		if (transformed !== undefined) {
			out[key] = transformed;
		}
	}
	return out;
}

function convertNumericAccountDiscriminator(discriminator, ctx) {
	const constant = discriminator?.constant;
	const type = constant?.type;
	if (
		discriminator?.kind === "constantDiscriminatorNode"
		&& constant?.kind === "constantValueNode"
		&& type?.kind === "numberTypeNode"
		&& constant.value?.kind === "numberValueNode"
	) {
		const width = { u8: 1, u16: 2, u32: 4 }[type.format];
		if (width) {
			const number = Number(constant.value.number);
			const bytes = Buffer.alloc(width);
			const bigEndian = type.endian === "be";
			if (width === 1) {
				bytes.writeUInt8(number);
			} else if (width === 2) {
				if (bigEndian) {
					bytes.writeUInt16BE(number);
				} else {
					bytes.writeUInt16LE(number);
				}
			} else if (bigEndian) {
				bytes.writeUInt32BE(number);
			} else {
				bytes.writeUInt32LE(number);
			}
			return {
				kind: "constantDiscriminatorNode",
				offset: discriminator.offset ?? 0,
				constant: {
					kind: "constantValueNode",
					type: { kind: "bytesTypeNode" },
					value: {
						kind: "bytesValueNode",
						data: bytes.toString("hex"),
						encoding: "base16",
					},
				},
			};
		}
	}
	return transformRustKitNode(discriminator, ctx);
}

const idl = JSON.parse(readFileSync(process.argv[2], "utf8"));
const prepared = prepareRustKitRoot(structuredClone(idl));
const out = join(tmpdir(), "pina-rust-kit-live-" + Math.random().toString(16).slice(2));
rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });
visit(prepared, renderVisitor(out));
const files = readdirSync(out);
if (files.length === 0) {
	throw new Error("renderer wrote no files");
}
console.log("OK", files.length);
"#;
