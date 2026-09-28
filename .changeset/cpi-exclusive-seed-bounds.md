---
pina: fix
---

# Slice PDA-creation seeds and signers with exclusive bounds

The PDA-creation CPI spine assembled its derivation seeds, combined-seed signer, and signer list with inclusive ranges (`[a..=len]`). Every inclusive slice over a seed or signer array monomorphizes its own 216-byte `RangeInclusive<usize>::index` copy plus panic plumbing — three copies, roughly 1.3 KB of deployed text, in every program that creates a PDA through the checked or compact builders.

The spine now slices with exclusive bounds (`[a..len + 1]`, each guarded by the `len < MAX` check that already ran in the same function), which inline to a few instructions. Measured on the framework-comparison counter fixture with the exact comparison profile and proven by its verifier: the ELF shrinks 12,720 → 11,400 bytes (−10.4%) and `initialize` drops 3,295 → 3,203 compute units (−92) with byte-identical behavior; `increment` is unchanged. Behavior is identical because `[a..=len]` and `[a..len + 1]` denote the same half-open prefix; only the code shape changes.

The program-size guide documents the exclusive-bound shape for generated and user code, and the `pina_lean` comparison fixture — the measurement bed for ADR 0010's lean-entrypoint strategy — now combines this fix with a bounded entrypoint budget (`nostd_entrypoint!(process_instruction, 3)`) to measure the counter at 9,976 bytes (−21.6% from stock), 1,280 bytes from Anchor v2's 8,696 with the same account model and stricter validation.
