---
pina: feat
pina_macros: fix
---

# Verify stored-bump PDAs with sha256

The generated stored-bump loaders, `load_pda`, `load_pda_mut`, and `with_stored_bump_pda`, now re-derive the account's address from its stored bump with `sha256` instead of the `sol_create_program_address` syscall. That removes about 1,350 compute units from every load: the counter comparison fixture's `increment` fell from 1,738 to 378.

The syscall adds one check the hash does not: that the address lies off the ed25519 curve. It is redundant for these loaders. Each one first requires the program to own the account and its data to pass the type's checks, and the program can only have created an account at a seed-derived address through `invoke_signed`, which the runtime signs only for an off-curve address. The security model guide spells out the argument. Loaders and checks that validate a caller-supplied or canonical bump keep the full derivation.

The new `pina::is_derived_address` exposes the same check for hand-written loaders, with its contract documented. Anchor v2 and Quasar verify stored-bump PDAs the same way.
