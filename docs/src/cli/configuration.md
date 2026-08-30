# Project Configuration

`pina.toml` is the project marker used by `pina build` and `pina generate`. Pina searches from the command's `--project` directory, or the current directory, through its ancestors and uses the nearest configuration file. The legacy uppercase `Pina.toml` spelling is still discovered but deprecated and prints a warning, so new projects should use `pina.toml`.

The complete schema is intentionally small:

```toml
[project]
program = "."
# idl_dir = "target/idl"

[clients]
output = "clients"
languages = ["rust", "typescript"]
```

| Field               | Required | Default                    | Meaning                                         |
| ------------------- | -------- | -------------------------- | ----------------------------------------------- |
| `project.program`   | no       | `.`                        | Directory containing the program `Cargo.toml`.  |
| `project.idl_dir`   | no       | Cargo target directory/idl | Override for generated IDL files.               |
| `clients.output`    | no       | `clients`                  | Root directory for generated client ecosystems. |
| `clients.languages` | no       | `rust`, `typescript`       | Any of `rust`, `typescript`, and `dart`.        |

An empty `pina.toml` uses every default. Configuration paths are resolved relative to the directory containing `pina.toml`; absolute paths, `..` traversal, and symbolic-link escapes are rejected. Unknown sections and fields are rejected so misspellings cannot silently change a build.

Command-line path overrides follow normal shell behavior instead: `pina generate --output <DIR>` resolves a relative directory from the caller's current working directory. Standard Cargo variables remain supported. In particular, a relative `CARGO_TARGET_DIR` is resolved by Cargo metadata and passed to the compiler as an explicit absolute target directory.

Pina can also discover an existing, unambiguous Cargo package without `pina.toml`. Add the file when a workspace contains multiple programs or when a team wants reproducible client selections.
