---
pina_cli: breaking
---

# `pina_cli` build types gain fields for the size profile

`BuildOptions` replaces its `no_lto`, `no_size_profile`, and `overflow_checks` booleans with a single `size_profile: SizeProfile`. The new enum has `Production` (fat LTO, one codegen unit, overflow checks off), `ProductionWithoutLto` (the same profile with LTO turned off), `ProductionWithOverflowChecks`, and `None`. `Project` gains `library_crate_types`, which reports the crate types declared by the program's library target so callers can tell whether a program can be linked with LTO.

Both structs are exhaustively constructible through the public API, so existing struct literals must set the new fields. The CLI flags keep their names and meanings: `--overflow-checks` selects `ProductionWithOverflowChecks`, `--no-lto` selects `ProductionWithoutLto`, and `--no-size-profile` selects `None` and leaves the program's own release profile alone.

`ProductionWithoutLto` exists because `--no-lto` has to disable LTO _as an override_. Leaving the setting untouched would let a program whose manifest declares `lto = "fat"` — which `pina init` generates — keep LTO enabled while the flag claimed to turn it off.
