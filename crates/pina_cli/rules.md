# Rules for Reliable IDL Generation

These rules describe the source shapes Pina's IDL extractor recognizes most reliably. If you stay inside them, `pina idl` and `pina codama generate` should produce stable, complete output.

## 1. Parser entrypoint and module graph

- Keep `declare_id!` in the crate once and only once.
- Pina starts parsing from `src/lib.rs` and recursively follows `mod` declarations.
- The parser does **not** require a single-file layout, but it only sees files that are reachable from this module graph.
- Any module names are fine (for example `accounts`, `instructions`, `state`, or custom names) as long as they’re connected from `lib.rs`.
- Avoid placing IDL-relevant definitions behind `mod` paths the parser never visits.

## 2. Make instruction and account discriminators explicit

- Use `#[discriminator]` on enums that define instruction or account tags.
- Give every variant an explicit integer value.
- Keep the variant names stable.
- Use `#[instruction(discriminator = ..., variant = ...)]` on each instruction payload struct.
- Use `#[account(discriminator = ...)]` on account structs.

This keeps the on-chain layout predictable and gives the extractor a clear path from source to IDL.

## 3. Keep dispatch canonical

- Prefer a single `match` over the parsed instruction enum inside `process_instruction`.
- Each match arm should call the corresponding accounts type: `SomeAccounts::try_from(accounts)?.process(data)`.
- Avoid hiding routing behind trait objects, closures, or helper layers if you want the IDL to remain easy to inspect.

## 4. Put IDL-relevant validation in direct `self.field.assert_*()` chains

The extractor looks for validation on direct account-field chains. Examples:

```rust
self.authority.assert_signer()?;
self.sample.assert_writable()?;
self.system_program.assert_address(&system::ID)?;
self.escrow.assert_seeds_with_bump(seeds, &ID)?;
```

Recognized assertions currently include:

- `assert_signer`
- `assert_writable`
- `assert_seeds`
- `assert_seeds_with_bump`
- `assert_canonical_bump`
- `assert_address`

### Best practice

- Keep the assertion on the field itself.
- If you use helper functions, ensure the parser can still see the direct field chain.
- Avoid moving the important checks into opaque helper layers if you expect the IDL to infer signer/writable/PDA/default-value metadata.

## 5. Model PDA seeds with byte-string constants and `seeds` macros

For reliable PDA extraction:

- Use byte-string constants:

```rust
const COUNTER_SEED: &[u8] = b"counter";
```

- Use `macro_rules!` names that contain `seeds`. Good examples:
  - `counter_seeds!`
  - `seeds_counter!`
  - `escrow_seeds!`

- Keep the non-bump arm simple.
- Prefer direct constants and direct variable captures.
- Avoid constructing seeds dynamically in helper functions if the result should appear in the IDL.

### Good pattern

```rust
const ESCROW_SEED_PREFIX: &[u8] = b"escrow";

#[macro_export]
macro_rules! escrow_seeds {
	($maker:expr, $seed:expr) => {
		&[ESCROW_SEED_PREFIX, $maker, $seed]
	};
	($maker:expr, $seed:expr, $bump:expr) => {
		&[ESCROW_SEED_PREFIX, $maker, $seed, &[$bump]]
	};
}
```

## 6. Use known program ID paths for default-account inference

The extractor currently recognizes default account public keys when it sees the canonical path in `assert_address`. Use the standard imports and paths when possible:

- `system::ID`
- `token::ID`
- `token_2022::ID`
- `associated_token_account::ID`

If you use a different address source, the IDL may not infer a default value unless the parser is taught that new path.

## 7. Keep the source readable enough for humans and the parser

The parser is heuristic-based, not a full compiler. It is strongest when your source reads like the following:

- direct account assertions
- explicit discriminators
- named seed constants
- obvious dispatch
- ordinary Rust modules

It is weaker when you rely on:

- deep helper indirection
- runtime seed construction
- dynamic dispatch
- inferred discriminators
- clever but opaque validation paths

## 8. Treat the IDL contract as a tested API

Whenever you add or change a pattern that should be extracted:

- add or update a fixture under `crates/pina_cli/tests/fixtures/`
- add or update a snapshot under `crates/pina_cli/tests/snapshots/`
- run `test:idl`
- verify that regeneration is still deterministic

If the IDL should _not_ infer something, add a negative test so the behavior is explicit.

## 9. Recommended checklist for new programs

Before merging a new example or major parser change, check:

- `declare_id!` appears once.
- Instruction and account discriminators are explicit.
- Validation uses direct `self.field.assert_*()` chains.
- Seed constants are byte strings.
- Seed macros contain `seeds` in the name.
- `process_instruction` uses a canonical `match` dispatch.
- Multi-file structure still resolves from `src/lib.rs`.
- `test:idl` passes with no diff.

## Canonical source-shape examples

The following snippets show the source shapes the extractor handles most reliably.

<!-- {=pinaIdlCanonicalExamples} -->

### Multi-file layout

```rust
// src/lib.rs
use pina::*;

mod accounts;
mod instructions;
mod pda;
mod state;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");
```

### Canonical dispatch

```rust
#[cfg(feature = "bpf-entrypoint")]
pub mod entrypoint {
	use super::*;

	nostd_entrypoint!(process_instruction);

	pub fn process_instruction(
		program_id: &Address,
		accounts: &[AccountView],
		data: &[u8],
	) -> ProgramResult {
		let ix: MyInstruction = parse_instruction(program_id, &ID, data)?;

		// Add one arm per instruction variant.
		match ix {
			MyInstruction::Initialize => InitializeAccounts::try_from(accounts)?.process(data),
			MyInstruction::Update => UpdateAccounts::try_from(accounts)?.process(data),
		}
	}
}
```

