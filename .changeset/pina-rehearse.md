---
pina_cli: feat
pina_skill: docs
---

# Rehearse upgrades against real traffic with `pina rehearse`

`pina rehearse` replays a deployed program's recent transactions against the build you are about to ship, before you ship it. It fetches the program's latest transactions (`--limit`, default 25, up to 1000) or the ones named with `--signature`, read-only, then starts a private Surfpool forked from the same RPC endpoint and profiles every signed transaction twice on one frozen snapshot: against the deployed program, and after the candidate is installed in the program's own program data. Select the cluster with `--network mainnet|devnet|testnet` or a credential-free `--rpc-url`; the candidate is the project's `target/deploy` artifact, `--program`, or a fresh `--build`.

Each transaction is reported as `unchanged`, `cu_changed`, `state_changed`, `outcome_changed`, or `skipped`. A transaction that fails identically in both runs is skipped as `failed_in_both`, because its state has moved on and it says nothing about the upgrade; different errors in both runs count as an outcome change. Changed account state is decoded field by field from the project's IR, with byte ranges for anything a fixed layout does not cover, and a per-instruction compute-unit table compares the two binaries. `--json` prints a stable camelCase document with `schemaVersion: 1`. The command exits `2` when behaviour changed (unless `--allow-changes`), `3` when no transaction could be compared so nothing was verified, `0` otherwise, and `1` for operational errors; compute-unit changes alone never fail.

The fork skips blockhash validation, because real traffic carries expired blockhashes and Surfpool's profiler rejects them, and freezes its clock so both runs see the same time. Surfpool silently keeps a previously cached program when it cannot load an ELF, so every program swap is confirmed with a sentinel write the runtime only stores after loading; a candidate the runtime rejects fails the rehearsal rather than rehearsing as unchanged. Requests are never retried and any failed request stops the rehearsal, including a JSON-RPC error such as a rate limit; only a transaction the RPC cannot encode in a supported version (`-32015`) is skipped. A transaction Surfpool refuses is skipped only when both runs refuse it identically. Redirects are not followed, and reports show only a custom URL's origin. A program that does not exist on the cluster fails before Surfpool starts. The command requires Surfpool 1.6 or newer.

The agent skill's CLI reference describes the rehearsal workflow and how to read its exit status.
