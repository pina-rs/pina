<!-- {@accountReallocationContract} -->

`target_size` is the exact physical allocation after the call. When the account grows, `rent_account` funds the missing rent before Pina resizes the data. When the account shrinks, `rent_account` receives the excess rent. New bytes are zero-initialized by the Solana runtime.

The Solana runtime limits account growth to `MAX_PERMITTED_DATA_INCREASE` bytes per top-level instruction. Pina rejects a single larger increase before it moves rent. Pinocchio does not expose the original serialized length, so cumulative growth from several reallocations in one instruction can still fail during `AccountView::resize`.

Propagate reallocation errors. If a later resize or callback fails after rent moves, Solana restores the account only when the instruction returns that error.

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

`ReallocAccountZeroed` has the same behavior and field names. Its name records the caller's intent that newly allocated bytes start at zero. The current Solana runtime zero-initializes new bytes for both builders.

<!-- {/accountReallocationLowLevelExample} -->

<!-- {@resizeCompactAccountExample} -->

Use `ResizeCompactAccount` for an exact compact update. The callback receives the account bytes under one mutable borrow. Load the generated mutable view inside the callback, stage every tail, and call `commit()` before returning:

```ignore
let target_size = Journal::projected_bytes(entries.len(), markers.len())?;

ResizeCompactAccount {
	account,
	rent_account,
	target_size,
	program_id,
}
.invoke::<Journal, _>(|data| {
	let mut journal = Journal::try_from_bytes_mut(data)?;
	journal
		.set_entries(entries)
		.map_err(|_| ProgramError::InvalidAccountData)?;
	journal
		.set_markers(markers)
		.map_err(|_| ProgramError::InvalidAccountData)?;
	journal.commit().map_err(|_| ProgramError::InvalidAccountData)?;

	Ok(())
})?;
```

The builder grows before the callback. After the callback, it verifies that the committed logical size equals `target_size`, drops the data borrow, and then shrinks and refunds rent when needed. Use `invoke_signed` with the same callback when `rent_account` is a PDA that funds growth.

<!-- {/resizeCompactAccountExample} -->

<!-- {@reallocCompactAccountExample} -->

`ReallocCompactAccount` is the lower-level compact builder. It changes the physical allocation immediately and does not edit or commit the compact view:

```ignore
ReallocCompactAccount {
	account,
	rent_account,
	target_size,
	program_id,
}
.invoke::<Journal>()?;
```

Call `invoke` before editing when the compact layout grows. To shrink, commit the shorter layout, drop its mutable data borrow, and then call `invoke`. Use `invoke_signed` instead when a PDA rent account must fund growth.

<!-- {/reallocCompactAccountExample} -->