### Validation chains

```rust
impl<'a> ProcessAccountInfos<'a> for InitializeAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let args = InitializeInstruction::try_from_bytes(data)?;
		let seeds = my_seeds!(self.authority.address().as_ref(), args.bump);

		self.authority.assert_signer()?;
		self.system_program.assert_address(&system::ID)?;
		self.token_program.assert_address(&token::ID)?;
		self.ata_program
			.assert_address(&associated_token_account::ID)?;
		self.state
			.assert_empty()?
			.assert_writable()?
			.assert_seeds_with_bump(seeds, &ID)?;

		Ok(())
	}
}
```

### PDA seed helpers

```rust
const MY_SEED: &[u8] = b"my";

#[macro_export]
macro_rules! my_seeds {
	($authority:expr) => {
		&[MY_SEED, $authority]
	};
	($authority:expr, $bump:expr) => {
		&[MY_SEED, $authority, &[$bump]]
	};
}
```

### Discriminators and account layouts

```rust
#[discriminator]
pub enum MyInstruction {
	Initialize = 0,
	Update = 1,
}

#[discriminator]
pub enum MyAccountType {
	MyState = 1,
}

#[instruction(discriminator = MyInstruction, variant = Initialize)]
pub struct InitializeInstruction {
	pub bump: u8,
}

#[instruction(discriminator = MyInstruction, variant = Update)]
pub struct UpdateInstruction {
	pub value: PodU64,
}

#[account(discriminator = MyAccountType)]
pub struct MyState {
	pub bump: u8,
	pub value: PodU64,
}
```

<!-- {/pinaIdlCanonicalExamples} -->

## Example roadmap: what to add next

The current examples are strong as feature probes, but several are still more "demo-sized" than "real-world-sized". I would add 2–3 larger examples that look like production programs:

### 1. Token vesting / lockup program

Why this is a good addition:

- exercises long-lived state
- has a real lifecycle: create, fund, cliff, claim, cancel, close
- uses PDAs, ATAs, token transfers, and authority checks
- gives the IDL extractor a realistic multi-instruction app

What it should cover:

- beneficiary and admin authorities
- vesting schedule/state account
- claimable amount calculations
- cliff + linear unlock logic
- cancel/revoke path
- event emission for claims and cancellations

### 2. Role-based registry / configuration program

Why this is a good addition:

- mirrors common on-chain admin/config patterns
- demonstrates role/permission checks cleanly
- can be split into modules without becoming obscure
- gives a realistic example of mutable config and gated operations

What it should cover:

- config PDA and per-item registry PDAs
- admin rotation
- add/update/remove entry flows
- freeze/unfreeze logic
- account sizing / optional realloc if the registry grows
- clear error types for permissions and duplicates

### 3. Staking / rewards distribution program

Why this is a good addition:

- shows a real business flow with accrual over time
- combines state mutation, token movement, and claim paths
- gives a more complex but still understandable lifecycle

What it should cover:

- deposit, withdraw, claim, and emergency withdraw
- per-user position PDAs
- reward index or epoch accounting
- fee recipient / treasury paths
- tested overflow and underflow handling

## Which current examples are too shallow, and how to deepen them

### `hello_solana`

Status: intentionally minimal.

- Keep it as the "hello world" example.
- Don't try to make it production-like; its value is simplicity.
- If you need deeper coverage, add a separate example rather than bloating this one.

### `counter_program`

Status: good beginner example, but still shallow.

How to deepen it:

- split it into modules (`state`, `instructions`, `accounts`, `entrypoint`)
- add an admin reset or close path
- add event emission on increments
- add a state migration or authority-rotation path
- keep overflow tests and PDA derivation tests

### `todo_program`

Status: still closer to a toy than a full app.

How to deepen it:

- model a real todo list with multiple item accounts or a collection account
- add create/update/complete/archive/delete flows
- add owner transfer or shared access roles
- add pagination or bounded growth if the account stores multiple items
- add close/refund behavior for archived items

### `transfer_sol`

Status: useful for transfer mechanics, but linear.

How to deepen it:

- add fee collection or treasury splitting
- support wrapped SOL and unwrap/refund paths
- include failure-mode tests for insufficient funds and rent edge cases
- add an authority or whitelist gate so it looks like a real payment flow

### `escrow_program`

Status: currently the strongest real-world example.

How to deepen it further:

- add cancel/expire logic
- add partial fills or fee splitting
- add a dispute or timeout path
- split the code into more modules to demonstrate maintainable scale

### `anchor_events`

Status: useful parity test, but the current shape is synthetic.

How to deepen it:

- emit events from a real state transition instead of only building event values
- connect event emission to a user-facing flow like create/update/close
- keep the serialization tests, but make the example tell a product story

### `anchor_realloc`

Status: narrow by design, but more technical than user-facing.

How to deepen it:

- turn it into a real growing account example, such as a proposal list, document store, or metadata registry
- keep the realloc guards, but embed them in a useful lifecycle
- add tests that show how growth affects serialization and validation

### `anchor_*` parity examples in general

Status: valuable, but their purpose is niche.

- They are excellent for regression and parity checks.
- They are not the best place to teach new users how to structure a Pina app.
- If you want more teaching value, pair them with one production-shaped example that uses the same primitives in a realistic flow.

## Bottom line

For reliable IDL generation, Pina should treat source structure as a contract. The extractor is already useful, but the rules need to be explicit, aligned with multi-file support, and backed by examples/tests that demonstrate the canonical shape.
