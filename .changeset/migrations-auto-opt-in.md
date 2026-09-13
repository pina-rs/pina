---
pina: feat
pina_abi: breaking
pina_cli: breaking
pina_macros: feat
---

# Add an auto opt-in to the migrations config

`[migrations].auto` in `pina.toml` accepts `true`, `false`, or a list of `accounts`, `events`, and `instructions`. `pina migrations make` records the resolved policy as `auto` in `migrations/manifest.json` — manifest format 4 — and snapshots every contract of the listed kinds.

The recorded policy and its status reach the public API of two crates: `pina_abi`'s manifest type gains the `auto` field, and `pina_cli` gains new public fields (`Project::migration_auto`, `MakeMigrationsOutput::{auto, build_script}`, the `migrations` field on the account, instruction, and event IR), new error variants (`ProjectError::{InvalidMigrationAuto, UnknownMigrationKind}` and `MigrationError::{AutoPolicyChanged, EnvelopeRemoval, BuildScriptRerunMissing}`), renames the IR's `migratable` field to `migrations`, and changes the arity of `extract_migratable_events`.

Macros now resolve opt-in as an explicit `migrations` token or the manifest's recorded policy, so a program with `auto = true` envelopes every contract without per-item annotations, while a contract that is not yet snapshotted still fails the build with the `pina migrations make` remedy. `migrations = false` overrides the policy for one contract and removing an envelope the manifest already records fails closed as a wire-format change. When a policy is recorded, `make` scaffolds an idempotent `build.rs` emitting `cargo:rerun-if-changed=migrations/manifest.json`, reports the exact line instead of rewriting a hand-written build script, and `check` verifies the directive.

The `examples/migrations_program` example now uses `auto = true` and marks its relay payload `migrations = false`.
