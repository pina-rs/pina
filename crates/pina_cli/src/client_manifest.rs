//! Dependency declarations for scaffolded Rust client crates.
//!
//! The Rust, CPI, and Rust CLI renderers scaffold manifests that inherit every
//! dependency with `{ workspace = true }`. That is right inside a workspace
//! whose `[workspace.dependencies]` declares them, such as this repository.
//! A project created with `pina init` has no such table, so a freshly
//! scaffolded client there would fail to parse. This module rewrites those
//! manifests to name concrete requirements and to form their own workspace.

use std::path::Path;

use toml::Table;
use toml::Value;

use crate::error::CodamaError;

/// Requirements for every crate a scaffolded client inherits, pinned to the
/// ranges this Pina release is built and tested with: `(crate, version,
/// features)`. Default features are off, as in this repository's workspace,
/// so each client's own `default-features` setting decides.
pub(crate) const CLIENT_DEPENDENCY_REQUIREMENTS: &[(&str, &str, &[&str])] = &[
	("bs58", "^0.5.1", &[]),
	("clap", "^4", &["derive", "std"]),
	("num-derive", "^0.5", &[]),
	("num-traits", "^0.2", &[]),
	("serde_json", "^1", &["std"]),
	("solana-account-info", "^3", &[]),
	("solana-cpi", "^3", &[]),
	("solana-instruction", "^3", &[]),
	("solana-program-error", "^3", &[]),
	("solana-pubkey", "^4", &[]),
	// A fresh resolution of `^4` can select an older RPC client whose
	// transitive `wincode` versions conflict, so pin the tested minimum.
	("solana-rpc-client", "^4.2.2", &[]),
	("solana-sdk", "^4.0.1", &[]),
	("thiserror", "^2", &[]),
];

/// Whether a client crate at `crate_dir` can inherit `pina` from an enclosing
/// workspace's `[workspace.dependencies]`.
pub(crate) fn inherits_workspace_dependencies(crate_dir: &Path) -> bool {
	let Some(start) = crate_dir
		.ancestors()
		.find_map(|candidate| std::fs::canonicalize(candidate).ok())
	else {
		return false;
	};
	start
		.ancestors()
		.skip(1)
		.find_map(|directory| {
			let contents = std::fs::read_to_string(directory.join("Cargo.toml")).ok()?;
			let manifest = contents.parse::<Table>().ok()?;
			manifest.get("workspace").cloned()
		})
		.is_some_and(|workspace| {
			workspace
				.get("dependencies")
				.and_then(Value::as_table)
				.is_some_and(|dependencies| dependencies.contains_key("pina"))
		})
}

/// Rewrite the scaffolded manifest in `crate_dir` to name concrete
/// dependency requirements and declare its own workspace.
pub(crate) fn make_manifest_standalone(crate_dir: &Path) -> Result<(), CodamaError> {
	let path = crate_dir.join("Cargo.toml");
	let error = |message: String| {
		CodamaError::ClientManifest {
			path: path.clone(),
			message,
		}
	};
	let contents = std::fs::read_to_string(&path).map_err(|source| error(source.to_string()))?;
	let rewritten = standalone_manifest(&contents).map_err(error)?;
	std::fs::write(&path, rewritten).map_err(|source| error(source.to_string()))
}

fn standalone_manifest(contents: &str) -> Result<String, String> {
	let manifest = contents
		.parse::<Table>()
		.map_err(|source| source.to_string())?;
	let mut output = String::with_capacity(contents.len());
	let mut section = String::new();
	for line in contents.lines() {
		let trimmed = line.trim();
		if trimmed.starts_with('[') {
			trimmed.clone_into(&mut section);
		}
		let rewritten = (section == "[dependencies]")
			.then(|| inherited_dependency_line(trimmed))
			.transpose()?
			.flatten();
		output.push_str(rewritten.as_deref().unwrap_or(line));
		output.push('\n');
	}
	// An empty `[workspace]` keeps an enclosing workspace that does not list
	// this crate from claiming it.
	if !manifest.contains_key("workspace") {
		output.push_str("\n[workspace]\n");
	}
	Ok(output)
}

