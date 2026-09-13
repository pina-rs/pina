---
pina: feat
pina_abi: feat
pina_cli: feat
pina_macros: feat
---

# Add an auto opt-in to the migrations config

`[migrations].auto` in `pina.toml` accepts `true`, `false`, or a list of `accounts`, `events`, and `instructions`. `pina migrations make` records the resolved policy as `auto` in `migrations/manifest.json` — manifest format 4 — and snapshots every contract of the listed kinds.

Macros now resolve opt-in as an explicit `migrations` token or the manifest's recorded policy, so a program with `auto = true` envelopes every contract without per-item annotations, while a contract that is not yet snapshotted still fails the build with the `pina migrations make` remedy. `migrations = false` overrides the policy for one contract and removing an envelope the manifest already records fails closed as a wire-format change. When a policy is recorded, `make` scaffolds an idempotent `build.rs` emitting `cargo:rerun-if-changed=migrations/manifest.json`, reports the exact line instead of rewriting a hand-written build script, and `check` verifies the directive.

The `examples/migrations_program` example now uses `auto = true` and marks its relay payload `migrations = false`.
