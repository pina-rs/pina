---
pina_codama_renderer: fix
---

# Emit lint-clean migration checks for version-zero clients

Generated Rust clients compared the stored `migrationVersion` against the current-version constant even when that constant was `0`, so the stale check `stored < 0` (and the equivalent comparison inside `{account}_needs_migration`) was always false and tripped the deny-by-default `clippy::absurd_extreme_comparisons` in any generated crate. The renderer now omits the impossible stale arm and emits the constant-false `needs_migration` body for version 0, and symmetrically omits the future arm when the current version equals its type maximum. Emitted semantics are unchanged — stale, future, and current envelopes are distinguished exactly as before — and the embedded contract tests now assert only the reachable cases, with version 0 pinning current versus future.
