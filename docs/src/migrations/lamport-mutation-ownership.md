# Migrate lamport mutation calls

Pina now checks program ownership inside every public helper that directly debits lamports. This release changes Rust source APIs but does not change instruction or account wire formats.

## Replace `send` with `send_owned`

Remove the separate `assert_owner` call and pass the executing program ID to `send_owned`:

```rust
// Before
vault.assert_owner(&ID)?;
vault.send(amount, recipient)?;

// After
vault.send_owned(&ID, amount, recipient)?;
```

`send_owned` rejects a sender that `ID` does not own before it changes either balance. It also retains the existing writable, self-transfer, insufficient-funds, and overflow checks.

## Pass the program ID when closing an account

Pass the executing program ID to both close methods:

```rust
// Before
account.close_with_recipient(recipient)?;
account.close_account_zeroed(recipient)?;

// After
account.close_with_recipient(&ID, recipient)?;
account.close_account_zeroed(&ID, recipient)?;
```

Add `program_id` to the close builders:

```rust
CloseAccountZeroed {
	account,
	recipient,
	program_id: &ID,
}
.invoke()?;
```

Both builders validate ownership before changing account data or lamports. `CloseAccountZeroed` still clears the existing data bytes before closing. `CloseAccount` still leaves the old backing bytes unchanged.

## Remove the old lint configuration

Delete `require_program_owned_before_lamport_mutation` from `pina.toml` if the project lists it explicitly. Pina removed the lint because `send_owned` now performs the check itself. Other close and lamport safety lints remain available.

## Verify the migration

Run the program tests and Pina lints after updating every call:

```sh
cargo test
pina lint
```
