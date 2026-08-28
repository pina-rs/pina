# Automation and Agent Usage

The CLI help and stream contracts are designed to let an agent discover capabilities without reading repository source.

## Discovery protocol

Use this sequence before constructing a command:

```bash
pina --version
pina --help
pina <command> --help
```

For Codama, inspect both levels:

```bash
pina codama --help
pina codama generate --help
```

For deployment verification, inspect the group and selected leaf:

```bash
pina verify --help
pina verify check --help
pina verify record --help
```

For project testing and a persistent local network:

```bash
pina lint --help
pina test --help
pina dev --help
```

For project diagnostics and identity:

```bash
pina doctor --help
pina keys --help
```

For framework and extractor constraints:

```bash
pina docs
pina docs pina-overview
pina docs pina-idl
```

Do not infer flags from examples alone. The command-specific `--help` output is the authoritative interface and includes defaults, output routing, requirements, and examples.

## Machine-readable workflows

Generate and validate an IDL through stdout:

```bash
pina idl --path ./programs/counter_program --compact > /tmp/counter.json
jq -e . /tmp/counter.json
```

Write directly to a known artifact path:

```bash
mkdir -p ./artifacts
pina idl \
  --path ./programs/counter_program \
  --output ./artifacts/counter.json
```

Profile a binary as JSON:

```bash
pina profile ./target/deploy/counter_program.so --json > /tmp/profile.json
jq -e '.functions | type == "array"' /tmp/profile.json
```

Diagnose project readiness through the versioned JSON contract:

```bash
pina doctor --json > /tmp/pina-doctor.json
```

Read-only identity inspection is also JSON-safe:

```bash
pina keys show --json > /tmp/pina-keys.json
```

These commands write no progress or ANSI styling to stdout. Doctor check IDs and statuses are stable agent inputs; do not parse the human report when JSON is available.

Inspect a deployment without executing a child process:

```bash
pina deploy \
  --project ./programs/counter_program \
  --cluster devnet \
  --upgrade-authority ./keys/devnet-authority.json \
  --payer ./keys/devnet-payer.json \
  --dry-run --json > /tmp/deploy-plan.json
jq -e '.program_id and .commands' /tmp/deploy-plan.json
```

## Automation rules

- Check the exit status before consuming output.
- Run `pina lint` before review; use `--fix` only when working-tree edits are authorized and always inspect the diff.
- Treat stderr as diagnostics and progress, not as part of IDL JSON.
- Use explicit paths; relative paths depend on the process working directory.
- Create the parent of an `idl --output` file before invoking the command.
- Treat Codama output roots as replaceable generated directories.
- Use repeated `--example` flags instead of assuming comma-separated parsing.
- Inspect `pina docs` before requesting a topic.
- Never use the input `.so` path as the profile output path.
- Treat exit code `2` from `verify check` or `verify record` as a verified hash mismatch, not an operational failure.
- Run `pina build --verify` first and pass its printed content-addressed JSON path to `pina verify record --build-record`.
- Never infer a repository, revision, cluster, authority, or uploader. The build record binds source provenance; every network target and signing identity remains explicit.
- Use `--yes` only for a reviewed record plan. Mainnet submissions additionally require `--acknowledge-mainnet`; transaction export requires neither flag.
- Never put secrets in a custom RPC URL. Pina passes the RPC origin to `solana-verify` as argv.
- Use `pina test --unit` when only native Rust or Mollusk tests are required.
- Treat a missing `tests/surfpool` package or built `.so` as a failed integration setup, not a skip.
- `pina dev` is offline unless `--network` or a credential-free HTTP(S) `--rpc-url` is explicitly supplied. The URL is visible in Surfpool's process arguments, so never place a secret anywhere in it.
- Always inspect `deploy --dry-run --json` before remote automation.
- Never pass `deploy --yes` until the exact target, program ID, authority, payer, and command plan have been reviewed.
- Never put a secret anywhere in a custom deploy RPC URL. Pina rejects user information, queries, and fragments, but accepted hosts and paths remain visible in plan output and process listings because Solana receives the endpoint through `--url`. Prefer named clusters.

## Stable verification

CLI help is snapshot-tested at every command level. IDL stdout is also regression-tested as valid JSON. The book is built by `verify:docs` and published by the repository's GitHub Pages workflow, so command changes should update help tests and this reference in the same change.
