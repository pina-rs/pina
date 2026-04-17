# Custom Dylint Lints

<br>

Pina ships custom [dylint](https://github.com/trailofbits/dylint) libraries for Solana development. These lints are meant to be consumed by end users and example authors as part of the normal build and review loop.

They focus on the safety properties that matter most in this codebase:

- account ownership and signer validation
- CPI target verification
- writable / resize / close preconditions
- sysvar and ATA safety
- zero-copy and type-cosplay protection
- IDL-friendly example structure
- performance-sensitive on-chain code

## How to use the lints

<br>

The workspace registers all Pina lint libraries in `Cargo.toml`:

```toml
[workspace.metadata.dylint]
libraries = [
	{ path = "lints/require_owner_before_token_cast" },
	{ path = "lints/require_empty_before_init" },
	{ path = "lints/require_program_check_before_cpi" },
	{ path = "lints/deny_heap_allocations_in_onchain_instruction_handlers" },
	{ path = "lints/require_program_owned_before_lamport_mutation" },
	{ path = "lints/require_writable_before_account_resize" },
	{ path = "lints/require_zeroed_before_close" },
	{ path = "lints/require_sysvar_assert_before_sysvar_use" },
	{ path = "lints/require_type_assert_before_zero_copy_cast" },
	{ path = "lints/require_associated_token_address_before_ata_cast" },
	{ path = "lints/require_idl_root_to_define_one_program_id" },
	{ path = "lints/require_canonical_instruction_dispatch_for_idl" },
	{ path = "lints/require_explicit_discriminators_and_seed_namespaces" },
]
```

Run the full lint set with:

```sh
cargo dylint --all -- --all-targets
```

That command is the easiest way to check:

- workspace crates
- tests
- examples
- security fixtures

If you only want to inspect a specific package, run `cargo dylint` with that package's manifest or build scope instead.

## Lints shipped by Pina

<br>

### Security and correctness

- `require_owner_before_token_cast` — require `assert_owner()` / `assert_owners()` before token casts.
- `require_empty_before_init` — require `assert_empty()` before creating program accounts.
- `require_program_check_before_cpi` — require a program-address check before CPI.
- `require_program_owned_before_lamport_mutation` — require program ownership before direct lamport mutation.
- `require_writable_before_account_resize` — require `assert_writable()` before `resize()`.
- `require_zeroed_before_close` — require `zeroed()` before closing accounts.
- `require_sysvar_assert_before_sysvar_use` — require `assert_sysvar()` before sysvar reads.
- `require_type_assert_before_zero_copy_cast` — require type validation before raw zero-copy casts.
- `require_associated_token_address_before_ata_cast` — require ATA derivation checks before ATA casts.

### Performance

- `deny_heap_allocations_in_onchain_instruction_handlers` — discourage `Vec`, `String`, `format!`, and similar heap-heavy patterns in on-chain handlers.

### IDL generation and example structure

- `require_idl_root_to_define_one_program_id` — keep one clear `declare_id!` at the crate root of IDL examples.
- `require_canonical_instruction_dispatch_for_idl` — keep instruction routing as a direct `match` in example entrypoints.
- `require_explicit_discriminators_and_seed_namespaces` — keep explicit discriminators and visible byte-string seed namespaces in example code.

## Suggested workflow

<br>

1. Add or edit an example / security crate.
2. Run `cargo dylint --all -- --all-targets`.
3. Fix any lint failures before merging.
4. Keep the code shaped the way `pina idl` expects so the examples remain easy to introspect.

## Notes

- The current lints are intentionally conservative and tuned to the shapes Pina already uses.
- Some of the newer lints are heuristic-based; they are designed to catch likely mistakes early without making the codebase noisy.
- If a lint warning seems wrong, prefer adjusting the code shape to something more explicit and IDL-friendly.
