# Project Configuration

`pina.toml` is the project marker used by `pina build` and `pina generate`. Pina searches from the command's `--project` directory, or the current directory, through its ancestors and uses the nearest configuration file. The legacy uppercase `Pina.toml` spelling is still discovered but deprecated and prints a warning, so new projects should use `pina.toml`.

The complete schema is intentionally small. Every section is optional; an empty file uses every default, and unknown sections and fields are rejected so misspellings cannot silently change a build.

```toml
[project]
program = "." # directory containing the program Cargo.toml
# idl_dir = "target/idl"             # override where generated IDLs are written

# Optional named path anchors. Values may reference `{{root}}`.
[project.paths]
sdks = "{{root}}/sdks"

[clients]
output = "clients" # root directory for generated clients
languages = ["cpi", "rust", "typescript"]
mode = "auto" # destination lifecycle policy
scaffold = true # initialize missing manifests and entrypoints

[lints] # optional per-lint level overrides
require_canonical_instruction_dispatch_for_idl = "deny"

# Optional ABI migrations. `version_type` is `u8` (default and recommended),
# `u16`, or `u32`; it freezes at the first published release.
[migrations]
version_type = "u8"
auto = true

# Optional persisted disambiguation answers for `pina migrations create`.
[migrations.answers]
rename = ["value:points"]
assume_removed = []

# Optional target-specific overrides. Any of output, mode, and scaffold may be set.
[clients.cpi]
output = "onchain/cpi"
scaffold = false

[clients.dart]
mode = "update"
```

## Path fields and anchors

Configuration paths are resolved relative to the directory containing `pina.toml`. Paths may also climb out of the project directory with `..`, or anchor at the repository root with template anchors:

- `{{root}}` expands to the git repository root — the working-tree top level, discovered with `git rev-parse --show-toplevel`. Linked worktrees resolve to the worktree itself, so each worktree gets its own output locations. Without a git binary, Pina falls back to the nearest ancestor containing a `.git` entry (a directory for repositories, a file for worktrees). Using `{{root}}` outside a repository is an error.
- A declared anchor such as `{{ sdks }}` expands to the value of the `[project.paths]` entry of the same name. Entry values may only reference `{{root}}` — they cannot reference each other — which keeps resolution single-step and cycle-free. Anchor names must match `[A-Za-z_][A-Za-z0-9_-]*`; `root` is reserved. Whitespace inside the braces is allowed (`{{ sdks }}` and `{{sdks}}` are the same anchor).

The discovery directory for `{{root}}` is the `pina.toml` folder, so a config nested in `programs/my_program/` can still write clients to `{{root}}/clients`. The git root is only consulted when an anchor is actually used.

Validation rules for every configured path:

| Rule                 | Behavior                                                                                                                                                                                                                                                                               |
| -------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Non-empty            | An empty or whitespace-only path is rejected.                                                                                                                                                                                                                                          |
| Relative or anchored | Literal absolute paths (`/tmp/clients`, `C:\tmp`) are rejected; write `{{root}}/...` instead so the anchor is explicit.                                                                                                                                                                |
| `..` traversal       | Allowed, including paths that leave the project directory. Traversing above the filesystem root is rejected.                                                                                                                                                                           |
| Symbolic links       | Existing components beneath the trusted base (the project directory or the repository root for anchored paths) may not be symbolic links. Generation-time checks still refuse destinations that are themselves links, link-like targets, filesystem roots, or a git working-tree root. |

Command-line path overrides follow normal shell behavior instead: `pina generate --output <DIR>` resolves a relative directory from the caller's current working directory, and `..` is accepted. Standard Cargo variables remain supported. In particular, a relative `CARGO_TARGET_DIR` is resolved by Cargo metadata and passed to the compiler as an explicit absolute target directory.

## `[project]` fields

| Field                  | Required | Default                    | Meaning                                                                                                                                                   |
| ---------------------- | -------- | -------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `project.program`      | no       | `.`                        | Directory containing the program `Cargo.toml`. May contain `..` or anchors. The directory must contain a Cargo package with a library (or cdylib) target. |
| `project.idl_dir`      | no       | Cargo target directory/idl | Override for generated IDL files. `pina build` writes `<idl_dir>/<library-name>.json`.                                                                    |
| `project.paths.<name>` | no       | —                          | Declares a named path anchor usable as `{{ name }}` in any path field. Values may reference `{{root}}`.                                                   |

## `[clients]` fields

| Field               | Required | Default              | Meaning                                                                                                                       |
| ------------------- | -------- | -------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| `clients.output`    | no       | `clients`            | Root directory for generated client ecosystems. Resolved relative to the `pina.toml` directory; anchors and `..` are allowed. |
| `clients.languages` | no       | `rust`, `typescript` | Which ecosystems to generate: any of `cpi`, `rust`, `typescript`, `dart`, `cli-rust`, `cli-ts`, and `cli-dart`.               |
| `clients.mode`      | no       | `auto`               | Default destination policy for every selected client; see the mode table below.                                               |
| `clients.scaffold`  | no       | `true`               | Whether missing package-level files (manifests, entrypoints) are initialized around the generated sources.                    |

