# Automation and Agent Usage

The CLI help and stream contracts are designed to let an agent discover capabilities without reading repository source.

## Discovery protocol

Use this sequence before constructing a command:

```bash
pina --version
pina --help
pina <command> --help
```

For client generation, inspect the command and its ecosystems:

```bash
pina generate --help
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
pina rehearse --help
```

For project diagnostics, failed transactions, and identity:

```bash
pina doctor --help
pina explain --help
pina keys --help
```

For write-lock contention between instructions, and its interactive map:

```bash
pina locks --help
pina map --help
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

Trace executed compute units from the project's Mollusk tests as JSON. Build and test output goes to stderr, so stdout stays a single JSON document; a failing test run exits with the test runner's status:

```bash
pina profile trace --json > /tmp/trace.json
jq -e '.schemaVersion == 1 and (.instructions | length > 0)' /tmp/trace.json
jq '.instructions[] | {name, executedInstructions, syscalls}' /tmp/trace.json
```

Diff the current build against a saved baseline; the exit status is 2 when a total CU regression reaches both `--fail-cu` and `--fail-percent`:

```bash
pina profile compare /tmp/profile.json --json > /tmp/comparison.json
jq -e '.status == "unchanged" or .status == "improved"' /tmp/comparison.json
```

Rehearse an upgrade against recent traffic. Gate on the exit status first: it is 0 only when at least one transaction was compared and none changed its outcome or written account state, 2 when behaviour changed, 3 when nothing could be compared, and 1 for an operational failure with empty stdout. A JSON check must require the same three conditions, so a report whose only changes are account state, or that compared nothing, never passes:

```bash
pina rehearse --network devnet --json > /tmp/rehearsal.json
jq -e '.schemaVersion == 1 and .summary.total > .summary.skipped and .summary.stateChanged == 0 and .summary.outcomeChanged == 0' /tmp/rehearsal.json
```

Report write-lock contention through the versioned JSON contract, and gate CI on hotspots that `[locks] allow` does not accept:

```bash
pina locks --json > /tmp/pina-locks.json
jq -e '.hotspots | map(select(.allowed | not)) | length == 0' /tmp/pina-locks.json
pina locks --deny-hotspots
```

Read the program map's data, the locks document plus per-instruction and per-account-type detail, without writing its HTML page:

```bash
pina map --json > /tmp/pina-map.json
jq -e '.instructionDetails | length > 0' /tmp/pina-map.json
```

Diagnose project readiness through the versioned JSON contract:

```bash
pina doctor --json > /tmp/pina-doctor.json
```

Read-only identity inspection is also JSON-safe:

```bash
pina keys show --json > /tmp/pina-keys.json
```

Explain a failed transaction through the versioned JSON contract. A saved `getTransaction` result works offline:

```bash
pina explain --transaction-file ./failed.json --json > /tmp/pina-explain.json
jq -e '.status == "succeeded" or (.candidates | type == "array")' /tmp/pina-explain.json
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
- Treat exit code `2` from `rehearse` as a completed rehearsal that found behaviour changes; read `transactions[].status` from the JSON report. Exit code `3` means no transaction could be compared, so nothing was verified; it is never a pass. Exit code `1` is an operational failure with empty stdout. Never pass `--allow-changes` until every `state_changed` and `outcome_changed` transaction has been reviewed.
- `pina rehearse` sends each RPC request once and stops on the first failure. Do not loop it against a rate-limited public endpoint; use a dedicated endpoint or a smaller `--limit`.
- Run `pina build --verify` first and pass its printed content-addressed JSON path to `pina verify record --build-record`.
- Never infer a repository, revision, cluster, authority, or uploader. The build record binds source provenance; every network target and signing identity remains explicit.
- Use `--yes` only for a reviewed record plan. Mainnet submissions additionally require `--acknowledge-mainnet`; transaction export requires neither flag.
- Never put secrets in a custom RPC URL. Pina passes the RPC origin to `solana-verify` as argv.
- Treat `pina explain` candidates by their `confidence`: only `confirmed` is proven by the transaction, `checked_against_current_state` reads state that may have changed after the transaction, and `possible` needs runtime values. Exit code `0` means an explanation was produced, including for a transaction that succeeded.
- `pina explain` queries localnet unless `--network` or `--rpc-url` names another endpoint, and never retries a request.
- Use `pina test --unit` when only native Rust or Mollusk tests are required.
- Treat a missing `tests/surfpool` package or built `.so` as a failed integration setup, not a skip.
- `pina dev` is offline unless `--network` or a credential-free HTTP(S) `--rpc-url` is explicitly supplied. The URL is visible in Surfpool's process arguments, so never place a secret anywhere in it.
- Always inspect `deploy --dry-run --json` before remote automation.
- Never pass `deploy --yes` until the exact target, program ID, authority, payer, and command plan have been reviewed.
- Never put a secret anywhere in a custom deploy RPC URL. Pina rejects user information, queries, and fragments, but accepted hosts and paths remain visible in plan output and process listings because Solana receives the endpoint through `--url`. Prefer named clusters.

## Stable verification

CLI help is snapshot-tested at every command level. IDL stdout is also regression-tested as valid JSON. The book is built by `verify:docs` and published by the repository's GitHub Pages workflow, so command changes should update help tests and this reference in the same change.
