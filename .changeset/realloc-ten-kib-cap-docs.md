---
pina: docs
---

# Document the real 10 KiB realloc growth limit

Investigating #277 showed the per-instruction account growth cap has always been Solana's `MAX_PERMITTED_DATA_INCREASE` (`1_024 * 10` = 10,240 bytes); there has never been a 1 KiB cap. `AccountView::data_len()` reads the runtime-serialized length correctly, and the runtime itself rejects growth beyond 10 KiB with `InvalidRealloc`.

`MAX_PERMITTED_DATA_INCREASE`'s rustdoc now states the numeric value and its runtime enforcement, and the realloc example documents that its parity guard mirrors the runtime cap and is bounded further by the compact codec. New unit tests pin the constant to 10,240 bytes so any future change fails loudly instead of silently drifting.
