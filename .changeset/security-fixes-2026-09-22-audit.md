---
pina: fix
pina_cli: fix
pina_codama_nodes: fix
---

# Close live 2026-09-22 audit findings: runtime and CLI

`UpdateResizableAccount` now routes every feature combination through the generated account-level update contract, so a compact update on a stale migration envelope is refused — and the envelope advanced — even without the `validation` feature. The preflight-versus-commit length agreement is a release-mode error instead of a compiled-out `debug_assert`. `pinapod` is now exact-pinned (`=0.4.3`) like `fixed`, since its generated layouts are the wire contract.

The pinapod and fixed pins are a downstream contract, not just an internal one: `pina` re-exports both crates (`pina::pinapod`, `pina::fixed` behind the `fixed` feature), so consumers derive fixed-point schemas against the only `fixed` build pinapod recognizes without adding either dependency. `security/regressions/sec33-downstream-reexport` proves the contract by building against `pina` alone; upstream pina-rs/pinapod#41 tracks the pinapod-side re-export that would let the duplicate `fixed` entry disappear.

The generated Codama node package's client contract test follows the retired example ABIs, so its assertions move with the instruction account lists.

`pina import` redacts URL query credentials and fragments from the echoed source, the persisted provenance README, and the import outcome. The exported verification transaction is no longer accepted by encoding and length alone: the payload must deserialize as a structurally valid Solana transaction (legacy or versioned) with in-range account indices and a non-empty instruction list before a byte is written.
