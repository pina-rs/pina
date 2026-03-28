---
default: patch
pina: patch
---

Improve diagnostics and validation ergonomics in `pina`.

- `parse_instruction` still remaps `ProgramError::Custom(_)` discriminator errors to `InvalidInstructionData` for compatibility, but now logs the original custom error code when the `logs` feature is enabled.
- The escrow example now adds stronger account checks in both `Make` and `Take` flows, including explicit system program ID validation, vault owner validation, and associated-token-address validation for `taker_ata_b` and `maker_ata_b` before CPIs.
- Regenerated escrow Codama IDL and generated clients to reflect account metadata changes (default `systemProgram` and writable ATA fields where required).
- Clean up internal test assertions in `traits.rs` to avoid unnecessary qualification warnings.
- Make `PinaProgramError` independent from proc-macro expansion so `pina` now compiles with `--no-default-features` (without requiring the `derive` feature), and add regression coverage to keep the enum wire-size aligned to `u32`.
- Add a dedicated CI/devenv build gate (`build:pina:no-default`) to continuously verify `pina` no-default feature compatibility across key feature subsets.
