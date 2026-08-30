# `pina build`

Build the current Pina program for SBF and refresh its Codama IDL.

## Synopsis

```text
pina build [OPTIONS]
```

| Input                       | Default           | Meaning                                                 |
| --------------------------- | ----------------- | ------------------------------------------------------- |
| `-p, --project <DIR>`       | current directory | Start directory for project discovery.                  |
| `--features <FEATURE>`      | none              | Extra Cargo features; repeat or separate by commas.     |
| `--no-default-features`     | off               | Disable the program's default Cargo features.           |
| `--verify`                  | off               | Build deterministically through Solana Verify.          |
| `--solana-verify <COMMAND>` | `solana-verify`   | Select the exact 0.5.1 executable; requires `--verify`. |

`bpf-entrypoint` is always enabled and deduplicated from explicit features.

SBF compilation delegates to the Agave CLI's `cargo-build-sbf` driver, which owns the SBF toolchain (its own rustc and sbf-linker from platform-tools). The Agave CLI must therefore be installed and on `PATH`; a nightly toolchain with `rust-src` is no longer required.

## Outputs

Pina uses Cargo metadata, including `CARGO_TARGET_DIR`, to derive stable output paths:

```text
<cargo-target>/deploy/<library-name>.so
<cargo-target>/idl/<library-name>.json
```

The library target name is authoritative. It may differ from the package name when `[lib].name` is set.

Cargo output is streamed to the terminal. Pina stages both outputs first and atomically replaces each destination file. A project-scoped advisory lock prevents concurrent builds from interleaving different IDL and SBF versions. If the second replacement fails, Pina attempts to restore the previous IDL. A failed Cargo build does not publish either output; the two separate files do not form a crash-atomic filesystem transaction.

```bash
pina build
pina build --features logs,cpi --no-default-features
pina build --project ./programs/counter
```

## Deterministic verified-build artifacts

`pina build --verify` switches the SBF compiler backend to [`solana-verify`](https://github.com/solana-foundation/solana-verifiable-build). It does not query a cluster, compare an on-chain program, upload verification metadata, or install any tools.

Prerequisites:

- `solana-verify` must report exactly version `0.5.1`.
- Docker must be installed and running. Solana Verify owns Docker discovery and diagnostics.
- The project must be in a completely clean Git worktree. Staged, unstaged, and untracked files all cause the command to fail.
- The Cargo workspace root reported by Cargo metadata must contain a tracked `Cargo.lock`.
- Pinocchio and modular Solana SDK workspaces should declare the Solana CLI version at the workspace root so Solana Verify can select a pinned build image:

  ```toml
  [workspace.metadata.cli]
  solana = "3.0.0"
  ```

Pina copies only files tracked in the Git index into a private snapshot. Ignored files—including `.env`, keypairs, `node_modules`, `.devenv`, and ordinary build output—are not mounted into the container. Tracked symbolic links are rejected instead of followed. Git submodules are not supported in this first version; vendor or replace those inputs with tracked workspace dependencies.

```bash
cargo install solana-verify --version 0.5.1 --locked
docker version
pina build --verify
```

The deterministic build publishes the same canonical deployment paths as an ordinary build:

```text
<cargo-target>/deploy/<library-name>.so
<cargo-target>/idl/<library-name>.json
```

It also retains a content-addressed copy and a Pina-local build record:

```text
<cargo-target>/pina/verifiable/<library-name>-<executable-hash>.so
<cargo-target>/pina/verifiable/<library-name>-<executable-hash>.json
```

The executable hash follows Solana Verify semantics: trailing zero padding is removed before SHA-256. The JSON records only facts Pina can compute: the exact Solana Verify version, library and relative workspace paths, Cargo features, default-feature selection, lockfile hash, Git revision, repository URL when it is credential-free HTTPS, and source-state diagnostics. It is not an official on-chain verification record. Consumers must recompute the executable hash before trusting it, and publication workflows must independently confirm that the recorded revision is reachable from the public remote.

Pina atomically replaces each destination file. The content-addressed artifact and build record are written before the canonical artifact and IDL. These separate files do not form a single crash-atomic filesystem transaction; a failed final publication can leave a valid, hash-bound content-addressed record without updating the canonical deploy path.

Feature selection is identical for both backends:

```bash
pina build --verify --features logs,cpi --no-default-features
```

Pina forwards only its validated feature selection to Solana Verify. It intentionally offers no raw Cargo, Docker, image, or shell-argument passthrough.

Solana Verify 0.5.1 is supported on Linux and macOS when a compatible executable and Docker runtime are available. Native Windows and FreeBSD execution fail before source staging; use a supported Linux environment instead.

Project discovery uses the nearest ancestor `pina.toml`. Existing projects without that file fall back to Cargo metadata; an ambiguous workspace root fails with the candidate package names instead of guessing.

See [Project Configuration](./configuration.md) for the complete `pina.toml` schema and path rules.
