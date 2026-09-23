---
pina: patch
pina_cli: patch
---

# Close the live 2026-09-22 audit findings in the runtime and CLI

`UpdateResizableAccount` now routes every feature combination through the generated account-level update contract, so a compact update on a stale migration envelope is refused — and the envelope advanced — even without the `validation` feature. The preflight-versus-commit length agreement is a release-mode error instead of a compiled-out `debug_assert`. `pinapod` is now exact-pinned (`=0.4.3`) like `fixed`, since its generated layouts are the wire contract.

`pina import` redacts URL query credentials and fragments from the echoed source, the persisted provenance README, and the import outcome. The exported verification transaction is no longer accepted by encoding and length alone: the payload must deserialize as a structurally valid Solana transaction (legacy or versioned) with in-range account indices and a non-empty instruction list before a byte is written.
