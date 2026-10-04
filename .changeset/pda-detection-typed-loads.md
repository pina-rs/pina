---
pina_cli: breaking
pina_macros: docs
pina_skill: docs
---

# Detect PDAs behind stored bumps, typed loads, and helpers

`pina idl` and `pina generate` missed PDA accounts whose processor never called one of the few validators the extractor knew, so generated clients made callers pass addresses Pina could have derived. Three gaps are closed:

- **Every generated PDA validator counts.** `State::assert_stored_bump(..)`, `State::load_checked_pda(..)`, and `State::load_checked_pda_mut(..)` now mark their account as a PDA, like `assert_seeds` and `load_pda` already did. The type in the call names the PDA, so a field called `vault` validated by `PoolVault::assert_seeds` links to `pool_vault` instead of failing on its field name.
- **Typed loads name the PDA.** `self.state.as_account::<State>(..)`, `as_account_mut`, `with_compact_account`, `update_compact_account`, `assert_type`, and `assert_compact_type` link the slot to the PDA `State` declares with `#[pda]`. A PDA whose seeds are all constants has one address, so clients now fill in singletons such as a pool config or Merkle tree. A variable-seed PDA loaded only by type records which PDA the slot belongs to but gets no client default: the processor never ties the address to seeds, so a seed that shares a name with another account is no proof that account supplies it. A typed load of an account without `#[pda]` changes nothing.
- **Helpers are followed.** When `process` passes an account field to a module-level helper function, the helper's assertions count for that field, up to four calls deep. Signers asserted inside helpers are now marked as signers too, so a client no longer builds a transaction the program rejects with `MissingRequiredSignature`. `pina explain` reports a check found in a helper at the helper's own file and line.

`InstructionAccountIr::pda_name` can now be set while `is_pda` is false, for the variable-seed typed load above; `is_pda` keeps meaning that the processor pins the address.

The validation guide shared by the `pina_macros` readme and the agent skill's program-authoring reference (`pina_skill`) now says that inference also reads module-level helpers and typed loads of `#[pda]` account types.

## Migrating

Regenerate your clients after upgrading. Generated Rust instruction builders no longer take accounts Pina now derives: for example, the privacy pool example's `Deposit::new(depositor, pool_config, merkle_tree, note_commitment)` becomes `Deposit::new(depositor, note_commitment)`, deriving `pool_config` and `merkle_tree` from their constant seeds. Remove the derived arguments from your calls, or set the field on the returned builder to use a different account. In the TypeScript and Dart clients those accounts become optional inputs, so existing calls keep compiling.

An account newly detected as a signer is a wire change. Generated clients now require that signer. A migration-aware instruction that records the slot is re-recorded by `pina migrations create` while it is unpublished. Against a published version, `pina migrations check` reports the drift: the program always required that signature, so the recorded snapshot was wrong, and the remedy is the one for any process change, a new instruction discriminator. A slot that only gains a PDA or known-address hint needs nothing, because hints are not part of process compatibility.
