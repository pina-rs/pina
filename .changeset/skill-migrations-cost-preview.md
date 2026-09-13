---
pina_skill: feat
---

# Document the migration cost preview in the bundled skill

`references/migrations.md` now covers the `pina migrations status` cost preview: per-contract current size, day-one growth, and rent deficit at the shared 6,960-lamports-per-byte convention; per-instruction worst-case ladders bounded by `MAX_INLINE_STEPS` (8); and the program-wide summary that names the transaction funding the most rent and, independently, the one carrying the longest ladder. It records that the static CU figure sums `pina profile` estimates, excludes executor and runtime effects, prints `CU unavailable: <reason>` instead of a zero, and that only writable non-signer instruction slots join against account contracts, with unlinked slots producing an explicit note.
