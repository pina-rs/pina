# Runtime-core sweep — `crates/pina/src/**` (2026-09-19)

Sweep window: 2026-09-19, ~22:20–23:20. Scope: `crates/pina/src/**` at worktree `security-sweep-2026-09-19`. Read-only audit; the only files written were scratch probes under `tmp/sweep/scratch-runtime/` (untracked).

Method: full reads of `traits.rs`, `impls.rs`, `token.rs`, `pda.rs`, `utils.rs`, `lib.rs`, `event.rs`, `introspection.rs`, `transaction.rs`, `error.rs`, `pod/mod.rs`; targeted reads of `cpi.rs` (creation/allocate/realloc/close/CPI paths; remainder is tests) and of `migration.rs` limited to the discriminator envelope surface (migration detail owned by the two migration agents). Cross checks against pinned upstream sources: `pinocchio 0.11.2`, `solana-account-view 2.0.0`, `pinocchio-token 0.7.0`, `pinocchio-token-2022 0.4.0`, `pinocchio-associated-token-account 0.4.0`. Branch delta `main...HEAD` touching `crates/pina` is a readme-only change (15 added lines), so no behavioral diff review was needed.

Excluded as known (prior audit `security/deep-audit-2026-09-18.md` findings H1/H2/M1–M4/L1–L11 and its "verified sound" list) and as sibling-agent findings (`sweep-2026-09-19/macro-codegen.md` mid-list optional shift + `next_opt`/`next_mut_opt` sentinel mechanics, `cpi.rs` `CreateProgramAccountWithUncheckedBump` shadow-PDA path, `migration.rs:814-820` grown-region zeroing): not re-reported below except where this sweep adds a new mechanism or a new affected path.

---

## Verdict

One new Medium finding: the AccountsCursor duplicate-writable-alias guard has a binding-order blind spot that lets a classic duplicate-mutable-account exploit shape parse successfully (confirmed with a host probe). Everything else probed held: close/revive ordering, realloc rent/resize ordering and the cumulative growth residual, guard-backed loader aliasing, token loader program-selection and extension policy surface, discriminator width matching, arithmetic (checked/saturating; no value-bearing bare casts), and CPI flag forwarding.

---

## New findings

### RC-1 — Duplicate-writable-alias guard misses aliases whose first binding is immutable (or rides in a `remaining` field) — CONFIRMED with host probe

**(1) Location.** `crates/pina/src/traits.rs:1212-1227` (`AccountsCursor::track_mutable_account`), applied from `next_mut` (`traits.rs:1087-1097`) and `next_mut_opt` (`traits.rs:1124-1139`); `remaining_mut_distinct` (`traits.rs:1181-1194`) has the same blind spot for `#[pina(remaining)]` mutable fields. Instruction shape involved: any `#[derive(Accounts)]` struct where a `&AccountView` (or `Option<&AccountView>`) field precedes a `&mut AccountView` / `Option<&mut AccountView>` / mutable `#[pina(remaining)]` field.

**(2) Mechanism.** The anti-aliasing check runs at the moment a _mutable_ slot is parsed and scans only `self.remaining` — the slots **not yet consumed** — and only flags futures that are themselves **writable**. Two consequences:

- An alias already **consumed** by an earlier `next()`/`next_opt()` binding is invisible to the check. When the earlier slot was marked **writable** in the instruction (nothing forbids passing a writable slot to an immutable field — `next()` never calls `validate_writable`), the pair "writable slot bound immutably, then the same address bound mutably" parses without any error, even though both instruction entries are writable.
- `remaining_mut_distinct` checks distinctness **within** the trailing slice only; it never sees slots consumed earlier, so `[x: &AccountView,
  #[pina(remaining)] rest: &mut [AccountView]]` accepts `rest[0] == x` when both entries are writable.

