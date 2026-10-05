# Rehearse an Upgrade

`pina rehearse` replays a deployed program's real transactions against the binary you are about to ship, before you ship it. Every recent transaction runs twice on the same forked state, once against the deployed program and once against the candidate, and every difference in outcome, written account state, and compute units is reported.

```bash
# Rehearse the conventional artifact against the latest 25 devnet transactions.
pina rehearse --network devnet

# Build first, and replay a deeper sample of mainnet traffic.
pina rehearse --network mainnet --build --limit 100

# Rehearse an explicit binary against a custom endpoint.
pina rehearse --rpc-url http://127.0.0.1:8899 --program ./target/deploy/my_program.so

# Rehearse specific transactions instead of the latest ones.
pina rehearse --network devnet --signature <SIGNATURE> --signature <SIGNATURE>
```

Nothing is sent to the cluster and no project file is written. Pair it with [`pina migrations`](./migrations.md) and [`pina deploy`](./deploy.md): migrations prove the new code can read old data, and a rehearsal proves it still behaves the same on the traffic your users actually send.

To gate the upgrade itself, run the rehearsal as part of the deployment with [`pina deploy --rehearse`](./deploy.md#rehearse-first). It rehearses the exact artifact the deployment plan pins, against the cluster the deployment targets, and stops the deployment before anything is sent when behaviour changes or nothing could be compared.

## How a rehearsal runs

1. **Fetch.** Pina confirms the program exists on the cluster (`getAccountInfo`), so a program that was never deployed fails before any Surfpool starts. It then asks the cluster for the program's most recent signatures (`getSignaturesForAddress`, at `confirmed` commitment) and fetches each transaction exactly as it landed. With `--signature`, only the named transactions are fetched.
2. **Fork.** Pina starts a private Surfpool forked from the same RPC endpoint, on free loopback ports, inside a temporary directory, and waits up to a minute for it to answer. The fork is always stopped: on success, on error, on panic, and on Unix even when `pina` is interrupted or killed, because Surfpool runs under a small supervisor that stops it as soon as `pina` exits.
3. **Freeze.** Every account the transactions touch is loaded into the fork in one pass. Accounts that do not exist are marked offline, so the remote is never consulted again: both runs see one frozen snapshot, even on a busy cluster.
4. **Baseline.** Each signed transaction is profiled against the deployed program. Profiling executes on a throwaway copy of the fork, so nothing commits and every transaction sees the same snapshot.
5. **Install.** The candidate is written into the program's own program-data account, as an upgrade would write it: the same account and upgrade authority, with the old executable's tail zeroed.
6. **Candidate.** Each transaction is profiled again and compared with its baseline.

Two Surfpool behaviours shape the design:

- **Profiling validates blockhash age.** Real traffic carries blockhashes that expired long ago and would fail with `Blockhash not found`, so the fork runs with `--skip-blockhash-check`. Signature verification stays on: only authentic signed transactions are replayed.
- **Surfpool hides failed program loads.** When it cannot load an ELF, it logs the error and keeps executing the previously cached program, so a broken candidate could otherwise rehearse as "unchanged". Pina confirms every program swap: it rewrites the program account with one extra lamport, which the runtime only stores after loading the ELF, and reads it back. A candidate the runtime rejects fails the rehearsal, because an upgrade to that ELF would be rejected the same way.

The fork's clock never advances (`--block-production-mode manual`), so programs that read the `Clock` sysvar see the same time in both runs.

## Reading the report

Each transaction gets one status:

| Status            | Meaning                                                                   | Fails the rehearsal |
| ----------------- | ------------------------------------------------------------------------- | ------------------- |
| `unchanged`       | Identical outcome, written account state, and compute units               | No                  |
| `cu_changed`      | Identical outcome and state; compute units differ                         | No                  |
| `state_changed`   | Both runs succeed but leave a writable account different                  | Yes                 |
| `outcome_changed` | One run fails and the other succeeds, or both fail with different errors  | Yes                 |
| `skipped`         | Not compared, with a reason (`failed_in_both`, `unavailable`, and others) | No                  |

Both runs execute the same signed transaction on the same snapshot, so any difference belongs to the binaries. In particular:

- **A transaction that fails identically in both runs is skipped (`failed_in_both`).** This is the common case for older traffic: an `initialize` whose account now exists, or a transfer from an account that has since been drained. It says nothing about the upgrade, so it is listed and never counted as a regression.
- **A transaction that fails in both runs with different errors is an outcome change.** Error codes are part of a program's observable contract; clients and other programs branch on them.
- **A transaction that failed in the baseline but succeeds with the candidate is an outcome change.** The new code accepts something the deployed code rejects, which deserves review before it ships.
- **A transaction Surfpool refuses to run is skipped (`not_profiled`) only when both runs refuse it identically.** Surfpool refuses before the program executes, while it verifies signatures and loads accounts and lookup tables, so a refusal cannot depend on the binary: the same refusal twice comes from the forked state, such as a lookup table closed since. A refusal in only one run, or a different one in each, can only be the environment failing, and stops the rehearsal with exit code `1`.

For `state_changed` transactions the report lists every writable account whose final state differs: lamports, owner, data length, and the data itself. For accounts the program owns, the account type is matched by discriminator and the differing bytes are decoded field by field from the project's IR, baseline value first:

```text
2y8JkzTSum83V1nSkpy64WHZv1S8pqWFMJ8YRxP3pqtqgwTBdyKGMwZ1rBsg4auwogTkpoJunSjxcBnj8XG4EECP  state_changed  [increment]
  compute units 473 -> 473 (0)
  account 14P1xLXPqLmxipH1tno2oKiS9GQPZJosNuK1AJPe4Ffn (CounterState)
    count: 4 -> 5
```

Fixed `PinaPod` layouts decode integers, booleans, addresses, and floats, and the discriminator and migration-version header are named too. Compact layouts, and bytes no field covers, are reported as byte ranges. Account differences are computed only when both runs succeed, because a failed run commits no state. For outcome changes the report shows the end of each run's logs instead.

The compute-unit table covers transactions that succeeded in both runs, per instruction of your program: minimum, median, and maximum for each binary, and the change in the median. The median of an even sample is the lower middle value.

## Exit status

| Code | Meaning                                                                                                                                               |
| ---- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| `0`  | At least one transaction was compared and none changed outcome or state. Compute-unit changes alone pass.                                             |
| `2`  | At least one `state_changed` or `outcome_changed` transaction, without `--allow-changes`                                                              |
| `3`  | No transaction was compared: every one was skipped, or there was no traffic. Nothing was verified.                                                    |
| `1`  | Operational error: invalid input, missing Surfpool, a program that is not deployed, an RPC failure, a candidate the runtime rejects. Stdout is empty. |

This follows the CLI's convention for comparisons (`pina profile compare`, `pina idl diff`, `pina verify check`): `2` means the comparison completed and found a difference. Clap also exits `2` for invalid arguments, but then prints usage to stderr and nothing to stdout. Use `--allow-changes` once the differences are reviewed and intended.

A rehearsal that compares nothing is never a pass, so `0` always means the upgrade was exercised. With exit code `3` the report still prints, and its skipped section explains each transaction. `--allow-changes` does not change it: there are no changes to accept. Rehearse more or newer traffic with `--limit` or `--signature`, or treat `3` as acceptable in automation for a program that has no traffic yet.

## JSON report

`--json` prints one stable document with camelCase keys. Progress lines go to stderr, so stdout stays a single document.

```json
{
	"schemaVersion": 1,
	"cluster": "devnet",
	"programId": "GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS",
	"deployedSha256": "1e62…",
	"candidateSha256": "d380…",
	"slot": 2001,
	"summary": {
		"total": 4,
		"unchanged": 0,
		"cuChanged": 0,
		"stateChanged": 3,
		"outcomeChanged": 0,
		"skipped": 1
	},
	"instructions": [
		{
			"name": "increment",
			"samples": 3,
			"baseline": { "min": 473, "median": 473, "max": 473 },
			"candidate": { "min": 473, "median": 473, "max": 473 },
			"medianDelta": 0
		}
	],
	"transactions": [
		{
			"signature": "2y8J…",
			"status": "state_changed",
			"skip": null,
			"instructions": [
				{ "name": "increment", "baselineUnits": 473, "candidateUnits": 473 }
			],
			"baseline": { "error": null, "computeUnits": 473 },
			"candidate": { "error": null, "computeUnits": 473 },
			"accounts": [
				{
					"address": "14P1…",
					"accountType": "CounterState",
					"baseline": { "lamports": 967440, "owner": "GJQc…", "dataLen": 11 },
					"candidate": { "lamports": 967440, "owner": "GJQc…", "dataLen": 11 },
					"fields": [{ "name": "count", "baseline": "4", "candidate": "5" }],
					"byteRanges": [],
					"omittedByteRanges": 0
				}
			],
			"logs": null
		}
	]
}
```

Every key is always present; absent values are `null`. `status` and `skip.reason` use the snake_case spellings shown in the status table. Executable hashes are SHA-256 with trailing zero padding removed, matching `solana-verify`, so a deployed program's zero-padded program data and its local `.so` hash the same. The `schemaVersion` changes only with a breaking change to the document.

## Requirements and limits

- Surfpool 1.6.0 or newer must be on `PATH`, or named by `PINA_SURFPOOL`. Pina checks the version before reading the project.
- On Windows, Surfpool runs as a direct child: it is stopped on every normal exit path, but a console interrupt that terminates `pina` while Surfpool is still starting can leave it running.
- The deployed program must use the upgradeable loader, as `solana program deploy` and `pina deploy` do.
- The program ID is the project's `declare_id!`. The candidate defaults to `<cargo-target>/deploy/<library-name>.so`; `--program` names another file and `--build` runs the same build as `pina build` first.
- Transactions replay against the cluster's current state, not the state they originally saw, and each one sees the same snapshot: state written by one is not visible to the next.
- Legacy and v0 messages are supported. A transaction the RPC no longer returns is skipped as `unavailable`; one it cannot return in a supported version is skipped as `undecodable`.
- `--limit` accepts 1 to 1000, a single `getSignaturesForAddress` page.

## Network safety

Pina only reads from the cluster: one `getAccountInfo` request for the program, one `getSignaturesForAddress` request, and one `getTransaction` request per transaction, each sent exactly once with a timeout. A failed request stops the rehearsal with exit code `1` instead of being retried, so a rate-limited endpoint is not hammered; use a dedicated RPC endpoint or a smaller `--limit` when a public endpoint refuses. That includes JSON-RPC errors returned inside an HTTP 200 response, such as an unhealthy node (`-32005`) or a provider's rate limit. The one exception is `-32015`, the RPC's answer for a transaction it cannot encode in a supported version: that transaction is skipped as `undecodable`. Redirects are not followed. The fork fetches each account the transactions touch from the same endpoint once.

A custom `--rpc-url` must use HTTP or HTTPS with a host. Pina rejects user information, query parameters, fragments, and control characters. Surfpool receives the URL as a child-process argument, where local process inspection can reveal it, so never put a secret anywhere in the URL, including its path. Reports show only the URL's origin.

## Options

| Option                   | Meaning                                                    |
| ------------------------ | ---------------------------------------------------------- |
| `--project <DIR>`        | Project directory or a directory below it                  |
| `--network <CLUSTER>`    | Rehearse against `mainnet`, `devnet`, or `testnet`         |
| `--rpc-url <URL>`        | Rehearse against a credential-free HTTP(S) RPC endpoint    |
| `--program <PROGRAM.SO>` | Candidate binary; conflicts with `--build`                 |
| `--build`                | Run the `pina build` build first and rehearse its artifact |
| `--limit <N>`            | Recent transactions to replay, 1 to 1000 (default 25)      |
| `--signature <SIG>`      | Rehearse this transaction instead; repeatable              |
| `--json`                 | Print the stable JSON report                               |
| `--allow-changes`        | Exit `0` even when behaviour changed                       |

Run `pina rehearse --help` for the authoritative command contract.
