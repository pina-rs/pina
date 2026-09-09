---
pina: feat
pina_macros: feat
pina_cli: feat
pina_cpi_renderer: fix
pina_skill: docs
---

# Add Declarative Validation

Add feature-gated, allocation-free validation annotations for accounts, instructions, events, and instruction account lists. Teach Codama extraction about client-visible account constraints while preserving manual validation-chain inference, and publish the complete API through the crate, book, terminal docs, and Pina skill.

## Before and After

Programs can keep using direct checks:

```rust
let args = TransferInstruction::try_from_bytes(data)?;
if args.amount() == 0 || args.amount() > 1_000_000 {
	return Err(TransferError::InvalidAmount.into());
}

self.authority.assert_signer()?;
self.source.assert_owner(&ID)?.assert_not_empty()?;
```

With the `validation` feature, the equivalent reusable contract can live beside the affected fields:

```rust
#[instruction(discriminator = Instruction::Transfer)]
pub struct TransferInstruction {
	#[pina(validate(
		min = 1,
		max = 1_000_000,
		error = TransferError::InvalidAmount
	))]
	pub amount: u64,
}

#[derive(Accounts)]
pub struct TransferAccounts<'a> {
	#[pina(validate(signer))]
	pub authority: &'a AccountView,

	#[pina(validate(owner = ID, not_empty))]
	pub source: &'a mut AccountView,
}
```

The generated implementation uses the same validation primitives as the direct form, so the annotations remain optional syntax sugar. Codama continues to generate clients from either style.

## Examples

The new `examples/validation_program` project exercises numeric and length rules, custom errors, and cross-field hooks on instructions, account state, events, and `#[derive(Accounts)]` account lists. It also shows an explicit rule that combines decoded instruction data with loaded state. The existing `events_program` now provides a smaller retrofit example with numeric, exact-length, and custom event checks.

The reusable before-and-after guide and full example summary are maintained in `templates/validation.t.md` with MDT and consumed by the validation guide and the new example readme.
