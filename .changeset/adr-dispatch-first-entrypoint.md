---
# ADR 0011 and the ADR index belong to `pina_root`, which is unpublished, so
# the coverage is recorded without a bump.
pina_root: none
---

# Propose a dispatch-first entrypoint in ADR 0011

ADR 0011 proposes an entrypoint that reads the instruction data from the SIMD-0321 pointer, dispatches before reading any account, and parses only the routed instruction's accounts with the derived checks unchanged. A size-only prototype measured the counter fixture at 7,960 bytes, down from 8,632, and at 7,720 when the entrypoint's error conversion folds, against Quasar's 7,808. ADR 0010 now points its dispatcher decision at the proposal, and the ADR index lists ADR 0010's accepted status correctly.
