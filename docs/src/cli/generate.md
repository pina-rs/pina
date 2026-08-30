# `pina generate`

Refresh the current program's IDL and generate selected client ecosystems.

## Synopsis

```text
pina generate [OPTIONS]
```

| Input                 | Default           | Meaning                                      |
| --------------------- | ----------------- | -------------------------------------------- |
| `-p, --project <DIR>` | current directory | Start directory for project discovery.       |
| `--client <LANGUAGE>` | `pina.toml`       | `rust`, `typescript`, or `dart`; repeatable. |
| `-o, --output <DIR>`  | configured output | Override the client output root.             |
| `--npx <COMMAND>`     | `npx`             | Codama runner for TypeScript or Dart.        |

```bash
pina generate
pina generate --client rust
pina generate --client typescript --client dart
```

Repeating a language is harmless. Explicit `--client` values replace the configured list for that invocation. Rust-only generation does not invoke Node.js.

Outputs are grouped by ecosystem:

```text
clients/
├── rust/<library-name>/
├── typescript/<library-name>/
└── dart/
```

Pina rejects filesystem-root and symbolic-link generation targets before a renderer runs. `pina codama generate` remains available for the repository-wide, explicitly pathed compatibility workflow.

See [Project Configuration](./configuration.md) for client defaults and the distinction between configuration-relative and command-line paths.
