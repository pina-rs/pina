---
pina: fix
---

# Adopt the Pinapod 0.4.3 compact-writer hardening

Move from the temporary `pinapod` 0.4.1 hold to 0.4.3, which brings the 0.4.2 commit-entry hardening at its intended compute-unit cost. 0.4.2 validates every generated compact buffer before `commit` relocates tails, so a stale or tampered writer fails closed instead of computing offsets over bytes nobody checked, and a preflight and commit length disagreement returns `InvalidLength` in release builds instead of relying on a debug assertion. 0.4.3 keeps both properties and restores the compute-unit profile.

The intermediate 0.4.2 release was held back on a diagnosis that turned out to be wrong: Pina reported that 0.4.2 moved `commit` ahead of the inline writes and turned a length check into an unconditional panic. Neither was true — the panic-shaped block was 0.4.1's own `debug_assert_eq!` expansion, and `update` never changed order. [pinapod#36](https://github.com/pina-rs/pinapod/issues/36) recorded the correction and [pinapod#38](https://github.com/pina-rs/pinapod/issues/38) specified how to keep the safety without the cost, which is what 0.4.3 implements.

`commit` entry now proves layout rather than semantics through `PinaPodCompact::validate_layout`: the storage length, each stored length prefix, its capacity, and the chained bounds. Relocation's pointer arithmetic consumes exactly those, so this is the complete precondition and not a weakening of it. The element walks that checked UTF-8, enum ranges, and tags move to the boundaries that interpret those bytes, which already had them — the `Ref` and `Mut` constructors, `updated_len`, and `try_initialize`'s post-commit check. Trivially valid element types such as `u64` and `u8` skip their walks entirely rather than relying on the compiler to delete a dead loop, which SBF at `-C opt-level=3` does not reliably do.

An account that is layout-valid but semantically invalid, such as non-UTF-8 bytes in an untouched tail, now commits and is rejected at the next read instead of by `commit` itself. The update path keeps its existing guarantee by construction, because `updated_len` runs the full validation over the same bytes the staged edits will produce. A `compact-commit-full-validation` feature restores the full walk at commit entry for consumers who prefer it.

The observable effect on Pina is the regenerated `pda.expanded.rs` snapshot, and the compute units that 0.4.2 added are gone: the `compact_accounts_program` instructions that regressed by 46 to 308 units under 0.4.2 measure at or below their 0.4.1 cost again.
