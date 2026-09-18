---
pina_cli: none
---

# Align the advisory gates on deny.toml

`security:deny` ran `cargo-deny check bans licenses sources`, so the `[advisories]` section of `deny.toml` — and the four suppressions it documented — never executed. The task now runs the advisories check with `--workspace`, because cargo-deny's default graph roots at the workspace members' normal dependencies and therefore omits the dev/test stack (surfpool, litesvm, mollusk, solana-runtime) where every suppressed advisory actually lives. Without `--workspace` the check cannot see them at all.

`security:audit` carried a second, different seven-id ignore list. `deny.toml` is now the single source of advisory policy: the task reads the `[advisories] ignore` array out of `deny.toml` and passes each id to `cargo-audit`, so the two gates cannot drift. Every suppression in that array carries a reachability justification and a review date, and each needs a tracking issue filed on the repository — the entries are all dev/test-only today, verified with `cargo tree -i <spec> -p pina -p pina_abi -p pina_macros -p pina_cli` and the `--workspace` cargo-deny run, but a file cannot open issues on its own.

The `[advisories]` section also needed the `RUSTSEC-2021-0139`, `RUSTSEC-2024-0375`, `RUSTSEC-2024-0384`, `RUSTSEC-2020-0016`, `RUSTSEC-2025-0134`, and `RUSTSEC-2026-0173` entries that only the old cargo-audit list had, so moving advisories into the deny gate does not weaken it.
