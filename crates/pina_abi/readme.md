# `pina_abi`

Canonical, host-side ABI history types shared by Pina's migration CLI and procedural macros. It owns the checked-in migration manifest and publication ledger, and the adjacent converters that read older Pina ABI documents into the current model before validation. The documents' `abiVersion` follows this crate's release line and is independent from application account, instruction, and event versions; it also fixes the wire codec every recorded schema is derived under (`SCHEMA_CODEC`). This crate is not linked into Solana programs.
