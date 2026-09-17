---
pina_cli: feat
---

# Scaffold migrations for new projects

`pina init` now enables ABI migrations by default. The generated `pina.toml` carries a `[migrations]` section with `version_type = "u8"` and `auto = true`, and the scaffold includes the `build.rs` that emits `cargo:rerun-if-changed=migrations/manifest.json`, so a policy flip re-expands the macros without a source edit.

The manifest is deliberately _not_ scaffolded. A recorded history is bound to the program address declared in the source, and a new project still carries the placeholder `declare_id!`, so pre-recording it would pin the history to an address the user is about to replace. `pina migrations make` remains the bootstrap step; the scaffolded next-steps output now lists it first and says to set the program address before running it. Nothing else changes: a project with an `auto` policy and no manifest already fails `pina build` and `pina migrations check` with the `pina migrations make` remedy, so it cannot be built unenveloped by accident.

`pina migrations status` reports the remaining version budget per contract — `account State v0 (draft, 255 version(s) remaining)` — and the same value reaches `status --json` and `check --json` as `versionsRemaining`. `VersionExhausted` now explains the remedy instead of stating the condition: versions are counted per contract, the width cannot be widened after the first publication, and the path forward is a successor contract with a new discriminator plus a bridge instruction.

New documentation in `docs/src/migrations/flow.md` covers version exhaustion end to end: why `u8` is the default, the pre-launch re-baseline that is the only widening path, and the successor-contract remedy that replaces history pruning — which a program cannot do, because it cannot enumerate its own accounts.
