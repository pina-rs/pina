# CLI and Code Generation

## Discover commands from help

The installed CLI is authoritative:

```sh
pina --help
pina build --help
pina generate --help
pina idl --help
pina idl generate --help
pina idl fetch --help
pina idl diff --help
pina idl publish --help
pina docs --help
pina init --help
pina lint --help
pina test --help
pina dev --help
pina keys --help
pina doctor --help
pina completions --help
pina profile --help
pina deploy --help
pina codama generate --help
```

Use `pina docs` to list bundled terminal topics. Custom topics can be supplied through `PINA_TEMPLATES_DIR` when a project maintains its own operational guidance.

## Daily project workflow

Run project-aware commands from the program directory or any descendant. Pina uses the nearest ancestor `pina.toml`; an existing unambiguous Cargo package also works without configuration.

```sh
pina lint
pina build
pina test --unit
pina test
pina generate
```

`pina build` compiles SBF with the required `bpf-entrypoint` feature and refreshes the IDL. Pass program features explicitly when required:

```sh
pina build --features logs,cpi --no-default-features
```

Run `pina lint` before review to execute the official Pina security lint set associated with the installed CLI release. Use `pina lint --fix` only when source edits are authorized, then inspect and test every change. The command intentionally ignores additional project Dylint metadata; use direct `cargo dylint` only after reviewing and pinning any extra native lint libraries.

The library target name determines the canonical outputs:

```text
<cargo-target>/deploy/<library-name>.so
<cargo-target>/idl/<library-name>.json
```

`pina generate` refreshes that IDL and renders the client languages selected in `pina.toml`. Override the selection for one run with repeatable `--client rust`, `--client typescript`, or `--client dart` flags. Generated ecosystem roots may be replaced, so keep hand-written code outside them.

## Project diagnostics and identity

Use the versioned diagnostic report before changing a project:

```sh
pina doctor --json
pina keys show --json
```

`doctor --json` keeps stdout valid JSON and returns a failing exit status when required project or SBF prerequisites are unavailable. Its tool requirements follow the clients selected in `pina.toml`.

Treat program identity changes as security-sensitive. `pina keys sync` validates an existing Ed25519 keypair and updates exactly one parsed `declare_id!`. `pina keys new` creates a local identity; only `pina keys new --force` may rotate an existing one. Never copy or print keypair bytes. On platforms where Pina cannot guarantee private permissions, generate the keypair with trusted platform tooling and then run `pina keys sync --keypair <path>`.

## Deterministic build artifacts

Use the verified-build backend when you need a deterministic SBF artifact:

```sh
pina build --verify
```

This requires an exact `solana-verify 0.5.1` installation, a working Docker-compatible daemon, a root `Cargo.lock`, the generated Solana CLI workspace metadata, and a completely clean Git worktree. Pina does not install or update these prerequisites.

The successful command prints the canonical `target/deploy` artifact, the generated IDL, a content-addressed SBF artifact, and its Pina-local build-record path. Keep the printed record and adjacent SBF together; consumers recompute the executable hash before trusting the record.

`pina build --verify` creates deterministic build inputs and outputs. It does not compare the artifact with an on-chain program or record an on-chain verification result.

## Testing and development

Use `pina test --unit` for native/Mollusk tests and `pina test` for the generated SBF/Surfpool integration package under `tests/surfpool`. Cargo remains attached to the terminal in both modes. `pina dev` delegates persistent artifact watching and redeployment to Surfpool, which owns terminal input, output, errors, prompts, and Ctrl-C until it exits. Its default is offline, so select `--network` or `--rpc-url` only when remote state is required. Prefer a named network. Explicit RPC URLs must be credential-free HTTP(S) URLs with a host and no user information, query, fragment, or control character. They are visible in Surfpool's child-process arguments, so never put a secret anywhere in the host, path, or other URL text. On the first run, use `pina dev --yes`, then inspect and commit the `txtx.yml` runbook Surfpool creates.

## IDL extraction

Generate a Codama root-node document from a program crate. Bare invocation remains compatible; `generate` makes the operation explicit:

```sh
pina idl --path ./programs/counter_program --output ./idls/counter_program.json
pina idl generate --path ./programs/counter_program --output ./idls/counter_program.json
```

Without `--output`, JSON is the only stdout content; progress and extraction counts go to stderr. This makes the command safe in pipelines:

```sh
pina idl --path ./programs/counter_program --compact | jq -e '.program'
```

Treat the IDL as a public contract. Review instruction, account, PDA, error, and type changes rather than accepting generated churn wholesale.

## Canonical on-chain IDLs

Network IDL operations always require an explicit cluster. Fetch the canonical direct, zlib-compressed UTF-8 Codama IDL under the fixed `idl` seed:

```sh
pina idl fetch --cluster devnet --program-id <PROGRAM_ADDRESS> --output ./idl.json
pina idl diff --cluster devnet --program-id <PROGRAM_ADDRESS> --file ./idl.json
```

`diff` compares parsed JSON: object order and whitespace are ignored, but array order is preserved. Exit status `0` means equal, `2` means different, and `1` means the command failed.

Direct publication requires the canonical upgrade-authority keypair and explicit confirmation (`--yes` in automation):

```sh
pina idl publish --cluster devnet --file ./idl.json \
  --authority ~/.config/solana/upgrade-authority.json --yes
```

For review, multisig, or DAO signing, export every transaction the official planner requires without submitting any:

```sh
pina idl publish --cluster mainnet-beta --file ./idl.json \
  --export <MULTISIG_ADDRESS> --output ./idl-plan.txt
```

Do not describe an export as one transaction. Preserve the complete upstream `[Transaction #N]` framing and order. An exported authority is a noop signer; do not combine `--export <ADDRESS>` with local authority or payer keypairs.

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
pina profile
pina profile ./target/deploy/counter_program.so
pina profile ./target/deploy/counter_program.so --json --output ./profile.json
```

When the path is omitted, Pina discovers `<cargo-target>/deploy/<library-name>.so`. The report is a static estimate, not a validator execution trace. Use it for deterministic comparisons and investigate material changes in context. Output files are written atomically and cannot alias the input binary through hardlinks or linked paths.

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

## Safe deployment

Plan deployments before permitting a write:

```sh
pina deploy --cluster devnet \
  --upgrade-authority ./keys/devnet-authority.json \
  --payer ./keys/devnet-payer.json \
  --dry-run --json
```

The cluster is always explicit. Pina never inherits a Solana CLI target or wallet, and deploy never creates a program identity. Conventional artifacts are `<cargo-target>/deploy/<library-name>.so` and `<cargo-target>/deploy/<library-name>-keypair.json`; override either path only when the plan requires it. Review the program ID and complete argument vector in the plan. Keep keypairs below 4 KiB and owner-private; on Unix use mode `0600`, while Windows ACLs must be restricted with operating-system tooling.

Remote execution requires an interactive `deploy` confirmation or `--yes`. Named mainnet and custom remote endpoints also require `--allow-mainnet`. Custom URL user information, queries, and fragments are rejected, but accepted hosts and paths remain visible in plan output and process listings. Never put a secret anywhere in the URL; prefer a named cluster. Use `--build` when the canonical artifact must be refreshed before the final plan. Deployment requires the external Agave `solana` executable on `PATH`.
