<!-- {@accountReallocationContract} -->

`UpdateResizableAccount` derives the target allocation from its patch. Lower-level reallocation builders take an explicit `target_size`. Every reallocation builder uses `rent_account` for the account that funds growth or receives a shrink refund. When a compact account grows, `rent_account` funds the missing rent before Pina applies the patch. When it shrinks, Pina applies the shorter representation before returning excess rent to `rent_account`. The Solana runtime zero-initializes new bytes.

The Solana runtime limits account growth to `MAX_PERMITTED_DATA_INCREASE` bytes per top-level instruction. Pina rejects a single larger increase before it moves rent. Pinocchio does not expose the original serialized length, so cumulative growth from several reallocations in one instruction can still fail during `AccountView::resize`.

Propagate reallocation errors. If a later resize or update fails after rent moves, Solana restores the account only when the instruction returns that error.

<!-- {/accountReallocationContract} -->

<!-- {@accountReallocationLowLevelExample} -->

Use `ReallocAccount` when the bytes do not use a compact Pina schema:

```ignore
ReallocAccount {
	account,
	rent_account,
	target_size,
	program_id,
}
.invoke()?;
```

Use `invoke_signed` when `rent_account` is a PDA that must sign the system transfer used for growth:

```ignore
ReallocAccount {
	account,
	rent_account,
	target_size,
	program_id,
}
.invoke_signed(rent_account_signers)?;
```

`ReallocAccountZeroed` has the same field names. Its name records the caller's intent that newly allocated bytes start at zero. The current Solana runtime zero-initializes new bytes for both builders.

<!-- {/accountReallocationLowLevelExample} -->

<!-- {@updateResizableAccountExample} -->

Use `UpdateResizableAccount` for a compact update. Its generated patch distinguishes unchanged fields from replacements:

```ignore
UpdateResizableAccount {
	account: self.journal,
	rent_account: self.authority,
	program_id: &ID,
	patch: JournalPatch::new()
		.revision(next_revision)
		.replace_entries(&entries)
		.note(Some("Updated")),
}
.invoke::<Journal>()?;
```

The builder validates the complete patch and calculates the target size before changing bytes or lamports. It grows before applying a longer representation and applies a shorter representation before shrinking. It skips the resize when the allocation does not change. Use `invoke_signed::<Journal>(signers)` when `rent_account` is a PDA that funds growth.

<!-- {/updateResizableAccountExample} -->

<!-- {@reallocCompactAccountExample} -->

`ReallocCompactAccount` is the lower-level compact builder. It changes the physical allocation and does not apply a patch:

```ignore
ReallocCompactAccount {
	account,
	rent_account,
	target_size,
	program_id,
}
.invoke::<Journal>()?;
```

For growth, call `invoke` before writing the longer representation. For shrinkage, write a valid shorter representation, drop its mutable data borrow, and then call `invoke`. Use `invoke_signed` when a PDA rent account funds growth. Prefer `UpdateResizableAccount` unless the caller needs explicit allocation control.

<!-- {/reallocCompactAccountExample} -->
