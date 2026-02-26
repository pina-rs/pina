# Custom Dylint Lints

<br>

Pina ships three custom [dylint](https://github.com/trailofbits/dylint) lints that catch common Solana smart contract vulnerabilities at compile time. Each lint maps directly to a vulnerability in the [Pina Security Guide](../security/readme.md).

## Setup

<br>

The lints are registered in the workspace `Cargo.toml`:

```toml
[workspace.metadata.dylint]
libraries = [
	{ path = "lints/require_owner_before_token_cast" },
	{ path = "lints/require_empty_before_init" },
	{ path = "lints/require_program_check_before_cpi" },
]
```

Run all custom lints with:

```sh
cargo dylint --all -- --all-targets
```

## Lint Reference

<br>

### `require_owner_before_token_cast`

<br>

|                       |                                                           |
| --------------------- | --------------------------------------------------------- |
| **Level**             | `warn`                                                    |
| **Security category** | [#02 Owner Checks](../security/02-owner-checks/readme.md) |

#### What it does

Warns when `as_token_mint()`, `as_token_account()`, `as_token_2022_mint()`, or `as_token_2022_account()` is called without a preceding `assert_owner()` or `assert_owners()` call on the same account within the same function.

#### Why is this bad?

These methods perform raw layout casts without ownership verification. An attacker can create a fake account with arbitrary token data owned by a different program. Without an owner check, the program trusts spoofed data, which can lead to inflated balances, bypassed invariants, or drained funds.

#### Example

Bad — casts to token layout without verifying account ownership:

```rust
fn process(&mut self, _data: &[u8]) -> ProgramResult {
	let token = self.vault.as_token_account()?;
	// attacker can pass any account here
	Ok(())
}
```

Good — verifies ownership before the cast:

```rust
fn process(&mut self, _data: &[u8]) -> ProgramResult {
	self.vault.assert_owners(&[&token::ID, &token_2022::ID])?;
	let token = self.vault.as_token_account()?;
	Ok(())
}
```

#### Detected methods

| Cast method               | Description                       |
| ------------------------- | --------------------------------- |
| `as_token_mint()`         | Cast to SPL Token Mint layout     |
| `as_token_account()`      | Cast to SPL Token Account layout  |
| `as_token_2022_mint()`    | Cast to Token-2022 Mint layout    |
| `as_token_2022_account()` | Cast to Token-2022 Account layout |

#### Accepted checks

- `assert_owner(&program_id)` — verifies account owned by a specific program
- `assert_owners(&[&id_a, &id_b])` — verifies account owned by one of several programs

---

### `require_empty_before_init`

<br>

|                       |                                                               |
| --------------------- | ------------------------------------------------------------- |
| **Level**             | `warn`                                                        |
| **Security category** | [#04 Initialization](../security/04-initialization/readme.md) |

#### What it does

Warns when `create_program_account()` or `create_program_account_with_bump()` is called without a preceding `assert_empty()` call on the target account within the same function.

#### Why is this bad?

Without an emptiness check, an attacker can reinitialize an already-initialized account, overwriting existing state. This can reset authorities, zero balances, or corrupt program state. The pina `#[account]` macro does **not** inject automatic reinitialization protection.

#### Example

Bad — creates an account without checking if it already exists:

```rust
fn process(&mut self, _data: &[u8]) -> ProgramResult {
	create_program_account::<CounterState>(
		self.counter,
		self.payer,
		&ID,
		&counter_seeds!(self.authority.key()),
	)?;
	Ok(())
}
```

Good — checks the account is empty before creating:

```rust
fn process(&mut self, _data: &[u8]) -> ProgramResult {
	self.counter.assert_empty()?;
	create_program_account::<CounterState>(
		self.counter,
		self.payer,
		&ID,
		&counter_seeds!(self.authority.key()),
	)?;
	Ok(())
}
```

#### Detected functions

| Function                             | Description                                           |
| ------------------------------------ | ----------------------------------------------------- |
| `create_program_account()`           | Create a PDA-owned account                            |
| `create_program_account_with_bump()` | Create a PDA-owned account with an explicit bump seed |

#### Accepted check

- `assert_empty()` on the same target account passed as the first argument to the creation function

---

### `require_program_check_before_cpi`

<br>

|                       |                                                             |
| --------------------- | ----------------------------------------------------------- |
| **Level**             | `warn`                                                      |
| **Security category** | [#05 Arbitrary CPI](../security/05-arbitrary-cpi/readme.md) |

#### What it does

Warns when `.invoke()` or `.invoke_signed()` is called without a preceding `assert_address()`, `assert_addresses()`, or `assert_program()` call on a program account within the same function.

#### Why is this bad?

Without verifying the target program's address, an attacker can substitute a malicious program that executes arbitrary logic with the authority and accounts passed to the CPI. This can steal funds (by replacing the system program), mint unlimited tokens (by replacing the token program), or execute any other operation.

#### Example

Bad — invokes a CPI without verifying the program address:

```rust
fn process(&mut self, _data: &[u8]) -> ProgramResult {
	system::instructions::Transfer {
		from: self.payer,
		to: self.recipient,
		lamports: 1_000_000,
	}
	.invoke()?;
	Ok(())
}
```

Good — verifies the program address before the CPI:

```rust
fn process(&mut self, _data: &[u8]) -> ProgramResult {
	self.system_program.assert_program(&system::ID)?;
	system::instructions::Transfer {
		from: self.payer,
		to: self.recipient,
		lamports: 1_000_000,
	}
	.invoke()?;
	Ok(())
}
```

#### Detected methods

| CPI method         | Description                                              |
| ------------------ | -------------------------------------------------------- |
| `.invoke()`        | Invoke a cross-program instruction                       |
| `.invoke_signed()` | Invoke a cross-program instruction with PDA signer seeds |

#### Accepted checks

- `assert_address(&expected_id)` — verifies exact address match
- `assert_addresses(&[&id_a, &id_b])` — verifies address is one of a set
- `assert_program(&expected_id)` — verifies address **and** executable flag (most comprehensive)

The lint uses a heuristic to identify program accounts: the receiver's identifier must contain `program`, `system`, or `token`.
