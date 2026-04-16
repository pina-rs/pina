# Pina Coding Style Guide

This guide defines the visual organization and aesthetic patterns for the Pina codebase. It complements the automated formatting workflow (`fix:format`, `dprint fmt`) and focuses on readability, maintainability, and consistency.

## Philosophy

**Simple code is better than complex code.**

Code is read far more often than it is written. Optimize for the reader.

- Choose solutions with fewer lines and less nesting
- Use early returns to avoid deep indentation
- Group related operations and separate them with blank lines
- Explain _why_, not _what_ (the code itself should be clear)

## Language Standards

### Rust

- **Edition**: 2024
- **Formatter**: `fix:format` or `dprint fmt` (do not run `rustfmt` directly)
- **Style Edition**: 2024
- **Tabs**: Hard tabs (indent width: 2 for dprint, configured in rustfmt)
- **Max Width**: 100 characters

### TypeScript/JavaScript

- **Formatter**: dprint with TypeScript plugin
- **Tabs**: Hard tabs
- **Quote Style**: Double quotes
- **Semicolons**: Always

## Core Principles

### 1. Whitespace Is Semantics

Blank lines separate concepts and give the reader time to breathe.

**Rules:**

- Add blank lines before control flow statements (`if`, `match`, `for`, `while`)
- Group related operations, separate groups with blank lines
- Add blank lines after complex declarations
- Add blank lines before return statements (unless it's the immediate next line)

```rust
// Good: Logical grouping with breathing room
fn process_instruction(accounts: &[AccountView], data: &[u8]) -> ProgramResult {
	// Group 1: Parse and validate instruction discriminator
	let disc = parse_instruction(&ID, program_id, data)?;

	// Group 2: Route to appropriate handler
	match disc {
		Instruction::Initialize => initialize(accounts, data),
		Instruction::Update => update(accounts, data),
	}
}
```

### 2. Early Returns Over Deep Nesting

Indentation is a code smell. Prefer guard clauses and early returns.

**Maximum nesting depth**: 2-3 levels. If you exceed this, refactor.

```rust
// Avoid: Deep nesting
fn validate(account: &AccountView) -> Result<&AccountView, ProgramError> {
	if account.is_signer() {
		if account.is_writable() {
			if account.data_len() > 0 {
				Ok(account)
			} else {
				Err(ProgramError::InvalidAccountData)
			}
		} else {
			Err(ProgramError::InvalidAccountData)
		}
	} else {
		Err(ProgramError::MissingRequiredSignature)
	}
}

// Prefer: Early returns with guard clauses
fn validate(account: &AccountView) -> Result<&AccountView, ProgramError> {
	if !account.is_signer() {
		return Err(ProgramError::MissingRequiredSignature);
	}

	if !account.is_writable() {
		return Err(ProgramError::InvalidAccountData);
	}

	if account.data_len() == 0 {
		return Err(ProgramError::InvalidAccountData);
	}

	Ok(account)
}
```

### 3. Variables at the Top

Declare variables and constants at the start of functions when possible.

```rust
fn configure_server(config: &Config) -> Server {
	// Configuration extraction
	let port = config.port;
	let timeout = config.timeout_secs;
	let max_connections = config.max_connections;

	// Security settings
	let require_tls = config.environment == Environment::Production;

	// Build and return
	Server::builder()
		.port(port)
		.timeout(timeout)
		.max_connections(max_connections)
		.tls(require_tls)
		.build()
}
```

**Exception**: When a variable's value depends on prior computation, declare it near where it's computed.

### 4. Extraction Over Nesting

When logic becomes complex, extract it into smaller, focused functions.

```rust
// Avoid: Complex nested logic
fn process_data(data: Data) -> Result {
	if let Some(items) = data.items {
		for item in items {
			if item.is_active {
				// 20 lines of complex processing...
			}
		}
	}
}

// Prefer: Extract into focused functions
fn process_data(data: Data) -> Result {
	let active_items = data.active_items()?;

	for item in active_items {
		process_active_item(item)?;
	}

	Ok(())
}

fn process_active_item(item: &Item) -> Result {
	// 20 lines of focused processing...
}
```

### 5. Comments Explain Why, Not What

Comments should explain **why** code exists, not **what** it does.

```rust
// Avoid: Commenting the obvious
// Add 1 to the counter
let counter = counter + 1;

// Prefer: Explain why
// Increment counter to track unique visitors (resets daily)
let counter = counter + 1;
```

**Exception**: When security or performance requires non-obvious code, explain both:

```rust
// Security: Constant-time comparison to prevent timing attacks
// We compare every byte regardless of mismatches to ensure
// the operation takes the same time regardless of where the
// first difference occurs
if !constant_time_eq(provided_hash, stored_hash) {
	return Err(Error::InvalidCredentials);
}
```

### 6. Safety Comments for Unsafe Code

All `unsafe` blocks require a `// SAFETY:` comment explaining why the operation is safe.

```rust
// SAFETY: `try_borrow` yields a guard-backed slice of the account data.
// We rebuild a raw-parts slice from the same pointer with exactly
// `size_of::<T>()` bytes, which is guaranteed by `assert_data_len` above.
unsafe {
	T::try_from_bytes(from_raw_parts(self.try_borrow()?.as_ptr(), size_of::<T>()))
}
```

## Rust-Specific Patterns

### Import Organization

Imports are grouped and sorted by `rustfmt`:

```rust
// 1. Standard library imports
use std::collections::HashMap;
use std::path::Path;

// 2. External crate imports
use bytemuck::Pod;
use pinocchio::ProgramResult;

// 3. Internal crate imports
use crate::AccountView;
use crate::Address;
use crate::ProgramError;
```

### Error Handling

Always use `?` for propagation. Avoid `unwrap()` and `expect()` in production code.

```rust
// Good
let data = account.try_borrow()?;
let parsed = parse_data(&data)?;

// Avoid
let data = account.try_borrow().unwrap();
```

### Type Conversions

Use explicit conversion functions and avoid implicit casts.

```rust
// Good
let value = PodU64::from_primitive(100u64);
let native: u64 = value.into();

// Avoid
let value = PodU64::from(100); // Ambiguous
```

### Trait Implementations

Group trait implementations together. Use blank lines between different trait implementations.

```rust
impl AccountInfoValidation for AccountView {
	// methods...
}

impl AsAccount for AccountView {
	// methods...
}
```

### Constants and Statics

Use `SCREAMING_SNAKE_CASE` for constants. Define them near their usage when module-scoped, or at the top of the file when crate-scoped.

```rust
/// Maximum number of bytes for a discriminator.
pub const MAX_DISCRIMINATOR_SPACE: usize = 8;

/// Known program addresses for default value resolution.
const KNOWN_ADDRESSES: &[(&str, &str)] = &[
	("system::ID", "11111111111111111111111111111111"),
	("token::ID", "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"),
];
```

### Macro Definitions

Document macros with usage examples. Keep macro logic as simple as possible.

````rust
/// Sets up a `no_std` Solana program entrypoint.
///
/// # Usage
///
/// ```ignore
/// nostd_entrypoint!(process_instruction);
/// ```
#[macro_export]
macro_rules! nostd_entrypoint {
	($process_instruction:expr) => {
		// ...
	};
}
````

## Documentation Standards

### Module Documentation

Every module should start with a module-level doc comment explaining its purpose.

```rust
//! CPI and account-allocation helpers used by on-chain instruction handlers.
//!
//! These utilities wrap common system-program patterns with consistent
//! `ProgramError` behavior and PDA signing. All APIs are designed for
//! on-chain determinism.
```

### Function Documentation

Use doc comments (`///`) for all public APIs:

````rust
/// Creates a new PDA-backed program account and returns `(address, bump)`.
///
/// This helper derives the canonical PDA for `seeds` + `owner`, allocates
/// account storage for `T`, and assigns account ownership to `owner`.
///
/// # Errors
///
/// Returns `InvalidSeeds` when no valid PDA can be derived, plus any errors
/// from allocation/assignment steps.
///
/// # Examples
///
/// ```ignore
/// let seeds: &[&[u8]] = &[b"escrow", authority.address().as_ref()];
/// let (address, bump) =
///     create_program_account::<EscrowState>(escrow_account, payer, &program_id, seeds)?;
/// ```
pub fn create_program_account<'a, T: HasDiscriminator + Pod>(// ...)
 -> Result<(Address, u8), ProgramError> {
	// ...
}
````

### Documentation Sections

Use standardized documentation patterns for common sections:

- `# Errors` - Document possible error conditions
- `# Examples` - Include usage examples (use `ignore` for code that won't compile in docs)
- `# Safety` - Required for unsafe functions
- `# Panics` - Document when the function might panic

## Testing Patterns

### Test Organization

Place tests in a `#[cfg(test)]` module at the end of the file.

```rust
#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn parses_valid_instruction() {
		// ...
	}

	#[test]
	fn rejects_invalid_discriminator() {
		// ...
	}
}
```

### Test Naming

Use descriptive snake_case names that explain what is being tested.

```rust
// Good
#[test]
fn account_deserialize_rejects_wrong_discriminator() {}

// Avoid
#[test]
fn test_account() {}
```

## Formatter Integration

This style guide complements automated formatters. Always run the formatter after editing:

```bash
# Format all files
dprint fmt

# Or for specific files
dprint fmt <file-path>
```

### Pre-commit Checklist

1. **Format**: Run `dprint fmt` on all modified files
2. **Lint**: Run `lint:clippy` for clippy-only verification or `lint:all` for the full lint suite, then address remaining warnings
3. **Test**: Run `cargo test` to ensure changes don't break tests

## Language-Specific Formatting

### Rust Formatting

The project uses `rustfmt.toml` settings through the repository formatting workflow:

- `hard_tabs = true` - Use tabs for indentation
- `max_width = 100` - Wrap lines at 100 characters
- `imports_granularity = "Item"` - One import per line
- `group_imports = "StdExternalCrate"` - Group imports by category
- `format_code_in_doc_comments = true` - Format code in docs
- `wrap_comments = false` - Don't wrap comments

### TypeScript/JavaScript Formatting

The project uses dprint with these settings:

- `useTabs: true` - Use tabs for indentation
- `quoteStyle: "alwaysDouble"` - Use double quotes
- `semiColons: "always"` - Always use semicolons

## No-Std Considerations

All on-chain code must be `no_std` compatible:

```rust
#![no_std]

// Use core instead of std
use core::mem::size_of;

// Use alloc for collections when needed
extern crate alloc;
use alloc::vec::Vec;
```

## Summary

| Principle      | Rule                                            | Exception                                 |
| -------------- | ----------------------------------------------- | ----------------------------------------- |
| **Simplicity** | Choose simpler solutions                        | Security/performance requires complexity  |
| **Whitespace** | Blank lines before control flow, between groups | Short, tightly-coupled operations         |
| **Nesting**    | Max 2-3 levels deep                             | Language idioms require it                |
| **Comments**   | Explain why, not what                           | Security/performance requires explanation |
| **Extraction** | Break complex logic into functions              | Hurts performance                         |
| **Formatting** | Run `dprint fmt` after every edit               | N/A - Always run it                       |
| **Linting**    | Run `cargo clippy`, fix all warnings            | Only skip if very slow                    |

**Remember**: Code is read far more often than it is written. Optimize for the reader.
