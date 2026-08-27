# `pina verify`

Verify deployed executables and publish source-build records with the official `solana-verify` 0.5.1 workflow.

Start with a deterministic build:

```bash
pina build --verify
```

That command prints a content-addressed JSON record beside its `.so` artifact under `target/pina/verifiable`. Pina re-reads the record, rejects filesystem aliases and unsupported schema/tool versions, and recomputes the artifact's trailing-zero-aware executable hash before it uses any provenance.

## Compare a deployment

```bash
pina verify check \
  --program-id <ADDRESS> \
  --cluster devnet
```

Use `--program <PROGRAM.SO>` to select an executable directly or `--project <DIR>` to discover `target/deploy/<library>.so`. Pina delegates both hashes to `solana-verify`: executable hashing removes trailing zero padding before SHA-256, matching the official deployed-program comparison.

| Exit code | Meaning                                                     |
| --------- | ----------------------------------------------------------- |
| `0`       | The local and deployed executable hashes match.             |
| `2`       | The comparison completed, but the executable hashes differ. |
| `1`       | Validation, tool execution, or RPC access failed.           |

`check` is read-only.

## Record verified source

```bash
pina verify record \
  --program-id <ADDRESS> \
  --cluster devnet \
  --build-record ./target/pina/verifiable/my_program-<HASH>.json \
  --authority ./upgrade-authority.json
```

Before invoking the repository rebuild, Pina verifies that the record's artifact hash matches the deployed program. It then passes the record's public repository, full revision, mount path, workspace path, library name, Cargo features, and default-feature choice to `verify-from-repo`. Arbitrary repository or revision overrides are intentionally unavailable.

Interactive recording prints a plan and requires typing `record`. Automation must pass `--yes`. Submissions to mainnet and unknown remote RPC origins also require `--acknowledge-mainnet`; `--yes` does not imply that acknowledgement. Exports are non-mutating and require neither flag. The authority keypair also pays transaction fees because `solana-verify` 0.5.1 does not support a separate payer.

Pina recognizes the upstream 0.5.1 hash-mismatch transcript as exit code `2`. This guard matters because that upstream release exits successfully when its repository rebuild differs and does not write a verification record.

Repository recording uses upstream clone and Docker behavior and is supported only on Linux and macOS. Read-only comparison and remote-status commands remain available when the exact executable works on another platform.

## Export for a multisig

```bash
pina verify record \
  --program-id <ADDRESS> \
  --cluster mainnet-beta \
  --build-record ./target/pina/verifiable/my_program-<HASH>.json \
  --export <MULTISIG_ADDRESS> \
  --output ./verification.tx \
  --export-encoding base64
```

Export never submits. It runs the separate upstream `export-pda-tx` operation after Pina's build-record/deployment hash preflight. That upstream operation encodes the source and build arguments but does not rebuild the repository; remote verification happens only after the transaction is submitted. Upstream progress remains diagnostic output; only the final validated base58 or base64 transaction payload is atomically written to `--output`. Supplying a public address after `--export` does not require an authority secret.

## Remote verifier

After an on-chain record exists:

```bash
pina verify submit --program-id <ADDRESS> --uploader <ADDRESS>
pina verify status --program-id <ADDRESS>
```

These commands target the official mainnet remote verifier. `uploader` is the public address that created the record, not a keypair. Status is read-only.

## Tool and RPC security

Pina requires the exact version string `solana-verify 0.5.1` and never installs it. Select an executable explicitly with `pina verify --solana-verify <COMMAND> ...`.

Cluster aliases are `mainnet-beta`, `devnet`, `testnet`, and `localnet`. A custom endpoint must be a credential-free HTTPS origin with no path, query, or fragment; loopback HTTP is allowed for local development. RPC endpoints are necessarily passed to `solana-verify` in process arguments, where the operating system's process listing may expose them. Put no credentials or access tokens in an RPC URL.

Authority files must be private regular files, not symbolic links. Pina bounds the file size, parses the 64-byte Solana keypair, and cryptographically verifies that its public half matches its secret half before starting the network workflow. Before confirmation, Pina freezes the reviewed provenance in memory and copies the validated keypair into a private temporary file so replacing the original paths cannot change the approved submission. Keypair bytes are never printed or passed in arguments; only that private temporary path is passed to upstream `--keypair`.