/// Rewrite one `name = { workspace = true, ... }` dependency line, or return
/// `None` for any other line.
fn inherited_dependency_line(line: &str) -> Result<Option<String>, String> {
	let Ok(entry) = line.parse::<Table>() else {
		return Ok(None);
	};
	let Some((name, Value::Table(inherited))) = entry.into_iter().next() else {
		return Ok(None);
	};
	if inherited.get("workspace").and_then(Value::as_bool) != Some(true) {
		return Ok(None);
	}
	let dependency = concrete_dependency(&name, &inherited)?;
	Ok(Some(format!("{name} = {}", inline_table(&dependency))))
}

/// Render a dependency table inline, with its requirement first.
fn inline_table(table: &Table) -> String {
	let mut keys = table.keys().collect::<Vec<_>>();
	keys.sort_by_key(|key| {
		match key.as_str() {
			"version" => 0,
			"default-features" => 1,
			"features" => 2,
			_ => 3,
		}
	});
	let entries = keys
		.into_iter()
		.map(|key| format!("{key} = {}", table[key]))
		.collect::<Vec<_>>()
		.join(", ");
	format!("{{ {entries} }}")
}

fn concrete_dependency(name: &str, inherited: &Table) -> Result<Table, String> {
	let (version, features): (&str, &[&str]) = if name == "pina" {
		(env!("CARGO_PKG_VERSION"), &[])
	} else {
		CLIENT_DEPENDENCY_REQUIREMENTS
			.iter()
			.find(|(known, ..)| *known == name)
			.map(|(_, version, features)| (*version, *features))
			.ok_or_else(|| format!("no known requirement for inherited dependency `{name}`"))?
	};
	let mut dependency = Table::new();
	dependency.insert("version".to_owned(), Value::String(version.to_owned()));
	dependency.insert("default-features".to_owned(), Value::Boolean(false));
	if !features.is_empty() {
		dependency.insert(
			"features".to_owned(),
			Value::Array(
				features
					.iter()
					.map(|feature| Value::String((*feature).to_owned()))
					.collect(),
			),
		);
	}

	for (key, value) in inherited {
		match key.as_str() {
			"workspace" => {}
			// Inherited features add to the workspace's, as Cargo does.
			"features" => {
				let mut features = dependency
					.get("features")
					.and_then(Value::as_array)
					.cloned()
					.unwrap_or_default();
				for feature in value.as_array().into_iter().flatten() {
					if !features.contains(feature) {
						features.push(feature.clone());
					}
				}
				dependency.insert("features".to_owned(), Value::Array(features));
			}
			_ => {
				dependency.insert(key.clone(), value.clone());
			}
		}
	}
	Ok(dependency)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn inherited_dependencies_become_concrete_requirements() {
		let manifest = standalone_manifest(
			r#"[package]
name = "demo-client"
version = "0.0.0"

[dependencies]
pina = { workspace = true, features = ["compact"] }
solana-pubkey = { workspace = true, default-features = true, features = ["curve25519"] }
demo = { path = "../demo" }
serde = "1"
"#,
		)
		.unwrap_or_else(|error| panic!("rewrite manifest: {error}"));
		let parsed = manifest
			.parse::<Table>()
			.unwrap_or_else(|error| panic!("reparse manifest: {error}"));
		let dependencies = parsed["dependencies"].as_table().expect("dependencies");

		let pina = dependencies["pina"].as_table().expect("pina");
		assert_eq!(pina["version"].as_str(), Some(env!("CARGO_PKG_VERSION")));
		assert_eq!(pina["default-features"].as_bool(), Some(false));
		assert_eq!(pina["features"].as_array().map(Vec::len), Some(1));

		let pubkey = dependencies["solana-pubkey"].as_table().expect("pubkey");
		assert_eq!(pubkey["version"].as_str(), Some("^4"));
		assert_eq!(pubkey["default-features"].as_bool(), Some(true));
		assert!(pubkey.get("workspace").is_none());

		assert_eq!(dependencies["demo"]["path"].as_str(), Some("../demo"));
		assert_eq!(dependencies["serde"].as_str(), Some("1"));
		let clap = standalone_manifest("[dependencies]\nclap = { workspace = true }\n")
			.unwrap_or_else(|error| panic!("rewrite clap: {error}"));
		assert!(clap.contains(r#"features = ["derive", "std"]"#), "{clap}");
		assert!(parsed["workspace"].as_table().is_some_and(Table::is_empty));
	}

	#[test]
	fn existing_workspaces_and_multi_line_values_are_left_alone() {
		let manifest = standalone_manifest(
			"[dependencies]\nthiserror = { workspace = true, optional = true }\nlong = { version = \
			 \"1\", features = [\n\t\"a\",\n] }\n\n[workspace]\nmembers = []\n",
		)
		.unwrap_or_else(|error| panic!("rewrite manifest: {error}"));
		assert!(
			manifest.contains(
				r#"thiserror = { version = "^2", default-features = false, optional = true }"#
			),
			"{manifest}"
		);
		assert!(manifest.contains("long = { version = \"1\", features = [\n"));
		assert_eq!(manifest.matches("[workspace]").count(), 1);
	}

	#[test]
	fn unknown_inherited_dependencies_are_reported() {
		let error = standalone_manifest("[dependencies]\nmystery = { workspace = true }\n")
			.expect_err("an unknown crate has no requirement to substitute");
		assert!(error.contains("mystery"));
		assert!(standalone_manifest("not = [valid").is_err());
	}

	#[test]
	fn workspace_inheritance_requires_a_pina_workspace_dependency() {
		let temp = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
		let client = temp.path().join("clients/rust/demo");
		std::fs::create_dir_all(&client).unwrap_or_else(|error| panic!("mkdir: {error}"));
		assert!(!inherits_workspace_dependencies(&client));
		assert!(!inherits_workspace_dependencies(Path::new("")));

		std::fs::write(temp.path().join("Cargo.toml"), "[workspace]\n")
			.unwrap_or_else(|error| panic!("write root: {error}"));
		assert!(!inherits_workspace_dependencies(&client));

		std::fs::write(
			temp.path().join("Cargo.toml"),
			"[workspace]\n[workspace.dependencies]\npina = \"0.22\"\n",
		)
		.unwrap_or_else(|error| panic!("write root: {error}"));
		assert!(inherits_workspace_dependencies(&client));

		let missing = temp.path().join("missing");
		std::fs::write(client.join("Cargo.toml"), "[package]\nname = \"demo\"\n")
			.unwrap_or_else(|error| panic!("write client: {error}"));
		assert!(make_manifest_standalone(&missing).is_err());
		make_manifest_standalone(&client).unwrap_or_else(|error| panic!("rewrite: {error}"));
		let rewritten = std::fs::read_to_string(client.join("Cargo.toml"))
			.unwrap_or_else(|error| panic!("read client: {error}"));
		assert!(rewritten.contains("[workspace]"));
	}

	/// Every pinned range must admit the version this repository locks and
	/// tests the generated clients with.
	#[test]
	fn requirements_admit_the_repository_lockfile() {
		let lockfile = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.lock");
		let Ok(contents) = std::fs::read_to_string(lockfile) else {
			return;
		};
		let lock = contents
			.parse::<Table>()
			.unwrap_or_else(|error| panic!("parse lockfile: {error}"));
		let packages = lock["package"].as_array().expect("locked packages");
		let parse = |version: &str| {
			version
				.split('.')
				.map(|part| part.parse::<u64>().unwrap_or(0))
				.collect::<Vec<_>>()
		};
		for (name, requirement, _) in CLIENT_DEPENDENCY_REQUIREMENTS {
			let minimum = parse(
				requirement
					.strip_prefix('^')
					.unwrap_or_else(|| panic!("{name} must use a caret requirement")),
			);
			let admitted = packages.iter().any(|package| {
				package["name"].as_str() == Some(*name)
					&& package["version"].as_str().is_some_and(|version| {
						let locked = parse(version);
						locked.first() == minimum.first() && locked >= minimum
					})
			});
			assert!(admitted, "{name} {requirement} must admit a locked version");
		}
	}
}
