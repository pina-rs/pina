---
pina: none
---

# Unroll the repeated grow-then-shrink compact proof

The `compact_repeated_grow_and_shrink_operations_preserve_logical_values` Kani harness drove its two updates through a runtime `for` loop. Under the harness unwind bound, CBMC unrolled that loop over the full update-and-validate condition, and the per-element array validation that pinapod 0.4.0 introduced for `ZcValidate for [T; N]` multiplied the condition size until the harness stopped finishing: the compact job hit its 50-minute ceiling on `main` and every run since `430a2e95`.

The two updates are now unrolled by hand. Every operation and every assertion is preserved — the same initialize, the same two `update` calls with the same replacement lengths, and the same `bytes`/`words`/`triples` assertions after each — and the harness verifies in seconds again.
