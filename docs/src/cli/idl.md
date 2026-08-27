# Generate and publish IDLs

`pina idl` owns the complete IDL lifecycle without adding another top-level command: generate a Codama document, compare it with canonical Program Metadata, fetch it, or publish an update.

## Synopsis

```text
pina idl [OPTIONS]
pina idl generate [OPTIONS]
pina idl fetch --cluster <CLUSTER> [OPTIONS]
pina idl diff --cluster <CLUSTER> [OPTIONS]
pina idl publish --cluster <CLUSTER> [OPTIONS]
```

Bare `pina idl [OPTIONS]` remains exactly equivalent to `pina idl generate [OPTIONS]`. Existing scripts do not need to change.

## Generate a local IDL

| Option                | Default            | Meaning                                                 |
| --------------------- | ------------------ | ------------------------------------------------------- |
| `-p, --path <DIR>`    | `.`                | Program crate containing `Cargo.toml` and `src/lib.rs`. |
| `-o, --output <FILE>` | stdout             | Write JSON to a file.                                   |
| `-n, --name <NAME>`   | Cargo package name | Override the emitted program name.                      |
| `--compact`           | off                | Emit one-line JSON instead of pretty-printed JSON.      |

## Output contract

Without `--output`, stdout contains JSON and nothing else. Progress and extraction counts go to stderr, so redirection is safe:

```bash
pina idl --path ./programs/counter_program > ./counter_program.json
jq . ./counter_program.json
```

With `--output`, the JSON is written to that file and stdout is unused:

```bash
mkdir -p ./idls
pina idl \
  --path ./programs/counter_program \
  --output ./idls/counter_program.json
```

The output file is replaced if it exists. Its parent directory must already exist.

## What the extractor reads

The extractor starts at `src/lib.rs` and follows Rust `mod` declarations, including `#[path = "..."]` modules. Missing unconditional module files are errors. Missing modules behind `#[cfg(...)]` are treated as inactive because their configuration may not apply to the IDL build. It derives the public IDL from source shapes rather than requiring a separate schema file.

It recognizes:

- `#[instruction]` payload structs and discriminator values;
- `#[account]` state layouts;
- `#[error]` enums;
- `#[pda]` typed seed declarations;
- `#[derive(Accounts)]` account order and mutability;
- direct validation chains for signer, writable, address, owner, and PDA metadata;
- canonical and grouped instruction dispatch arms.

