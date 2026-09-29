# `pina deploy`

`pina deploy` resolves and validates a Pina program deployment, displays the complete operation, and delegates the write to the Solana CLI. It never inherits a cluster from Solana configuration.

```text
pina deploy [OPTIONS] --upgrade-authority <KEYPAIR> --payer <KEYPAIR> \
  --cluster <CLUSTER|URL>
```

Run `pina deploy --help` for the authoritative option list.

## Safe planning

Review a deployment without building, contacting an RPC endpoint, or invoking the Solana CLI:

```bash
pina deploy \
  --project ./programs/counter_program \
  --cluster devnet \
  --upgrade-authority ./keys/devnet-authority.json \
  --payer ./keys/devnet-payer.json \
  --dry-run
```

Use `--dry-run --json` for agents and CI. The plan includes the canonical project, artifact, program keypair, declared program ID, upgrade authority, fee payer, cluster, complete RPC endpoint, acknowledgement policy, and ordered command argument vector. Custom URL hosts and paths are intentionally preserved, so they must not contain secrets.

Dry runs perform no build or deployment. Consequently, `--dry-run` conflicts with `--build`, `--yes`, and `--allow-mainnet`.

## Input resolution

`--project` accepts a project directory or any directory below it. Pina uses the same project discovery as `pina build`: the nearest ancestor `pina.toml`, then unambiguous Cargo metadata as a fallback. The library target name and Cargo metadata target directory resolve:

- `<cargo-target>/deploy/<lib-name>.so`
- `<cargo-target>/deploy/<lib-name>-keypair.json`

Use `--program` or `--program-keypair` to override either conventional path. `--program` conflicts with `--build`. Every resolved file is canonicalized and must be a regular file. Keypair files cannot exceed 4 KiB and must contain a valid 64-byte Solana JSON keypair. On Unix, Pina rejects keypairs with any group or world permission bits; `chmod 600 <keypair>` is the recommended mode. Pina cannot inspect equivalent Windows ACL policy, so Windows operators must restrict keypair access with the operating system's ACL tooling. The program keypair address must match the program's `declare_id!`; deploy never generates or silently replaces an identity.

`--upgrade-authority` and `--payer` are always explicit. Pina does not inherit wallet paths from Solana configuration and does not store deployment credentials in `pina.toml`.

## Targets and confirmation

One explicit target is required:

- `--cluster localnet`
- `--cluster devnet`
- `--cluster testnet`
- `--cluster mainnet-beta`
- `--cluster <HTTP(S)-URL>`

Custom endpoints reject URL user information, query parameters, and fragments. The Solana CLI accepts its RPC endpoint through `--url`, so every accepted host and path is visible in the dry-run plan, confirmation, and operating-system process listings. Pina cannot distinguish a provider token embedded in a path from a legitimate endpoint path. Never put a secret anywhere in a custom URL; prefer a named cluster when possible.

Local endpoints execute after displaying the plan. Every remote endpoint prompts the operator to type `deploy`. Non-interactive remote deployment fails before starting a child process unless `--yes` is supplied. Named mainnet and every custom remote endpoint additionally require `--allow-mainnet`, because Pina cannot prove which Solana cluster an arbitrary URL serves. The flag is rejected for localnet, devnet, testnet, and custom loopback endpoints.

Local means the parsed host is a loopback address (`localhost`, `127.0.0.0/8`, or `[::1]`), so spellings such as `localhost.example.com`, `127.0.0.1.nip.io`, or `http://127.0.0.1@example.com` are classified as remote or rejected. A loopback port is still not proof of a local cluster: an SSH tunnel or proxy that forwards `127.0.0.1:8899` to a live cluster would skip confirmation and record no publication receipt. Deploy to a forwarded cluster through its real URL instead.

## Build and external requirements

Pina validates and redacts the explicit target before project discovery or any build begins. `--build` then invokes the same in-process project build workflow as `pina build` before Pina resolves and displays the final deployment plan. It does not search `PATH` for another Pina executable. A failed build stops immediately.

After confirmation, Pina copies the artifact and every keypair into a private, owner-only temporary directory (`pina-deploy-*`, mode `0700`, keypairs `0600`), revalidates the copies against the declared program ID and the SHA-256 fingerprints taken at planning time, and hands the child those copies rather than the original paths. A file replaced after the plan was displayed is therefore detected, and one replaced after the final check cannot reach the child. The displayed plan shows the paths you passed; the child's argument vector names the snapshot copies.

Without `--remote-command`, deployment requires the external `solana` executable from Agave on `PATH`; a custom deploy command does not. Pina runs every modeled command from the resolved project root, passes an argument vector directly rather than a shell command string, and closes the child's standard input after Pina handles confirmation. The npm-distributed Pina binary supports platforms on which Agave may not be available, so verify the local Agave installation before depending on deployment automation.

## Custom deploy commands

`--remote-command <COMMAND>` replaces `solana program deploy` with `sh -c <COMMAND>` (`cmd /C <COMMAND>` on Windows), for platforms that deploy through their own tooling. Every other safeguard still applies: target policy, confirmation, snapshot validation, and publication receipts. The deployment facts reach the command only as environment variables, never interpolated into the command string:

| Variable                        | Value                                      |
| ------------------------------- | ------------------------------------------ |
| `PINA_DEPLOY_RPC_URL`           | the normalized RPC URL                     |
| `PINA_DEPLOY_CLUSTER`           | the named cluster or `custom`              |
| `PINA_DEPLOY_PROGRAM`           | the snapshot copy of the SBF artifact      |
| `PINA_DEPLOY_PROGRAM_ID`        | the declared program ID                    |
| `PINA_DEPLOY_PROGRAM_KEYPAIR`   | the snapshot copy of the program keypair   |
| `PINA_DEPLOY_UPGRADE_AUTHORITY` | the snapshot copy of the upgrade authority |
| `PINA_DEPLOY_PAYER`             | the snapshot copy of the fee payer         |

Quote the variables inside the command (`"$PINA_DEPLOY_PAYER"`), and treat an exit status of zero as the command's claim that the deployment succeeded: Pina records the publication receipt on that claim alone.

## Migration publication receipts

For a program with checked-in migrations, a deployment to any non-loopback target freezes the ABI versions it ships. Before starting the deploy program, Pina writes a pending record to `migrations/publications.json`; after the program succeeds, it converts that record into a hash-chained receipt. From then on `pina migrations create` advances those contracts to a new version instead of rewriting them. `--record-publication` opts a loopback deployment into the same lifecycle, which is how a Surfpool run exercises it.

If the deploy program starts and then fails, the pending record stays, because the program may already be live. Rerunning the exact same deployment reconciles it, and `pina migrations reconcile` explains the state. If the deploy program never started, for example because `solana` is not installed, Pina discards the pending record it just wrote and says so, because nothing can have reached the cluster. A pending record left by an earlier attempt is never discarded this way.

## Output contract

Without `--json`, stdout contains the plan and completion status. Diagnostics, confirmation, and child failures use stderr. `--dry-run --json` emits one JSON object to stdout and no progress text. The full accepted RPC host and path appear in both formats and in the Solana argument plan. Missing files, malformed keypairs, program-ID mismatches, ambiguous projects, invalid RPC URLs, rejected confirmations, missing executables, signaled children, and non-zero child exits all fail closed.
