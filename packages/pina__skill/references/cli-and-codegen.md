# CLI and Code Generation

## Discover commands from help

The installed CLI is authoritative:

```sh
pina --help
pina build --help
pina generate --help
pina cpi --help
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
pina migrations --help
pina generate --help
```

Use `pina docs` to list bundled terminal topics. Custom topics can be supplied through `PINA_TEMPLATES_DIR` when a project maintains its own operational guidance.

## Daily project workflow

Run project-aware commands from the program directory or any descendant. Pina uses the nearest ancestor `pina.toml`; an existing unambiguous Cargo package also works without configuration. `pina idl` is the exception: it reads the crate at `--path` (default `.`) without discovery, so run it from the crate root or pass `--path`.

```sh
pina lint
pina build
pina test --unit
pina test
pina generate
```

`pina build` compiles SBF with the required `bpf-entrypoint` feature and refreshes the IDL. Pass program features explicitly when required:

```sh
pina build --features logs --no-default-features
```

Run `pina lint` before review to execute the official Pina security lint set associated with the installed CLI release. Use `pina lint --fix` only when source edits are authorized, then inspect and test every change. The bundled driver statically links the whole lint catalog, so additional project-defined lint libraries are never loaded.

The library target name determines the canonical outputs:

```text
<cargo-target>/deploy/<library-name>.so
<cargo-target>/idl/<library-name>.json
```

`pina generate` refreshes that IDL and renders the client languages selected in `pina.toml` under `[clients] languages`. Override the selection for one run with repeatable `--client cpi`, `--client rust`, `--client typescript`, `--client dart`, `--client cli-rust`, `--client cli-ts`, or `--client cli-dart` flags. Dart is also the Flutter target. CPI output is a separate `no_std` crate with `.invoke()` and `.invoke_signed()` builders. Each CLI application is generated on top of its base client (`cli-rust` requires `rust`, `cli-ts` requires `typescript`, `cli-dart` requires `dart`).

Generation defaults to `mode = "auto"`: it initializes an empty target, then updates only renderer-owned source directories on later runs. Existing manifests and crate/package entrypoints are preserved. Use `mode = "create"` or `mode = "update"` to enforce the expected state, `mode = "overwrite"` for an explicit complete cleanup, and `scaffold = false` for source-only output. These settings may be global under `[clients]` or overridden under `[clients.cpi]`, `[clients.rust]`, `[clients.typescript]`, and `[clients.dart]`. The CLI equivalents are `--mode` and `--no-scaffold`.

Generate the same crate from an external Codama or Anchor IDL with `pina cpi --idl <FILE> --output <DIR>`. Codama IDLs stay native; raw Anchor IDLs are normalized through `@codama/nodes-from-anchor` first.

TypeScript, Dart, `cli-ts`, and `cli-dart` generation runs the version-pinned Codama renderers through `npx` (falling back to `pnpm dlx`), so a project needs no local `node_modules`, but the first run needs registry access. Rust, CPI, and `cli-rust` crates inherit their dependencies from an enclosing workspace's `[workspace.dependencies]` when one declares `pina`. Otherwise, a freshly scaffolded client manifest names concrete, tested version requirements and declares its own empty `[workspace]`, so it builds on its own. An existing client manifest is yours and is never rewritten.

The IDL follows the migration policy recorded in `migrations/manifest.json`, the same one the macros expand against, so the program and its clients agree on every envelope. Without a manifest there is no policy, so nothing carries a version byte; record the baseline with `pina migrations create --auto true` before generating clients you intend to keep, because adding the policy later changes their wire format.

## Compute unit budgets

Priority fees are charged per requested compute unit, so clients should request a limit close to what the transaction uses. Record what each instruction costs, then regenerate:

```sh
pina test --record-compute-units
pina generate
```