Read [Codama Workflow](../codama-workflow.md#extractor-coverage) for supported dispatch shapes and source-authoring rules.

## Program naming

By default, the Codama program name comes from `package.name` in the target `Cargo.toml`. `--name` changes the emitted name without changing the Rust package or source tree:

```bash
pina idl -p ./programs/counter_program --name counter_v2
```

## Failure modes

The command exits unsuccessfully when the path is not a readable program crate, `[package].name` is absent, modules cannot be resolved, more than one source file defines `process_instruction`, declarations conflict, PDA attributes or validation links cannot be resolved, a supported schema cannot be represented safely, or JSON/output-file writing fails. Diagnostics include source or path context where available. These checks are fail-closed: the CLI does not silently omit malformed PDA metadata or choose one of several entrypoint dispatch sources.

For complete client generation, continue with [`pina codama generate`](./codama-generate.md).

## Canonical Program Metadata

Pina publishes an IDL to Solana's [Program Metadata program](https://github.com/solana-program/program-metadata) with these fixed semantics:

- seed: `idl`;
- account type: canonical metadata;
- authority: the deployed program's upgrade authority;
- source: direct, inline account data;
- encoding: UTF-8;
- compression: zlib;
- format: JSON.

The canonical PDA is derived from the program address and the fixed `idl` seed. This gives each program one authoritative IDL. Pina does not expose third-party metadata, custom seeds, URL data, external-account data, immutability, or account closure in this initial workflow.

The adapter delegates transaction planning to the exact official npm package `@solana-program/program-metadata@0.9.0`. The behavior was cross-checked against upstream commit `33eb527e124cc4a09d8aae448cd306a9bd87db14`. Pina does not duplicate the upstream allocation, reallocation, buffer, rent, or transaction-packing rules.

### Runtime requirement

Network IDL commands require Node.js, npm, and an npx-compatible runner. By default Pina executes:

```text
npx --yes @solana-program/program-metadata@0.9.0 ...
```

The package version is pinned, never `latest`. `npx` may download that pinned package if it is not already cached. Use `--npx <COMMAND>` only for a runner that accepts the same npx argument contract; it is not an arbitrary shell command. Pina constructs an argument vector and never invokes a shell.

Client stdout and stderr are drained concurrently into bounded buffers. Oversized output fails explicitly; Pina never truncates a transaction export and presents it as a valid plan.

## Clusters and RPC safety

Every network operation requires `--cluster`. Accepted aliases are `mainnet-beta`, `mainnet`, `devnet`, `testnet`, `localnet`, and `localhost`. An HTTPS RPC origin may also be supplied. Loopback HTTP is accepted for local testing; remote plaintext HTTP is rejected.

Custom RPC URLs may not contain user information, query parameters, fragments, or a path. This is intentional: RPC API credentials placed in a URL would otherwise be exposed to the child process argument list. Configure an authenticated local proxy and pass its credential-free origin when a provider requires secrets.

## Fetch

```bash
pina idl fetch \
  --cluster mainnet-beta \
  --program-id <PROGRAM_ADDRESS> \
  --output ./target/fetched/program.json
```

`--program-id` may be omitted inside a discoverable Pina project. Pina then generates the local IDL only to infer `program.publicKey`.

Fetch is deliberately fail-closed. Pina asks the official client for the raw stored bytes, then locally applies a bounded zlib decode, UTF-8/JSON parsing, complete Codama root-node deserialization, and target-program comparison. It does not follow URL or external-account metadata. Accounts using another otherwise-valid Program Metadata representation produce an actionable unsupported-content error instead of triggering an unexpected outbound request.

The decompressed IDL is capped at 16 MiB. `--output` is written atomically. Without it, stdout is the IDL JSON. `--json` wraps the document in a versioned result envelope for agents.

## Semantic diff

```bash
pina idl diff --cluster devnet

pina idl diff \
  --cluster mainnet-beta \
  --program-id <PROGRAM_ADDRESS> \
  --file ./target/idl/program.json \
  --json
```

The comparison parses both documents as JSON. Object key order and whitespace do not matter; array order does. Exit statuses are:

| Status | Meaning                                                    |
| ------ | ---------------------------------------------------------- |
| `0`    | Local and canonical on-chain IDLs are semantically equal.  |
| `1`    | Discovery, validation, RPC, decoding, or subprocess error. |
| `2`    | Both IDLs are valid, but their semantic values differ.     |

This makes `pina idl diff --cluster <CLUSTER> --json` suitable for deployment and release checks.

## Publish directly

```bash
pina idl publish \
  --cluster devnet \
  --authority ~/.config/solana/upgrade-authority.json
```

Pina generates the project IDL unless `--file` is supplied. If `--program-id` is also supplied, it must equal `program.publicKey` inside that complete Codama document. Pina cryptographically checks that a keypair file's public half matches its Ed25519 secret half, then gives the official client a stable private temporary copy so replacing the original path cannot change the signer after validation. Files larger than 4 KiB are rejected; on Unix, group or other permissions are also rejected and temporary copies use a `0700` directory with a `0600` file. Windows copies inherit the current user's protected temporary-directory ACL; Windows keypair validation does not attempt to interpret arbitrary source-file ACLs. Key material is never printed.

`--payer <KEYPAIR>` separates transaction fees and metadata-account rent from the upgrade authority. When omitted, the authority also pays. `--priority-fee <MICROLAMPORTS>` defaults to `100000`, matching the official client.

Interactive publication prints the cluster, program, and local IDL source, then requires the exact word `publish`. In CI, pass `--yes`. Export mode never submits and therefore does not require this confirmation.

The Program Metadata account must remain rent-exempt. Creation funds its 96-byte header and packed IDL content. Updates may transfer additional rent, extend the account in bounded steps, write through temporary buffers when a transaction cannot hold the content, trim unused bytes, and close temporary buffers. Those decisions belong to the pinned official planner.

## Export for review or a multisig

Export every planned transaction without submitting anything:

```bash
pina idl publish \
  --cluster mainnet-beta \
  --authority ./upgrade-authority.json \
  --export \
  --output ./idl-plan.txt
```

For a Squads vault or another multisig, provide its public address instead of a secret keypair:

```bash
pina idl publish \
  --cluster mainnet-beta \
  --program-id <PROGRAM_ADDRESS> \
  --file ./target/idl/program.json \
  --export <MULTISIG_AUTHORITY> \
  --export-encoding base58 \
  --output ./idl-update.txt
```

An exported authority is a noop signer used only to plan the transaction messages. Do not pass `--authority` or `--payer` with `--export <ADDRESS>`; the multisig must authorize and pay when it imports the plan. The export contains every transaction in the upstream order, including allocation, buffer, write, initialization/update, trim, and cleanup transactions where required. Pina preserves the official `[Transaction #N]` framing rather than pretending the workflow is one transaction.

The default export encoding is base64. Use base58 when the receiving multisig requires it. Without `--output`, the complete export is written to stdout and Pina writes no status text there.

Exported transactions contain a recent blockhash. If approval takes too long, regenerate the export; do not submit a partially approved subset from an old plan.

## Failure recovery

- **The authority is rejected:** verify the deployed program is upgradeable and the supplied signer is its current upgrade authority. Pina never falls back to non-canonical metadata.
- **The process stops during a multi-transaction write:** rerun `pina idl diff` first. If it differs, rerun the same publish command. The official planner inspects current state and creates an update plan; do not manually guess which write transaction was last confirmed.
- **An exported blockhash expires:** discard the complete export and regenerate it.
- **A buffer transaction succeeds but a later transaction fails:** retain logs and rerun publication. The upstream planner owns buffer lifecycle and recovery behavior.
- **Fetch reports unsupported content:** the canonical account is not direct zlib/UTF-8 JSON. Use the official Program Metadata tooling to inspect it. Pina will not follow its URL or external account.
- **IDL program mismatch:** regenerate from the correct program or pass the correct target. Never publish by editing only `program.publicKey` to bypass the check.
- **npx fails:** confirm Node.js and npm are installed and that the pinned package can be resolved. Corporate/offline environments should pre-cache version `0.9.0` or supply an npx-compatible runner.
