---
pina_cli: feat
---

# Report and scaffold the deployed-size settings

`pina doctor` gains a `project.size-profile` check that names the two ways a program silently pays for bytes it does not ship. A second crate type — `["cdylib", "lib"]` — makes rustc refuse LTO, which measured 28.5% on one real program and +40% on another when lost; the check reports the crate types, the measured cost, and the remedy of moving shared logic into a separate crate. A `[profile.release]` without `lto` reports the single largest size reduction it is leaving on the table. A cdylib-only target with LTO passes.

`pina init` now scaffolds `panic = "abort"`, `strip = true`, and a `[profile.release.build-override]` block alongside the existing `lto` and `codegen-units` settings, so a new program starts from the profile that real programs arrived at by measurement. Build scripts and proc macros no longer run at the deployed program's opt-level. The crate-type stays `cdylib`-only, which the scaffold already documented as the default that keeps LTO working.