`pina test --record-compute-units` runs the complete Surfpool suite (it conflicts with `--unit` and `--filter`) and writes `compute-units.json` beside the program's `Cargo.toml`: the most compute units each instruction consumed in a successful simulation, keyed by IDL name, plus the SHA-256 of the measured build. Failed transactions never count. An instruction no successful test sent is left out and named in a warning; send it through `ProgramTest::send`, `send_instruction`, `send_with_signers`, or `send_transaction` in a passing test to measure it. Commit the file with the clients it produced.

IDL generation attaches each measurement as a `pinaComputeUnits` plugin with `{ "measured": n, "limit": m }`, where `limit = round_up_to_100(measured × (100 + margin_percent) / 100) + 300`. `[compute_units] margin_percent` in `pina.toml` defaults to `20`; the 300 units cover `SetComputeUnitLimit` and `SetComputeUnitPrice` (150 each). Generators copy the limit and never recompute it:

- Rust: `<NAME>_MEASURED_COMPUTE_UNITS`, `<NAME>_COMPUTE_UNIT_LIMIT`, and `set_compute_unit_limit_instruction(units)`.
- TypeScript and Dart: the same constants and `get<Program>ComputeUnitLimit(instructions)`, which sums the program's instructions and returns no limit when one is unmeasured. Pass the result to `setTransactionMessageComputeUnitLimit`. The sum is conservative and ignores other programs' instructions.
- CLIs: every command requests its instruction's limit; `--compute-unit-limit <UNITS>` overrides it and `--simulate` prints consumption against the limit requested.

Re-record after changing the program. `pina generate` warns when `target/deploy/<library-name>.so` differs from the recorded build, and fails when `compute-units.json` names an instruction the program no longer declares; re-record or delete the stale entry. `pina build`, `pina idl`, and `pina test` warn about stale entries and ignore them, and the recording run never reads the file it replaces.

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

`pina generate` generates the clients for one project, discovered through its `pina.toml`. To generate a whole repository, run it once per project:

```sh
for project in ./programs/*/; do
  pina generate --project "$project"
done
```

Output locations come from each project's `[clients]` table, so a repository controls per-language roots in configuration rather than on the command line. Generated roots may be replaced; never store hand-written code inside them.

Pina's generated clients preserve discriminator-first layouts and PinaPod boundary checks. Compact client codecs enforce declared capacity at both encode and decode boundaries. If a repository uses a custom renderer command, keep that command as the source of truth.

## Migration-aware generated clients

Generated code reads the checked-in `migrations/manifest.json`, so run `pina migrations create` before regenerating clients and never hand-edit a generated file. For every opted-in contract the Rust, TypeScript, and Dart clients emit:

- `<Account>MIGRATION_VERSION` (Dart `stateMigrationVersion`) — the schema version this client was generated from. Encoders stamp it into the envelope automatically; callers never pass a version. Decoders enforce it and reject other versions with a stale/future distinction: `getPinaPodMigrationVersionDecoder` in TypeScript, `StateVersionError::{Stale, Future}` in Rust, and the generated Dart equivalent. A stale split tells the caller to migrate the account on chain and retry; a future split tells the caller to upgrade the client.
- `<account>NeedsMigration(bytes)` (Rust `state_needs_migration`) — a cheap envelope check that returns true only when the bytes name this account's discriminator and carry a version older than the client's schema. Foreign discriminators and future versions return false; the decoder explains those when the account is decoded.
- `Migrate` — the reserved framework instruction composer (TypeScript and Dart `getMigrateInstruction`, Rust `Migrate::new().instruction()`). Slots 0 and 1 are always sent: the payer (the program-address placeholder when omitted) and the system program, which defaults to `11111111111111111111111111111111` because the program rejects anything else there. Every migratable slot is optional: omitted slots become program-address placeholders and trailing omitted migratable slots are dropped, so a client sends only the accounts that need migrating. Intended flow: check `needsMigration`, send `Migrate`, then retry the original instruction.
- Event log decoders — `parse<Program>EventsFromLogs(logs, programAddress?)` plus per-event `parse<Event>FromLog` and `decode<Event>` (for example `decodeValueChangedEventEvent` and `decodeValueChangedEventV0Event`, the names the Dart clients use too). Pass the complete, ordered log messages of one transaction to the program-level parser: it follows the runtime's `Program <address> invoke [n]` and `success`/`failed` frames and decodes a data line only while this program (or the `programAddress` you pass for another deployment) is the innermost invoked program. Any program can write a `Program data:` line with your discriminator, including one your program invokes through CPI, so the per-event `parse<Event>FromLog` helpers, which cannot attribute a line, are only for bytes already known to come from your program. A `Log truncated` transaction loses frames, so treat its events as incomplete. A versioned event is decoded, never converted: each earlier version is its own generated event, `<Event>V<n>` (for example `valueChangedEventV0`), with that version's codec, and the program-level parser routes every record by discriminator and version. A version no generated event describes throws (`event "valueChangedEvent" log carries migration version N, which this client cannot decode; regenerate it`) instead of being misread. Decoded records carry only their version's fields — no `sourceVersion`, no `wasMigrated`, no zero-filled fields. The Rust event module has no program-level parser: each version's `try_from_bytes` separates a stale record from a future one and names the event generated for the other version.

