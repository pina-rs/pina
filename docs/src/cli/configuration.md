# Project Configuration

`pina.toml` is the project marker used by `pina build` and `pina generate`. Pina searches from the command's `--project` directory, or the current directory, through its ancestors and uses the nearest configuration file. The legacy uppercase `Pina.toml` spelling is still discovered but deprecated and prints a warning, so new projects should use `pina.toml`.

The complete schema is intentionally small:

```toml
[project]
program = "."
# idl_dir = "target/idl"

[clients]
output = "clients"
languages = ["cpi", "rust", "typescript"]
mode = "auto"
scaffold = true

# Optional per-lint level overrides.
[lints]
require_canonical_instruction_dispatch_for_idl = "deny"

# Optional target-specific overrides.
[clients.cpi]
output = "onchain/cpi"
scaffold = false

[clients.dart]
mode = "update"
```

| Field                       | Required | Default                    | Meaning                                                    |
| --------------------------- | -------- | -------------------------- | ---------------------------------------------------------- |
| `project.program`           | no       | `.`                        | Directory containing the program `Cargo.toml`.             |
| `project.idl_dir`           | no       | Cargo target directory/idl | Override for generated IDL files.                          |
| `clients.output`            | no       | `clients`                  | Root directory for generated client ecosystems.            |
| `clients.languages`         | no       | `rust`, `typescript`       | Any of `cpi`, `rust`, `typescript`, and `dart`.            |
| `clients.mode`              | no       | `auto`                     | Default destination policy for every selected client.      |
| `clients.scaffold`          | no       | `true`                     | Whether missing manifests and entrypoints are initialized. |
| `clients.<target>.output`   | no       | target name                | Target directory beneath `clients.output`.                 |
| `clients.<target>.mode`     | no       | `clients.mode`             | Destination policy for one target.                         |
| `clients.<target>.scaffold` | no       | `clients.scaffold`         | Scaffold policy for one target.                            |
| `lints.<lint-name>`         | no       | built-in level             | Per-lint override: `allow`, `warn`, or `deny`.             |

`<target>` is `cpi`, `rust`, `typescript`, or `dart`. Dart is the Dart and Flutter target; there is no separate Flutter generator.

Lint levels are validated against the bundled lint catalog; see [Run Security Lints](./lint.md) for the full lint-level workflow.

Generation modes make the destination lifecycle explicit:

| Mode        | Empty or missing destination | Existing nonempty destination               |
| ----------- | ---------------------------- | ------------------------------------------- |
| `auto`      | Initial generation           | Update generated source                     |
| `create`    | Initial generation           | Fail                                        |
| `update`    | Fail                         | Update generated source                     |
| `overwrite` | Initial generation           | Delete the complete target, then regenerate |

An update owns only generated source (`src/generated` for Rust/CPI and TypeScript, and `lib/src/generated` plus generated Dart library barrels for Dart). It does not rewrite an existing `Cargo.toml`, `package.json`, `pubspec.yaml`, or Rust crate entrypoint. This makes those files safe to customize after their initial generation. Set `scaffold = false` to generate only source files and never create those package-level files.

`overwrite` intentionally removes the complete target, including custom files, before rendering. Selecting it in `pina.toml` is treated as explicit authorization for that cleanup. Pina still refuses filesystem roots, Git working trees, symbolic-link targets, and output trees containing symbolic links.

An empty `pina.toml` uses every default. Configuration paths are resolved relative to the directory containing `pina.toml`; absolute paths, `..` traversal, and symbolic-link escapes are rejected. Per-target output paths are relative to `clients.output`. Unknown sections and fields are rejected so misspellings cannot silently change a build.

Command-line path overrides follow normal shell behavior instead: `pina generate --output <DIR>` resolves a relative directory from the caller's current working directory. Standard Cargo variables remain supported. In particular, a relative `CARGO_TARGET_DIR` is resolved by Cargo metadata and passed to the compiler as an explicit absolute target directory.

Pina can also discover an existing, unambiguous Cargo package without `pina.toml`. Add the file when a workspace contains multiple programs or when a team wants reproducible client selections.