`<target>` in the per-client table below is one of the language names. Dart is the Dart and Flutter target; there is no separate Flutter generator.

| Field                       | Required | Default            | Meaning                                                                                                          |
| --------------------------- | -------- | ------------------ | ---------------------------------------------------------------------------------------------------------------- |
| `clients.<target>.output`   | no       | target name        | Destination beneath `clients.output`. Anchored values resolve to absolute destinations outside the clients root. |
| `clients.<target>.mode`     | no       | `clients.mode`     | Destination policy for one target.                                                                               |
| `clients.<target>.scaffold` | no       | `clients.scaffold` | Scaffold policy for one target.                                                                                  |

Selecting a CLI target implies its base client (`cli-rust` ⇒ `rust`, `cli-ts` ⇒ `typescript`, `cli-dart` ⇒ `dart`), and projects normally pick one CLI; selecting several prints a warning. CLI apps render into `clients/cli-rust`, `clients/cli-ts`, and `clients/cli-dart` respectively — the Dart CLIs share one package at `cli-dart` with a `bin/<library-name>.dart` executable per program. Override a CLI target under the matching table (`[clients.cli_rust]`, `[clients.cli_ts]`, `[clients.cli_dart]`), using the kebab spelling as a deprecated alias (`[clients.cli-rust]`).

Override the selection for one run with repeatable `--client cpi`, `--client rust`, `--client typescript`, `--client dart`, `--client cli-rust`, `--client cli-ts`, or `--client cli-dart` flags.

## `[lints]` fields

| Field               | Required | Default        | Meaning                                                                                                                                                                    |
| ------------------- | -------- | -------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `lints.<lint-name>` | no       | built-in level | Per-lint override: `allow`, `warn`, or `deny`. Names are validated against the bundled lint catalog; see [Run Security Lints](./lint.md) for the full lint-level workflow. |

## `[migrations]` fields

These settings opt a program into version-envelope management. See [the migration flow](../migrations/flow.md) for the on-chain behavior; this section covers only the configuration.

| Field                               | Required | Default | Meaning                                                                                                                                                                                                                |
| ----------------------------------- | -------- | ------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `migrations.version_type`           | no       | `u8`    | Width of the version envelope: `u8`, `u16`, or `u32`. A width must be chosen before the first publication; the runtime implements only these three. `version-type` is accepted as a deprecated alias.                  |
| `migrations.auto`                   | no       | `false` | Program-wide opt-in for `pina migrations create`. `true` enrolls every contract kind, `false` disables the policy, and a list enrolls only the named kinds: `accounts`, `events`, or `instructions`.                   |
| `migrations.answers.rename`         | no       | `[]`    | Persisted rename answers in `from:to` form (for example `"value:points"`). `pina migrations create` consults them before prompting; command-line flags override them per field, and a contradicting flag fails closed. |
| `migrations.answers.assume_removed` | no       | `[]`    | Persisted data-dropping acknowledgements, replayed the same way. `assume-removed` is accepted as a deprecated alias.                                                                                                   |

`auto = true` enrolls every kind, so it cannot be combined with a kind list. `pina migrations create` records the resolved policy into `migrations/manifest.json`, which becomes the checked-in source of truth that macros consult; hand-editing the manifest, `migrations/publications.json`, or generated transition files is never allowed. Individual contracts can still opt out with an explicit `migrations = false` attribute.

## Generation modes

Generation defaults to `mode = "auto"`: it initializes an empty target, then updates only renderer-owned source directories on later runs. Use `mode = "create"` or `mode = "update"` to enforce the expected state, and `mode = "overwrite"` for an explicit complete cleanup. The CLI equivalents are `--mode` and `--no-scaffold`.

| Mode        | Empty or missing destination | Existing nonempty destination               |
| ----------- | ---------------------------- | ------------------------------------------- |
| `auto`      | Initial generation           | Update generated source                     |
| `create`    | Initial generation           | Fail                                        |
| `update`    | Fail                         | Update generated source                     |
| `overwrite` | Initial generation           | Delete the complete target, then regenerate |

`overwrite` intentionally removes the complete target, including custom files, before rendering. Selecting it in `pina.toml` is treated as explicit authorization for that cleanup. Pina still refuses filesystem roots, git working-tree roots, symbolic-link targets, and output trees containing symbolic links.

## What generation owns versus what it scaffolds

Every client target separates two kinds of files, and knowing the split tells you what is safe to customize:

- **Renderer-owned files** are rewritten on every generation and must never be hand-edited. Update mode replaces only these.
- **Scaffolded files** are created once, only when missing, and are yours afterwards. Later runs never rewrite them, so manifests can be renamed, repackaged, or extended freely after the first generation.

| Target       | Renderer-owned (every run)                                          | Scaffolded once (only when missing)                                                       |
| ------------ | ------------------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| `rust`       | `src/generated/**` — a `mod.rs`-rooted module tree                  | `Cargo.toml`, `src/lib.rs` (the two-line shim `pub mod generated; pub use generated::*;`) |
| `cpi`        | `src/generated/**`                                                  | `Cargo.toml`, `src/lib.rs` — the crate name honors the program's Cargo package name       |
| `typescript` | `src/generated/**`                                                  | `package.json`                                                                            |
| `dart`       | `lib/src/generated/<program>/**` plus the generated library barrels | `pubspec.yaml`                                                                            |
| `cli-rust`   | Application sources (`src/**`) and a `.pina-generated` marker       | `Cargo.toml`, `README.md`                                                                 |
| `cli-ts`     | The complete application, including `package.json`                  | — (everything is renderer-owned)                                                          |
| `cli-dart`   | Application sources (`lib/src/<program>/**`, `bin/**`, guard files) | `pubspec.yaml`                                                                            |

Set `scaffold = false` to generate only renderer-owned files and never create package-level files — the "files, not the package" mode for embedding generated sources into a crate or package you own. Note that the Rust `src/generated` tree references `crate::<PROGRAM>_PROGRAM_ID` (the upper-snake library name), which the default `lib.rs` shim satisfies by glob re-exporting `generated::*`; an embedded tree needs an equivalent re-export at its host crate root.

An update owns only renderer-owned paths. The Rust and CPI renderers refuse to replace a `src/generated` tree whose `mod.rs` lacks Pina's autogenerated header, and `cli-rust` update refuses a crate without its `.pina-generated` marker, so trees written by hand are never overwritten.

Names are read back from scaffolded manifests on later runs: the Rust CLI derives its dependency path from the crate name in the existing `Cargo.toml`, and the Dart CLI derives its package import from the `name:` in the existing `pubspec.yaml`. Renaming a client in its manifest after first generation is therefore supported and respected.

## Example configurations

Defaults only — generates Rust and TypeScript clients into `./clients` next to the program:

```toml
# pina.toml
```

A program publishing an SDK in every language, with clients at the top of the repository rather than inside the program directory:

```toml
[project]
program = "programs/lootbox"

[clients]
output = "{{root}}/clients"
languages = ["cpi", "rust", "typescript", "dart"]

[clients.cpi]
output = "{{root}}/crates/lootbox-cpi"
```

The same layout expressed once through named anchors, useful when several fields share a prefix:

```toml
[project]
program = "programs/lootbox"

[project.paths]
clients = "{{root}}/clients"

[clients]
output = "{{ clients }}"
languages = ["rust", "typescript"]

[clients.rust]
output = "{{ clients }}/rust"
```

Source-only generation beside an existing SDK crate — no manifest is created or touched, so the handwritten SDK keeps full ownership of its `Cargo.toml` and entrypoint. The `<program>` directory level is always present, so embed the tree from the SDK's `lib.rs` with a `#[path = "lootbox_program/src/generated/mod.rs"] pub mod generated;` declaration and re-export it:

```toml
[project]
program = "programs/lootbox"

[clients]
output = "{{root}}/sdks/rust"
languages = ["rust"]
scaffold = false

[clients.rust]
output = "."
```

This renders only `sdks/rust/lootbox_program/src/generated/**`; the SDK crate's manifest must also declare the generated dependencies (Pina scaffolds them into a fresh `Cargo.toml` when `scaffold = true`, so generate once with scaffolding to copy the pin list).

A CLI-first project that also keeps the CPI crate for other programs to compose with:

```toml
[project]
program = "."

[clients]
output = "clients"
languages = ["cpi", "rust", "cli-rust"]
mode = "auto"

[clients.cli_rust]
scaffold = false
```

A migration-aware program with lint strictness raised for the canonical-dispatch rule:

```toml
[project]
program = "."

[migrations]
version_type = "u16"
auto = ["accounts", "events"]

[migrations.answers]
rename = ["value:points"]
assume_removed = []

[lints]
require_canonical_instruction_dispatch_for_idl = "deny"
```

## Command-line overrides

`pina generate` accepts `--output <DIR>` (replaces `clients.output`, resolved from the current working directory), repeatable `--client <LANGUAGE>`, `--mode <MODE>`, and `--no-scaffold`. Per-run flags override the file for that invocation only; the committed configuration remains the source of record for teammates and CI.

Pina can also discover an existing, unambiguous Cargo package without `pina.toml`: defaults apply and clients land in `<package>/clients`. Add the file when a workspace contains multiple programs or when a team wants reproducible client selections.