The one-call `migrateIfNeeded` RPC helper is designed in ADR 0008 but not generated; compose the check, `Migrate`, and retry explicitly.

## Using generated clients safely

Generated decoders parse bytes; they do not establish where those bytes came from. When an agent writes code on top of a client, it has to add the checks the client cannot make:

- **Check the owner before trusting decoded account data.** TypeScript, Dart, and Rust account decoders and CPI-crate `parse`/`matches` helpers check the discriminator (and the version envelope) only. An attacker-owned account can carry the same leading bytes. Off chain, compare the fetched account's owner with the program address before using the decoded state, or fetch a PDA through `fetch<Account>FromSeeds`, which derives the address. On chain, `assert_owner(&callee::ID)` the `AccountView` before parsing another program's state with its CPI crate.
- **Do not treat client decoding as validation.** On-chain `try_from_bytes` requires an exact length; TypeScript decoders accept trailing bytes, and no client enforces the program's `#[pina(validate(...))]` rules on event or account values. Validate values that matter where they are consumed.
- **Trust the IDL's writable flags only as far as the program declares them.** The IDL marks an account writable when its field is `&mut`, when the program calls `assert_writable` or declares `#[pina(validate(writable))]`, or when it is the payer, `from`, or `to` of a Pina account-creation, allocation, or transfer builder. Any other path that moves an account's lamports must still declare writability explicitly. Otherwise every client sends the account read-only, and the transaction fails with a privilege escalation whenever that account is not also the fee payer, and always through CPI.
- **Read CPI size constants carefully.** A CPI crate drops IDL fields its renderer cannot express, so compare `LEN`/`MAX_LEN` with the program's own `SIZE` before sizing an allocation from them.

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

The plan prints the paths you passed, but the deploy program receives private owner-only copies (`<tmp>/pina-deploy-*/payer.json` and so on) that Pina re-verifies against the plan before running it. `--remote-command` replaces `solana program deploy` with `sh -c <command>` and passes those copies through `PINA_DEPLOY_*` environment variables, never on the command line.

Remote execution requires an interactive `deploy` confirmation or `--yes`. Named mainnet and custom remote endpoints also require `--allow-mainnet`. A loopback URL (`localhost`, `127.0.0.1`, `[::1]`) skips confirmation and records no publication receipt. Pina cannot tell a local validator from an SSH tunnel or proxy to a live cluster on a loopback port, so never deploy to a forwarded remote cluster through a loopback URL; use its real URL instead. Non-loopback deploys record a publication receipt that freezes the deployed migration versions; `--record-publication` opts a loopback deploy into the same lifecycle. Custom URL user information, queries, and fragments are rejected, but accepted hosts and paths remain visible in plan output and process listings. Never put a secret anywhere in the URL; prefer a named cluster. Use `--build` when the canonical artifact must be refreshed before the final plan. Deployment requires the external Agave `solana` executable on `PATH`.