This is exactly the sealevel-attacks class 06 shape the framework claims to close ("rejects writable aliases for mutable accounts parsed individually", `traits.rs:1029-1034`; prior audit's class table: "06 Duplicate mutable accounts — Covered (pairwise rejection … runtime-tested)"). The coverage claim is only true for mutable-binding-first orderings.

**(3) Exploit scenario.** Program schema `#[derive(Accounts)] struct SetState<'a> { pub config: &'a AccountView, pub
vault: &'a mut AccountView }` with handler logic `config.assert_address(&CONFIG_PDA)?;` followed by writes through `vault` (e.g. setting an attacker-supplied `owner`/`limit` field), where the program never re-derives `vault`'s address because it trusts the framework's duplicate-mutable rejection to make `config` and `vault` distinct.

1. Attacker builds the instruction with account list `[CONFIG_PDA, CONFIG_PDA]` and marks **both** entries writable.
2. `next()` binds slot 0 (`config`) with no alias tracking; `next_mut` binds slot 1 (`vault`) and `track_mutable_account` finds nothing in the (now empty) remaining slice → parsing succeeds.
3. The handler's `config.assert_address(CONFIG_PDA)` passes — it _is_ the config account — and the handler then writes through `vault`, which aliases the same memory. The attacker gets protocol-approved writes applied to the config account, or (mirror shape) reads a safety property off `config` that the `vault` write invalidates mid-instruction.

Probe: `tmp/sweep/scratch-runtime/probe/tests/alias_probe.rs` (harness copied from `crates/pina/tests/accounts_derive.rs`). Results, both passing:

- control `MutThenMut` (`&mut` + `&mut`, same writable alias) → `PinaProgramError::DuplicateMutableAccount` (documented behavior intact);
- probe `ImmutableThenMut` (`&` + `&mut`, same address, both slots writable) → **parses Ok**, and the test asserts both slots report `is_writable() == true`.

The existing suite pins only `optional-mut-then-mut` (rejected) and `mut-then-readonly` (accepted); the accepted-because-blind `writable-immutable-then-mut` ordering has no test and no guard.

**(4) Severity justification.** Medium. It is a real privilege-confusion class (the framework's advertised line of defense does not fire), the attack is one instruction with no special conditions, and pina programs are nudged into the vulnerable shape by the framework's own ergonomics (validate the immutable config, mutate the mutable vault). It stays below High because exploitation requires a handler that (a) orders an immutable field before a mutable one and (b) skips per-field identity validation on the mutable account on the strength of the framework's alias rejection — a program that validates each account's address is unaffected. `remaining`-field variant widens exposure for list-style handlers.

**(5) Fix.** Make the cursor able to see consumed slots: retain the original slice bounds (e.g. store the base pointer plus `consumed: usize` alongside `remaining`) and change `track_mutable_account` to reject when the current **mutable** binding aliases _any_ other slot — past or future — that is marked writable. Symmetrically, reject in `next()`/`next_opt()` when the current slot is writable and aliases a slot that will be (or has been) bound mutably; since the cursor cannot know future binding types, the robust variant is a macro-generated full-slice pre-pass in `#[derive(Accounts)]`: after parsing, assert that no two slots bound with mutable access share an address with any other writable-marked slot (the macro knows every field's binding kind, which the cursor never will). Keep the documented, intentional escape hatches explicit: `distinct = false` for `remaining_mut`, and readonly-duplicate pairs (mutable-then-readonly) remain legal.

**Suggested `pina_lints` rules.**

- `require_account_identity_validation` (warn): in a `#[derive(Accounts)]` struct, a `&mut AccountView` field whose preceding sibling fields include any immutable field, and whose handler (same module) contains no `assert_address`/`assert_seeds`/`assert_associated_token_address` call on that field — the lexical shape that makes RC-1 exploitable.
- `pairwise_writable_alias_prepass` (error, macro-level): generate the full distinctness pre-pass described above instead of relying on cursor-time detection; the lint is then a UI test pinning `writable-immutable-then-mut` → `DuplicateMutableAccount`.

---

### RC-2 — Fixed-account PDA creation skips the writability assertions its compact siblings perform — CONFIRMED (fail-closed)

**(1) Location.** `crates/pina/src/cpi.rs:638-675` (`PdaCreationTarget::allocate_and_initialize`, used by `CreateProgramAccount`, `CreateProgramAccountWithBump`, and `CreateProgramAccountWithUncheckedBump`) versus `cpi.rs:785-786` and `cpi.rs:866-867` (`CreateCompactProgramAccount*::invoke_signed_inner*`, which call `self.account.assert_writable()?` and `self.payer.assert_writable()?`).

**(2) Mechanism.** The fixed-account creation path never asserts `account.is_writable()` or `payer.is_writable()` before issuing the system CreateAccount/Transfer+Allocate+Assign CPIs. The compact paths do. The system program and the CPI privilege-inheritance rules still reject a non-writable target or an unsigned funder, so this is not a bypass — but the failure surfaces as a raw runtime/CPI privilege error instead of the framework's `InvalidAccountData`, and the two creation families reject at different layers for identical misuse.

**(3) Exploit scenario.** None for privilege escalation. Attack value is limited to error-oracle asymmetry: a fuzzer or client probing a program can distinguish "account not writable" (CPI-level failure code) on the fixed path from the typed `ProgramError::InvalidAccountData` the compact path returns, which mildly undermines the error-distinguishability work order (WO1) and can confuse handlers that pattern-match creation failures.

**(4) Severity justification.** Low. Fail-closed at the system program; no lamport, data, or authorization effect; consistency and diagnostics only.

**(5) Fix.** Add `self.account.assert_writable()?;` and `self.payer.assert_writable()?;` (borrow-cheap flag reads) at the top of `PdaCreationTarget::allocate_and_initialize`, matching the compact builders; add a UI/test pin that a read-only target yields `InvalidAccountData` on both families.

---

## Sentinels, parsing paths, and the "own-PDA confusion" question (asked explicitly)

- **Can a program's own program-id-owned PDA be confused with the program-address `None` sentinel?** No. The sentinel compares against the _executing program's own address_ (`traits.rs:1112`, `traits.rs:1131`). PDA derivation only yields off-curve addresses; a deployed program's address is a normal account address, so `try_find_program_address` can never return it and a program-owned PDA can never equal the sentinel. The sentinel can only fire when the program's _own account_ is passed, which is the intended filler semantics. The exploitable sentinel weakness remains the sibling-reported mid-list optional shift (`next_opt`/`next_mut_opt`); this sweep found no additional sentinel path beyond RC-1's alias blind spot.
- Non-optional slots bind the program's own address happily (`next()` does no sentinel check). A program account is executable, read-only, loader-owned, so downstream `assert_signer`/`assert_owner` checks reject it; `assert_program` _would_ accept it for the executing program itself, which is correct semantics, not a bypass.
- Other cursor paths re-checked: `take_remaining` (immutable, no tracking needed), `remaining_mut` (documented `distinct = false` escape hatch), `finish_exact`, and `peek` — no additional binding-shift or alias behavior beyond the two known items.

## Blocked attacks and checks that held (logged with the stopping check)

- **Close/revive (lens 2).** `close_with_recipient` / `close_account_zeroed` (`impls.rs:920-950`): recipient==self rejected (`impls.rs:837-841`); both parties `assert_writable`; owner check before any mutation; `check_borrow_mut` before arithmetic; zeroing borrows and drops before lamport writes; `close()` re-checks borrows before zeroing owner/lamports/data_len (verified in `solana-account-view 2.0.0`, `lib.rs:315-395`). Same-transaction revival with stale bytes remains the known documented residual (prior audit L4); with `close_account_zeroed` the revival target is all-zero, which the creation paths' non-zero check (`cpi.rs:648-651`, `cpi.rs:882-885`) treats as init-able, not as an authority carryover.
- **Realloc (lens 3).** Single-step growth cap enforced pre-rent-movement (`cpi.rs:1815-1816`); rent plan is pure (`ReallocPlan`), refund path re-runs the owner-checked direct transfer (`cpi.rs:1841-1843` over `impls.rs:886-918`); no borrow crosses the resize (`check_borrow_mut` at `cpi.rs:1815`, scoped borrows at `cpi.rs:1647-1657`, `1736-1739`); shrink path validates the truncated prefix is a fully valid compact account before shrinking (`cpi.rs:1736-1739`), so active payloads cannot be truncated; cumulative-growth overflow across multiple resizes is caught by the runtime at `resize` and honestly documented (`cpi.rs:1441`, `1531`). Shrink-then-grow in one instruction cannot exceed the runtime residual (runtime measures against the original length).
- **Borrow aliasing (lens 4).** Every typed path funnels through `try_borrow`/`try_borrow_mut` (pinocchio `Ref::try_map` chain) or the token loaders' checked borrows; combining a token loader and `as_account*` on the same address conflicts at the runtime borrow state; two `&mut` bindings of one slot are impossible (Rust exclusivity on the slice element); the only aliasing gap found is RC-1 (instruction-level, not borrow-level).
- **Token loaders (lens 5).** `TokenMintRef`/`TokenAccountRef::from_account_view` accept only the two canonical program IDs before delegating (`token.rs:66-82`, `212-228`), and upstream loaders verify owner+length+borrow (verified in `pinocchio-token 0.7.0` `state/account.rs:57-71` and `pinocchio-token-2022 0.4.0` `state/extension/state.rs:249-260`). ATA derivation seeds `[wallet, token_program, mint]` under the ATA program (`utils.rs:212-225`) match the SPL spec; `as_associated_token_account` re-checks stored mint/owner against the derivation inputs (`impls.rs:784-801`). Extension policy is opt-in (`assert_no_extensions` / `assert_extensions_allowed`) on both mint and account enums; the gap "account-level policy does not cover mint-level transfer-fee/transfer-hook" is explicitly documented in the API docs (`token.rs:238-250`, `traits.rs:929-931`) — documented residual, not a new finding; a `require_mint_extension_policy` lint would harden it.
- **Discriminators (lens 6).** Width-exact matching everywhere (`traits.rs:485-528`); short input → `false`/`InvalidInstructionData`, never a prefix alias; the migration `0xFF` route is width-exact with negative tests (`migration.rs:1330-1356`); cross-namespace (instruction vs account vs event) collision has no runtime effect because the data flows never cross. The u16+-reserved-value rejection remains prior-audit L6 (fail-closed).
- **Arithmetic (lens 7).** No value-bearing bare `as` in `crates/pina/src` outside the test fixtures (`cpi.rs:2307/2311`, `migration.rs:1659/1663` are test/proof harnesses; widening casts only elsewhere, `error.rs:123`, `cpi.rs:656/890`, `introspection.rs:64/115/193/247`). `introspection.rs:115`'s `num_instructions() as u16` cannot truncate for any packet the runtime accepts (≤1232 bytes bounds the count far below 65535). Lamport math is checked on both debit and credit (`impls.rs:804-826`); realloc rent math is checked-then-saturating on validated inputs.
- **CPI helpers (lens 8).** `CpiHandle::writable*` refuses non-writable sources before forwarding the writable bit (`cpi.rs:2005-2033`); readonly handles can only downgrade; `Program::try_new` asserts address+executable before the address is used as the CPI program id (`cpi.rs:2103-2131`, ordering address→executable at `impls.rs:146-149`). Signer bits in handles are honored-or-failed by the runtime; no seedless-signer escalation found. `AllocateAccountWithNonCanonicalBump` caps extra signers at 15 plus the derived target signer (`cpi.rs:1212-1213`, `1240-1256`) and derives-then-compares the exact address before any CPI (`cpi.rs:1220-1229`); the prefunded Transfer+Allocate+Assign route is system-program-gated at each step.
- **Introspection.** `Instructions::try_from` validates the sysvar address upstream (`pinocchio 0.11.2` `sysvars/instructions.rs:122-124`), so `introspection.rs` helpers cannot be fed a look-alike account; the no-CPI-detection caveat is deprecated-and-documented.

## Coverage notes for the fix work

- `traits.rs:1212-1227` (the RC-1 guard) has no test exercising an already-consumed alias; the probe in `tmp/sweep/scratch-runtime/probe/tests/alias_probe.rs` is a ready-made regression pair (convert both tests into `accounts_derive.rs` when fixing — `control_mut_then_mut_is_rejected` already matches the existing `DuplicateMutableAccount` assertions).
- RC-2's fix adds lines to `PdaCreationTarget::allocate_and_initialize`; patch coverage will need a read-only-target test on the fixed creation family.

## Scratch artifacts (untracked)

- `tmp/sweep/scratch-runtime/probe/` — cargo probe crate (source above); `cargo test --test alias_probe` executed under `devenv shell`, 2/2 passing.
