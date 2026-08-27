# CLI and Code Generation

## Discover commands from help

The installed CLI is authoritative:

```sh
pina --help
pina build --help
pina generate --help
pina idl --help
pina docs --help
pina init --help
pina profile --help
pina codama generate --help
```

Use `pina docs` to list bundled terminal topics. Custom topics can be supplied through `PINA_TEMPLATES_DIR` when a project maintains its own operational guidance.

## Daily project workflow

Run project-aware commands from the program directory or any descendant. Pina uses the nearest ancestor `Pina.toml`; an existing unambiguous Cargo package also works without configuration.

```sh
pina build
pina generate
```

`pina build` compiles SBF with the required `bpf-entrypoint` feature and refreshes the IDL. Pass program features explicitly when required:

```sh
pina build --features logs,cpi --no-default-features
```

The library target name determines the canonical outputs:

```text
<cargo-target>/deploy/<library-name>.so
<cargo-target>/idl/<library-name>.json
```

`pina generate` refreshes that IDL and renders the client languages selected in `Pina.toml`. Override the selection for one run with repeatable `--client rust`, `--client typescript`, or `--client dart` flags. Generated ecosystem roots may be replaced, so keep hand-written code outside them.

## Deterministic build artifacts

Use the verified-build backend when you need a deterministic SBF artifact:

```sh
pina build --verify
```

This requires an exact `solana-verify 0.5.1` installation, a working Docker-compatible daemon, a root `Cargo.lock`, the generated Solana CLI workspace metadata, and a completely clean Git worktree. Pina does not install or update these prerequisites.

The successful command prints the canonical `target/deploy` artifact, the generated IDL, a content-addressed SBF artifact, and its Pina-local build-record path. Keep the printed record and adjacent SBF together; consumers recompute the executable hash before trusting the record.

`pina build --verify` creates deterministic build inputs and outputs. It does not compare the artifact with an on-chain program or record an on-chain verification result.

## IDL extraction

Generate a Codama root-node document from a program crate:

```sh
pina idl --path ./programs/counter_program --output ./idls/counter_program.json
```

Without `--output`, JSON is the only stdout content; progress and extraction counts go to stderr. This makes the command safe in pipelines:

```sh
pina idl --path ./programs/counter_program --compact | jq -e '.program'
```

Treat the IDL as a public contract. Review instruction, account, PDA, error, and type changes rather than accepting generated churn wholesale.

## Repository-wide client generation

Use the legacy Codama surface when a repository intentionally generates clients for several example programs in one command:

```sh
pina codama generate --examples-dir ./programs --idls-dir ./idls \
  --rust-out ./clients/rust --js-out ./clients/js --dart-out ./clients/dart
```

Use repeatable `--example` filters for a focused run. Generated roots may be replaced; never store hand-written code inside them.

Pina's generated clients preserve discriminator-first layouts and zeropod boundary checks. If a repository uses a custom renderer command, keep that command as the source of truth.

## Static SBF profiling

Profile a compiled shared object:

```sh
pina profile ./target/deploy/counter_program.so
pina profile ./target/deploy/counter_program.so --json --output ./profile.json
```

The report is a static estimate, not a validator execution trace. Use it for deterministic comparisons and investigate material changes in context.

# Verified deployments

Use the content-addressed record produced by the deterministic build. Never invent or override its repository, revision, paths, library, or Cargo feature set.

```bash
pina build --verify
pina verify check --program-id <ADDRESS> --cluster devnet
pina verify record \
  --program-id <ADDRESS> \
  --cluster devnet \
  --build-record ./target/pina/verifiable/my_program-<HASH>.json \
  --authority ./upgrade-authority.json
```

`pina verify check` is read-only: it compares the local artifact with the deployed executable and returns exit code `2` for a completed hash mismatch. Do not retry that result as an infrastructure failure.

`pina verify record` rebuilds the exact repository revision from the validated build record and writes verification metadata on-chain. Review the program, cluster, record, and authority before adding `--yes`; mainnet and unknown remote RPC origins additionally require `--acknowledge-mainnet`.

Use `pina verify record --export [AUTHORITY] --output verification.tx` when another signer or multisig must submit the transaction. Export performs Pina's deployed-hash preflight but never submits or rebuilds the repository, does not require `--yes` or `--acknowledge-mainnet`, and writes only the validated base58 or base64 transaction payload. Remote verification begins only after the exported transaction is submitted.

`pina verify submit --program-id <ADDRESS> --uploader <ADDRESS>` submits an existing record to the official mainnet remote verifier. `pina verify status --program-id <ADDRESS>` is the corresponding read-only mainnet status query. Never place credentials in an RPC URL; the URL is necessarily visible in child-process arguments.
