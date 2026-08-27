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

`--project` accepts a project directory or any directory below it. Pina uses the same project discovery as `pina build`: the nearest ancestor `Pina.toml`, then unambiguous Cargo metadata as a fallback. The library target name and Cargo metadata target directory resolve:

- `<cargo-target>/deploy/<lib-name>.so`
- `<cargo-target>/deploy/<lib-name>-keypair.json`

Use `--program` or `--program-keypair` to override either conventional path. `--program` conflicts with `--build`. Every resolved file is canonicalized and must be a regular file. Keypair files cannot exceed 4 KiB and must contain a valid 64-byte Solana JSON keypair. On Unix, Pina rejects keypairs with any group or world permission bits; `chmod 600 <keypair>` is the recommended mode. Pina cannot inspect equivalent Windows ACL policy, so Windows operators must restrict keypair access with the operating system's ACL tooling. The program keypair address must match the program's `declare_id!`; deploy never generates or silently replaces an identity.

`--upgrade-authority` and `--payer` are always explicit. Pina does not inherit wallet paths from Solana configuration and does not store deployment credentials in `Pina.toml`.

## Targets and confirmation

One explicit target is required:

- `--cluster localnet`
- `--cluster devnet`
- `--cluster testnet`
- `--cluster mainnet-beta`
- `--cluster <HTTP(S)-URL>`

Custom endpoints reject URL user information, query parameters, and fragments. The Solana CLI accepts its RPC endpoint through `--url`, so every accepted host and path is visible in the dry-run plan, confirmation, and operating-system process listings. Pina cannot distinguish a provider token embedded in a path from a legitimate endpoint path. Never put a secret anywhere in a custom URL; prefer a named cluster when possible.

Local endpoints execute after displaying the plan. Every remote endpoint prompts the operator to type `deploy`. Non-interactive remote deployment fails before starting a child process unless `--yes` is supplied. Named mainnet and every custom remote endpoint additionally require `--allow-mainnet`, because Pina cannot prove which Solana cluster an arbitrary URL serves. The flag is rejected for localnet, devnet, testnet, and custom loopback endpoints.

## Build and external requirements

Pina validates and redacts the explicit target before project discovery or any build begins. `--build` then invokes the same in-process project build workflow as `pina build` before Pina resolves and displays the final deployment plan. It does not search `PATH` for another Pina executable. A failed build stops immediately. After confirmation, Pina revalidates the declared program ID, every keypair, and streamed SHA-256 fingerprints of the artifact and keypair files immediately before starting Solana. Changes detected by that final validation require the operator to review a new plan. As with any path-based external CLI handoff, a privileged local process could still replace a file between Pina's final validation and Solana opening it; keep deployment directories and keypairs writable only by the deploying user.

Deployment requires the external `solana` executable from Agave on `PATH`. Pina runs every modeled command from the resolved project root, passes an argument vector directly—never a shell command string—and closes the child's standard input after Pina handles confirmation. The npm-distributed Pina binary supports platforms on which Agave may not be available, so verify the local Agave installation before depending on deployment automation.

## Output contract

Without `--json`, stdout contains the plan and completion status. Diagnostics, confirmation, and child failures use stderr. `--dry-run --json` emits one JSON object to stdout and no progress text. The full accepted RPC host and path appear in both formats and in the Solana argument plan. Missing files, malformed keypairs, program-ID mismatches, ambiguous projects, invalid RPC URLs, rejected confirmations, missing executables, signaled children, and non-zero child exits all fail closed.
