---
pina_cli: breaking
---

# `pina_cli` build types gain fields for the size profile

`BuildOptions` replaces its `no_lto`, `no_size_profile`, and `overflow_checks` booleans with a single `size_profile: SizeProfile`. The new enum has `Production` (fat LTO, one codegen unit, overflow checks off), `ProductionWithOverflowChecks`, and `None`. `Project` gains `library_crate_types`, which reports the crate types declared by the program's library target so callers can tell whether a program can be linked with LTO.

Both structs are exhaustively constructible through the public API, so existing struct literals must set the new fields. The CLI flags are unchanged in name and meaning: `--no-lto` and `--no-size-profile` both select `SizeProfile::None`, and `--overflow-checks` selects `ProductionWithOverflowChecks`.
