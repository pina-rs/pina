---
pina: fix
pina_macros: breaking
pina_abi: fix
---

# Close the confirmed 2026-09-19 sweep findings

`#[derive(Accounts)]` now rejects optional account fields followed by positional fields at compile time: an absent optional consumes its program-address filler slot, so dropping the filler shifted every later binding by one and each validation ran against the wrong account. Move optional fields to the end; a trailing `remaining` slice remains allowed.

The generated PDA seed slices now emit in declaration order instead of constants-first, so derived addresses match the order the IDL publishes and clients derive the same address the program verifies.

Enveloped instructions now pin the migration version byte at dispatch through an `IntoDiscriminator` gate derived from the checked-in manifest: a missing or unknown version byte fails closed before any handler runs, closing the zero-field fail-open. Programs whose manifests declare no instruction contracts expand to identical code.

A migration manifest whose `rustName` is not a plain Rust identifier now fails validation with a typed error instead of panicking inside the macro expansion, and duplicate field names within one schema version are rejected per version.

The account migration executor zero-fills the grown region before each transition applies, matching the instruction and event workspaces, so a hand-written transition that skips an added field commits zeros rather than the account's own realloc residue.
