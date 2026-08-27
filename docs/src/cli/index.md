# Pina CLI

The `pina` command scaffolds programs, extracts Codama IDLs, renders clients, reads bundled reference material, profiles compiled SBF binaries, and plans explicit deployments. It is designed for both interactive use and scripted or agent-driven discovery.

## Install

Install the prebuilt npm package. It selects the native binary for the current operating system, CPU, and Linux C library:

```bash
npm install --global @pina-rs/cli
pina --version
```

Or install from crates.io with Rust:

```bash
cargo install pina_cli
pina --version
```

Inside this repository, enter the development shell and use the `pina` shortcut:

```bash
devenv shell
pina --help
```

The shortcut runs `cargo run -p pina_cli -- ...` against the checked-out source.

## Command map

| Command                                        | Purpose                                                      | Primary output                            |
| ---------------------------------------------- | ------------------------------------------------------------ | ----------------------------------------- |
| [`pina init`](./init.md)                       | Create a project-aware program scaffold                      | Files plus next steps                     |
| [`pina build`](./build.md)                     | Build SBF, optionally with deterministic verification inputs | SBF, IDL, and optional build-record files |
| [`pina verify`](./verify.md)                   | Compare deployments and record verified source               | Status or transaction                     |
| [`pina generate`](./generate.md)               | Generate configured client ecosystems                        | Generated clients                         |
| [`pina test`](./test.md)                       | Run native/Mollusk or SBF/Surfpool tests                     | Test runner output                        |
| [`pina dev`](./dev.md)                         | Start an offline Surfpool watch/redeploy loop                | Surfpool UI and logs                      |
| [`pina idl`](./idl.md)                         | Extract a Codama root-node IDL                               | JSON                                      |
| [`pina docs`](./docs.md)                       | List or render bundled terminal docs                         | Terminal text                             |
| [`pina profile`](./profile.md)                 | Estimate per-function SBF compute cost                       | Text or JSON                              |
| [`pina deploy`](./deploy.md)                   | Plan and execute an explicit cluster deployment              | Plan or JSON                              |
| [`pina codama generate`](./codama-generate.md) | Generate IDLs and Rust, JavaScript, and Dart clients         | Generated directories                     |

## Discover the interface

The help tree is intentionally self-describing:

```bash
pina --help
pina build --help
pina verify --help
pina verify check --help
pina verify record --help
pina verify submit --help
pina verify status --help
pina generate --help
pina idl --help
pina docs --help
pina init --help
pina profile --help
pina deploy --help
pina codama --help
pina codama generate --help
```

Long help includes the input contract, output behavior, defaults, and copyable examples. Run `pina docs` with no topic to discover the bundled architecture references.

## Streams and exit codes

| Command           | stdout                            | stderr                              |
| ----------------- | --------------------------------- | ----------------------------------- |
| `idl`             | JSON when `--output` is omitted   | Progress, extraction counts, errors |
| `docs`            | Topic index or rendered Markdown  | Errors                              |
| `init`            | Created path and next steps       | Errors                              |
| `build`           | Published artifact summary        | Cargo output and errors             |
| `verify check`    | Matching hash                     | Mismatch hashes and errors          |
| `verify record`   | Upstream streamed progress        | Upstream diagnostics and errors     |
| `generate`        | IDL and client summary            | Renderer output and errors          |
| `test`            | Child test-runner output          | Build output and errors             |
| `dev`             | Surfpool UI and logs              | Build output and errors             |
| `profile`         | Report when `--output` is omitted | Errors                              |
| `deploy`          | Inspectable plan and completion   | Confirmation, progress, errors      |
| `codama generate` | Completion summary                | Errors and renderer failures        |

Successful commands exit with code `0`. Operational failures exit with code `1`. Verification hash mismatches exit with code `2`. Invalid command-line syntax is rejected by Clap with a non-zero usage error before an operation begins.

For reliable automation, capture stdout only when the command documents it as machine-readable. See [Automation and Agent Usage](./automation.md) for a compact discovery protocol.

## Path behavior

Relative paths are resolved from the process working directory. Output commands create their documented output directories where applicable, but `pina idl --output` expects the parent directory to exist. Existing files at explicit output paths are overwritten after the command validates its inputs.

## Environment

Project-aware commands read `Pina.toml` and respect standard Cargo variables such as `CARGO_TARGET_DIR` and `CARGO`. The CLI also reads one Pina-specific optional environment variable:

| Variable             | Used by     | Meaning                                          |
| -------------------- | ----------- | ------------------------------------------------ |
| `PINA_TEMPLATES_DIR` | `pina docs` | Directory containing custom `<topic>.t.md` files |

No configuration file is required for an unambiguous Cargo package. `pina init` creates a small `Pina.toml` so every tool and agent discovers the same program and client choices.

Agents that maintain Pina projects can install the companion [`@pina-rs/skill`](../agent-skill.md) package.
