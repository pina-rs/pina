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

[locks] # optional intentional write-lock hotspots for `pina locks`
allow = ["program_config"]

# Optional persisted disambiguation answers for `pina migrations create`.
[migrations.answers]
rename = ["value:points"]
assume_removed = []
manual = []

# Optional margin applied to recorded compute unit measurements.
[compute_units]
margin_percent = 20

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

| Field               | Required | Default              | Meaning                                                                                                                                                                                                                                |
| ------------------- | -------- | -------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `clients.output`    | no       | `clients`            | Root directory for generated client ecosystems. Resolved relative to the `pina.toml` directory, and every configured output must resolve inside the project's Git worktree; pass `--output` on the command line to publish outside it. |
| `clients.languages` | no       | `rust`, `typescript` | Which ecosystems to generate: any of `cpi`, `rust`, `typescript`, `dart`, `cli-rust`, `cli-ts`, and `cli-dart`.                                                                                                                        |
| `clients.mode`      | no       | `auto`               | Default destination policy for every selected client; see the mode table below.                                                                                                                                                        |
| `clients.scaffold`  | no       | `true`               | Whether missing package-level files (manifests, entrypoints) are initialized around the generated sources.                                                                                                                             |

`<target>` in the per-client table below is one of the language names. Dart is the Dart and Flutter target; there is no separate Flutter generator.

| Field                       | Required | Default            | Meaning                                                                                                             |
| --------------------------- | -------- | ------------------ | ------------------------------------------------------------------------------------------------------------------- |
| `clients.<target>.output`   | no       | target name        | Destination beneath `clients.output`. Anchored values may reach anywhere inside the Git worktree, never outside it. |
| `clients.<target>.mode`     | no       | `clients.mode`     | Destination policy for one target.                                                                                  |
| `clients.<target>.scaffold` | no       | `clients.scaffold` | Scaffold policy for one target.                                                                                     |

Selecting a CLI target implies its base client (`cli-rust` ⇒ `rust`, `cli-ts` ⇒ `typescript`, `cli-dart` ⇒ `dart`), and projects normally pick one CLI; selecting several prints a warning. CLI apps render into `clients/cli-rust`, `clients/cli-ts`, and `clients/cli-dart` respectively — the Dart CLIs share one package at `cli-dart` with a `bin/<library-name>.dart` executable per program. Override a CLI target under the matching table (`[clients.cli_rust]`, `[clients.cli_ts]`, `[clients.cli_dart]`), using the kebab spelling as a deprecated alias (`[clients.cli-rust]`).

Override the selection for one run with repeatable `--client cpi`, `--client rust`, `--client typescript`, `--client dart`, `--client cli-rust`, `--client cli-ts`, or `--client cli-dart` flags.

## `[lints]` fields

| Field               | Required | Default        | Meaning                                                                                                                                                                    |
| ------------------- | -------- | -------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `lints.<lint-name>` | no       | built-in level | Per-lint override: `allow`, `warn`, or `deny`. Names are validated against the bundled lint catalog; see [Run Security Lints](./lint.md) for the full lint-level workflow. |

## `[locks]` fields

| Field         | Required | Default | Meaning                                                                                                                                                                                                                                                                                                                               |
| ------------- | -------- | ------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `locks.allow` | no       | `[]`    | Hotspots the program keeps on purpose, by account name, such as an admin configuration only admin instructions write. `pina locks` still reports them, marked allowed, and `--deny-hotspots` ignores them. Every entry must name a current hotspot; an entry that names none fails the command. See [Report Write Locks](./locks.md). |

## `[migrations.answers]` fields

These answers are replayed by `pina migrations create` so fresh clones and CI repeat a decision made once. See [the migration flow](../migrations/flow.md) for the on-chain behavior; this section covers only the configuration.

| Field                               | Required | Default | Meaning                                                                                                                                                                                                                |
| ----------------------------------- | -------- | ------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `migrations.answers.rename`         | no       | `[]`    | Persisted rename answers in `from:to` form (for example `"value:points"`). `pina migrations create` consults them before prompting; command-line flags override them per field, and a contradicting flag fails closed. |
| `migrations.answers.assume_removed` | no       | `[]`    | Persisted data-dropping acknowledgements, replayed the same way. `assume-removed` is accepted as a deprecated alias.                                                                                                   |
| `migrations.answers.manual`         | no       | `[]`    | Persisted `--manual` answers: added fields whose conversion is written by hand rather than generated.                                                                                                                  |

