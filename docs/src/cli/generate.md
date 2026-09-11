# `pina generate`

Refresh the current program's IDL and generate selected client ecosystems.

## Synopsis

```text
pina generate [OPTIONS]
```

| Input                 | Default           | Meaning                                                                               |
| --------------------- | ----------------- | ------------------------------------------------------------------------------------- |
| `-p, --project <DIR>` | current directory | Start directory for project discovery.                                                |
| `--client <LANGUAGE>` | `pina.toml`       | `cpi`, `rust`, `typescript`, `dart`, `cli-rust`, `cli-ts`, or `cli-dart`; repeatable. |
| `-o, --output <DIR>`  | configured output | Override the client output root.                                                      |
| `--mode <MODE>`       | `pina.toml`       | Override with `auto`, `create`, `update`, or `overwrite`.                             |
| `--no-scaffold`       | off               | Generate sources without creating manifests or entrypoints.                           |
| `--npx <COMMAND>`     | `npx`             | Codama runner for TypeScript or Dart.                                                 |

```bash
pina generate
pina generate --client rust
pina generate --client cpi
pina generate --client typescript --client dart
pina generate --client cli-rust
pina generate --client cli-ts
pina generate --client cli-dart
pina generate --mode create
pina generate --mode update --no-scaffold
pina generate --mode overwrite
```

Repeating a language is harmless. Explicit `--client` values replace the configured list for that invocation. CPI-only, Rust-only, and `cli-rust` generation do not invoke Node.js. Each CLI variant implies its base client (`cli-rust` ⇒ `rust`, `cli-ts` ⇒ `typescript`, `cli-dart` ⇒ `dart`); projects normally pick one CLI, and Pina warns when several are selected together.

`auto` initializes an empty destination and otherwise updates it. Updates replace only renderer-owned generated source, preserving customized manifests and crate/package entrypoints. `create` and `update` enforce the expected destination state. `overwrite` deletes the entire selected client target before regeneration; use it for an intentional clean sweep. Command-line `--mode` and `--no-scaffold` override every selected target for that invocation.

Outputs are grouped by ecosystem:

```text
clients/
├── cpi/<library-name>/
├── rust/<library-name>/
├── typescript/<library-name>/
├── dart/
├── cli-rust/<library-name>/
├── cli-ts/<library-name>/
└── cli-dart/
```

Pina rejects filesystem-root and symbolic-link generation targets before a renderer runs. `pina codama generate` remains available for the repository-wide, explicitly pathed compatibility workflow.

See [Project Configuration](./configuration.md) for client defaults and the distinction between configuration-relative and command-line paths.