The migration policy is not configured here. The version envelope width and the `auto` policy live only in `migrations/manifest.json`, the one source macros read, and `pina migrations create --version-type` and `--auto` record them there; see [Manage ABI migrations](./migrations.md#opt-whole-kinds-in). The retired `[migrations].version_type` (or `version-type`) and `[migrations].auto` keys fail every command that reads `pina.toml`, with an error naming the flag and value that replace them, for example:

```text
`[migrations].auto` no longer belongs in pina.toml: migrations/manifest.json records it, and macros read only the manifest. Remove the key and run `pina migrations create --auto true` to record it.
```

Hand-editing the manifest, `migrations/publications.json`, or generated transition files is never allowed. Individual contracts can still opt out of a recorded policy with an explicit `migrations = false` attribute.

## `[compute_units]` fields

These settings turn the measurements `pina test --record-compute-units` writes to `compute-units.json` into the compute unit limits generated clients request. See [compute unit limits](./generate.md#compute-unit-limits) for the formula and what each client receives.

| Field                          | Required | Default | Meaning                                                                                                                                                                                                                                            |
| ------------------------------ | -------- | ------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `compute_units.margin_percent` | no       | `20`    | Percentage added to each measurement before it is rounded up to a hundred and the compute budget instructions' 300 units are added. Raise it when real inputs cost more than the recorded test fixtures. `margin-percent` is accepted as an alias. |

The margin applies at generation time, so changing it and running `pina generate` updates every limit without recording again.

## Generation modes

Generation defaults to `mode = "auto"`: it initializes an empty target, then updates only renderer-owned source directories on later runs. Use `mode = "create"` or `mode = "update"` to enforce the expected state. The CLI equivalents are `--mode` and `--no-scaffold`.

| Mode        | Empty or missing destination | Existing nonempty destination                         |
| ----------- | ---------------------------- | ----------------------------------------------------- |
| `auto`      | Initial generation           | Update generated source                               |
| `create`    | Initial generation           | Fail                                                  |
| `update`    | Fail                         | Update generated source                               |
| `overwrite` | Initial generation           | Remove the recorded Pina-owned files, then regenerate |

`overwrite` is an operator decision, not a configuration value: `pina.toml` cannot select it, and requesting destructive regeneration takes `--mode overwrite` on the command line. A `pina.toml` ships with the repository, so it is not trusted to authorize removing directories.

Deletion is bounded by a tracked-files record (`.pina-generated.json`) written at each client root: regeneration and `overwrite` remove only the paths a previous Pina run recorded, so files you added to a generated tree survive, and a directory Pina never generated is refused instead of removed. A destination that predates tracked manifests — generated by an older Pina — is refused by `overwrite` with a remedy in the error; remove it by hand once, or generate without `overwrite` once to record its files. Pina still refuses filesystem roots, git working-tree roots, symbolic-link targets, and output trees containing symbolic links.

## Package resolution for renderers

The default `--npx npx` runner resolves the pinned renderer packages through the `npx`/`pnpm dlx` cache, from an isolated working directory with project-local entries removed from `PATH`: a committed `node_modules` in the project can neither shadow the pinned packages nor execute during generation. Passing a Node executable explicitly (`--npx node`, or a path to one) is the project-package mode — the documented way to resolve renderers from the project's own install on purpose, which is also what keeps generation offline.

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

Because a scaffolded `package.json` is never rewritten, its `@solana/kit` range stays wherever the first generation left it even as the generated sources around it move forward with the renderer toolchain. The same applies to a scaffolded Dart `pubspec.yaml` and its Solana Kit Dart ranges. Hand-editing those ranges is the supported way to move a client between Kit versions: later runs respect whatever is written.

Generation refuses to update a client only when the existing manifest provably cannot resolve the Kit version the generated sources compile against — `@solana/kit` for TypeScript, the `solana_kit_*` packages for Dart, in either the inline or the nested `version:` declaration form. It judges a range by what it can resolve rather than by its first number, so a bounded range like `>=7 <9` or a union like `^8.3.0 || ^7.0.0` passes because npm installs the newer version, while `^7.0.0` fails. The error names the manifest and the range to raise, and `overwrite` mode is the explicit way to start the scaffold over on the current ranges.

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

A migration-aware program with persisted answers and lint strictness raised for the canonical-dispatch rule. Its policy, for example `pina migrations create --auto accounts,events --version-type u16`, is recorded in `migrations/manifest.json` rather than here:

```toml
[project]
program = "."

[migrations.answers]
rename = ["value:points"]
assume_removed = []

[lints]
require_canonical_instruction_dispatch_for_idl = "deny"
```

## Command-line overrides

`pina generate` accepts `--output <DIR>` (replaces `clients.output`, resolved from the current working directory), repeatable `--client <LANGUAGE>`, `--mode <MODE>`, and `--no-scaffold`. Per-run flags override the file for that invocation only; the committed configuration remains the source of record for teammates and CI.

Pina can also discover an existing, unambiguous Cargo package without `pina.toml`: defaults apply and clients land in `<package>/clients`. Add the file when a workspace contains multiple programs or when a team wants reproducible client selections.
