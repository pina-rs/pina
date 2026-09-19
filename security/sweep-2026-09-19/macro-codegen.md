# Macro codegen sweep — 2026-09-19

Scope: `crates/pina_macros/src/**` and the code it emits, on branch `feat/abi-version-reset` @ `3f7d38f9`. Follow-up to `security/deep-audit-2026-09-18.md` (M1, L1, L2, L5, L6) and `multisig-example-hard-edges.md` (items 3, 4, 10). Method: full read of `pda.rs`, `args.rs`, `accounts.rs`, `entrypoint.rs`, `discriminator.rs`, `instruction.rs`, `event.rs`, `account.rs`, `schema.rs`, `support.rs`, `error.rs`, `migration.rs`, plus behavior probes compiled against this branch's `pina`/`pina_macros` in `worktrees/security-sweep-2026-09-19/tmp/sweep/scratch-macros/` (retained).

## Remediation verification (prior findings)

All five audited macro items landed on this branch:

| Item                       | Evidence on branch                                                                                                                                                                                                        |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| M1 `with_pda` weakening    | `pda.rs:279-394` emits `with_stored_bump_pda` (single derivation), a `#[deprecated]` `with_pda` forwarding alias with a doc banner naming the semantics, and `with_checked_pda` (canonical search + stored-bump equality) |
| L1 16-seed PDA             | `args.rs:186` `MAX_SEEDS_BEFORE_BUMP = 15`, enforced at `args.rs:365-376`; UI pin `tests/ui/fail/pda_seed_count_over_limit.rs`; pass case at 15 in `args.rs` unit tests                                                   |
| L2 unqualified `Address`   | `pda.rs:164-168` identity proof `const _: fn(Address) -> #crate_path::Address`; every generated `Address` qualified (`args.rs:201-232`, `pda.rs:175-179`); UI pin `pda_shadowed_address.rs`                               |
| L3/L5 runtime counterparts | `try_write_discriminator` exists (`crates/pina/src/traits.rs:508`); `#[event]` emits the `MAX_EVENT_RECORD_BYTES` const assert (`event.rs:138-147`), UI pin `event_record_over_stack_budget.rs`                           |
| L6 migrate prelude width   | `entrypoint.rs:445-450` selects `is_migrate_instruction_u16/u32/u64`; `entrypoint.rs` tests pin the exact spelling per width                                                                                              |

The "Schema grammar closed by construction" claim holds on re-read: generics, manual `PinaPod` derives, `#[pinapod]` overrides, custom `ZcField` mappings, non-literal array lengths, `char`, `NonZero*`, and associated-const capacities are all rejected at expansion (`schema.rs:85-118, 364-401, 724-843, 1136-1153`), with UI pins for each; `SIZE == discriminator + Σ pods` is a const assert (`schema.rs:150-153`). Transition files are hash-pinned and TODO-scanned at expansion (`migration.rs:1419-1473`).

---

## New findings

### N1 (CONFIRMED, Low-Medium) — Interleaved PDA seeds are silently reordered at derivation, and the IDL publishes the declared order

**Location.** `crates/pina_macros/src/pda.rs:100-144` pushes constant seeds and variable seeds into two separate vectors; `pda.rs:443-445` emits `as_slices()` as `[#(#seed_constants,)* #(#seed_slice_exprs,)*]` — constants first regardless of declaration position. The IDL side, `crates/pina_cli/src/parse/pda_attr.rs:60-104`, walks `args.seeds` in declaration order, and its header comment (lines 11-13) states "the generated IDL always matches the on-chain derivation".

**Mechanism.** For `seeds = [b"v1", authority: Address, b"v2"]` the macro derives `["v1", "v2", authority]` while every artifact a client sees records `["v1", authority, "v2"]`.

**Evidence.** Behavior probe (scratch crate `src/probe.rs`, `cargo test -- --nocapture`):

```
generated      = Some(wMoFhtuiwpeN3mxXvgZ4jSRi27o9rWQ1jop4LFi76K9)
declared order = Some(89f8XF5yzJSHhaJ4PkSa9P19nDWBMrVJes9FqhEv4E1C)
consts-first   = Some(wMoFhtuiwpeN3mxXvgZ4jSRi27o9rWQ1jop4LFi76K9)
```

The generated derivation matches the constants-first reordering and **differs from the declared order** (assertion `interleaved_seed_order_is_constants_first` passes).

**Exploit scenario.** A client generated from the IDL derives the PDA per the declared order and computes address X; the program creates and loads address Y. Deposits sent to X are unrecognizable by the program (every loader verifies the derived address), so value strands or integration silently breaks. Auditors and indexers reading the IDL are taught the wrong derivation rule. Not a direct theft: the program-side loaders remain fail-closed.

**Severity.** Low-Medium: deterministic divergence, no on-chain compromise, but it breaks the IDL's core contract and strands value sent by correctly-generated clients.

**Fix.** Preserve declaration order in the macro: push into one ordered `Vec<proc_macro2::TokenStream>` instead of `seed_constants` + `seed_slice_exprs`, so `as_slices()` interleaves exactly as declared. Alternatively (weaker) reorder seeds to the effective order in `pda_attr.rs`. Add an expansion snapshot with an interleaved declaration so the ordering is pinned.

### N2 (CONFIRMED, Medium) — A non-trailing optional account slot can silently rebind every later slot at runtime (hard edge #4's runtime form is worse than documented)

**Location.** `crates/pina_macros/src/accounts.rs:163-167` emits `cursor.next_opt()`/`next_mut_opt()` for `Option<&AccountView>` fields with no trailing-position check (the only ordering constraint in the derive is `remaining` must be last, `accounts.rs:131-137`). Runtime sentinel: `crates/pina/src/traits.rs:1105-1139` — a slot holding the executing program's own address means "absent".

**Mechanism.** Mid-list, "the program id is a filler" and "the optional is genuinely absent" are indistinguishable. An attacker drops the filler and appends one extra account; the optional captures the next real account, every later positional field shifts by one, and `finish_exact` passes because the total count matches. The shift is invisible: property-based validations (`signer`, `writable`, `empty`, `not_empty`, `data_len`) re-run against the shifted bindings and can pass.

**Evidence.** Host probe (`tests/optional_shift.rs`, harness copied from `crates/pina/tests/accounts_derive.rs`), shape `[authority: &AccountView, watcher: Option<&AccountView>, vault: &mut AccountView]`:

```
honest:  accounts [01, filler(EE), 02] -> authority=01 watcher=None   vault=02   (correct)
shifted: accounts [01, 02, 03]         -> authority=01 watcher=Some(02) vault=03 finish_exact=Ok
```

The shifted list — same account count — parses successfully with the **real vault captured as the optional** and the **attacker-chosen account bound into the required `&mut` state slot**.

**Exploit scenario.** Any program whose Accounts struct declares an optional that is not the final positional field and whose subsequent fields are validated by property rather than identity. The attacker arranges `[authority, victim, attacker]`; the handler reads/writes `attacker` as `vault` while believing it is operating on the client-intended account. `address = CONST` identity pins on the later fields stop it; property-only validation does not.

**Severity.** Medium: requires the program to declare a non-trailing optional (currently accepted without warning), and the failure is fully silent — strictly worse than the opaque ~340 CU error the hard-edges doc describes.

**Fix.** In `accounts.rs`, reject an `Optional*` field that is followed by any other positional field, with a message stating the rule and the program-id filler semantics ("optional account fields must trail; earlier slots cannot distinguish a program-id filler from a real account"). This is exactly the ask in `multisig-example-hard-edges.md` item 4. Interim `pina_lints` rule: `require_trailing_optional_accounts`.

### N3 (CONFIRMED, process) — Duplicate instruction discriminators remain unpinned; the guarantee is rustc's E0081, not the macro

**Location.** `crates/pina_macros/src/discriminator.rs:137-174` requires explicit discriminants; the generated `TryFrom` match carries `#[deny(unreachable_patterns)]` (`:194`); reserved all-ones is a const assert (`:150-162`).

**Mechanism.** Probe (`src/probe_dup.rs`): `A = 1, B = 0x1` fails with rustc's own `error[E0081]: discriminant value '1' assigned more than once` at the enum, before the macro's match is even considered. The audit's "duplicate values are a compile error (unpinned by a UI test)" state is unchanged on this branch: `tests/ui/fail/` contains no discriminator duplicate pin. The de-facto guarantee is language-level (E0081), which is stronger than the deny attribute — but nothing pins either layer.

**Fix.** Add `tests/ui/fail/discriminator_duplicate_value.rs` (E0081) and `discriminator_reserved_value.rs` (the const assert) so a refactor cannot silently drop the deny attribute or the assert. Blocked-attack note: zero-variant `entrypoint` enums are rejected ("at least one instruction variant", `entrypoint.rs:120-125`); >255 variants cannot find distinct `u8` values (const-eval overflow/E0081); both fail closed.

### N4 (CONFIRMED, Low) — `seeds = []` compiles and derives an undocumented singleton PDA from the bump alone

**Location.** `args.rs:327-379` — the seed-list parser checks only the upper bound (>15); an empty list passes. `pda.rs:443-445` then emits an empty constants list; every derivation path appends the bump, so `try_find_pda`/`find_pda`/stored-bump loaders derive from the single seed `[bump]`.

**Evidence.** Probe: both `#[pda(seeds = [])]` (no bump field) and `#[account] + #[pda(seeds = [], bump = bump)]` derive `Some((ADFiabG6eWE333XuhthdESFaArhHsR1YsBvryZjTgEc5, 255))` — identical addresses, since the canonical bump search depends on nothing else.

**Exploit scenario.** An author who writes `seeds = []` believing derivation is disabled ships a program-global singleton PDA anyone can compute. If any handler trusts "this PDA namespace is per-something" the shared-PDA class (lesson 08) reappears. Fail-closed on-chain, but silently surprising.

**Severity.** Low.

**Fix.** Require at least one seed in `parse_seed_list` (or document the singleton semantics in the `#[pda]` macro docs), plus a lint (`deny_empty_pda_seeds`).

### N5 (CONFIRMED, Low/DX) — `#[pda]`-only struct with `bump` fails as a trait-bound error inside generated code

**Location.** `pda.rs:182-215` emits `assert_seeds` (and the loaders) whenever `bump` is declared, gated only on `has_account_representation` for `load_pda`/`with_*_pda` — but `assert_seeds` itself calls `AsAccount::as_account::<Self>`, requiring `Self: PinaAccount`.

**Evidence.** Probe: `#[pda(seeds = [..], bump = bump)]` without `#[account]`/`#[pinapod]` produces `error[E0277]: the trait bound 'X: PinaAccount' is not satisfied`, spanned at the attribute, originating in macro-generated code — the audit's L2 "worst case is a self-inflicted compile error" residual. The L2 remediation (qualified `Address`, identity proof) landed; this sibling case did not get a macro-level diagnostic.

**Fix.** In `pda.rs`, when `bump` is declared and `!has_account_representation`, return a `syn::Error` naming the remedy ("add `#[account]`/`#[pinapod]` so the stored bump is loadable, or remove `bump`").

### N6 (CONFIRMED, Low/DX) — Generated schema field type resolves a trait associated const through the user's namespace

**Location.** `instruction.rs:104-108` and `account.rs:140-144` emit `discriminator: [u8; #discriminator::BYTES]`. `BYTES` is an associated const on the `IntoDiscriminator` trait (`crates/pina/src/traits.rs:588+` via `into_discriminator!`), so it resolves only when the trait is in the user's scope.

**Evidence.** Probe: without `use pina::IntoDiscriminator`, `error[E0599]: no variant, associated function, or constant named 'BYTES' found for enum 'Kind'` — a confusing failure pointing at a generated field.

**Mechanism/risk.** Same robustness class as L2's unqualified `Address`: generated code depends on the user's import surface. Fail-closed (compile error), no security impact.

**Fix.** Emit `<#discriminator as #crate_path::IntoDiscriminator>::BYTES` in all three sites.

---

## Blocked attacks (checks that stop them, with pins where they exist)

| Attack                                                                    | Blocked by                                                                           | Pin                                               |
| ------------------------------------------------------------------------- | ------------------------------------------------------------------------------------ | ------------------------------------------------- |
| 16 seeds (17 with bump) — unloadable PDA                                  | `args.rs:365-376` compile error                                                      | `pda_seed_count_over_limit.rs`                    |
| Seed literal > 32 bytes                                                   | `args.rs:336-345`                                                                    | `pda_seed_too_long.rs`                            |
| `[u8; 0]` seed array                                                      | `args.rs:423-431`                                                                    | (unit-tested)                                     |
| Shadowed user `Address` in seed scope                                     | `pda.rs:164-168` identity proof                                                      | `pda_shadowed_address.rs`                         |
| Generics on schemas                                                       | `schema.rs:85-90, 365-370`                                                           | `account_generic_schema.rs`                       |
| Manual `PinaPod` derive on schemas                                        | `schema.rs:99-118`                                                                   | `account_manual_pinapod_derive.rs`                |
| Custom `ZcField` / nested schema types                                    | `schema.rs:1136-1144`                                                                | `account_custom_zc_field.rs`                      |
| `char` / `NonZero*` fields (invalid bit patterns)                         | `schema.rs:819-831`                                                                  | `event_char_field.rs`, `account_nonzero_field.rs` |
| Non-literal array lengths / unresolvable capacities                       | `schema.rs:747-752, 639-677` (named diagnostics)                                     | `account_array_const_length.rs`                   |
| Capacity overflow in SIZE math                                            | const-eval hard error (`compact_prefix_proof`, `MAX_SIZE` asserts)                   | —                                                 |
| `remaining` not last / multiple remainings / `distinct` misuse            | `accounts.rs:93-152`                                                                 | three pins                                        |
| Duplicate `#[dispatch]`, unknown args, empty enum, two entrypoints        | `entrypoint.rs:60-127`, uniqueness marker                                            | four pins                                         |
| Reserved all-ones discriminator reuse                                     | `discriminator.rs:150-162` const assert                                              | none (N3)                                         |
| Duplicate discriminator values                                            | rustc E0081 (explicit values required)                                               | none (N3)                                         |
| Duplicate mutable accounts at runtime                                     | `traits.rs:1087-1097, 1181-1227` (`remaining_mut_distinct`, `track_mutable_account`) | runtime suites                                    |
| Migration transitions drift from checked-in hash                          | `migration.rs:1460-1470` sha256 pin + `verify_source_schema` fail-closed             | Kani + suites                                     |
| Manifest-declared historical field names not valid idents / hostile types | `migration.rs:1494-1500` (`syn::parse_str` then typed emission — no token injection) | —                                                 |

Ambiguous capacity constant names are tracked in `pina_abi::SchemaConsts` (`consts.rs:121-135`) — resolution is deterministic and ambiguity is detected, closing the wrong-const-pick concern for `resolve_capacities`.

## Hard edges re-checked

- **Item 3 (constant capacities): addressed on this branch.** `schema::resolve_capacities` (`schema.rs:43-75`) rewrites free `const` items in capacity position, so `Vec<Address, MAX_MEMBERS>` is now accepted and self-consistent. Residual ask: generate the companion exported constant from the literal so hand-written `pub const`s cannot drift.
- **Item 4 (mid-list optionals): NOT addressed — and worse than documented.** See N2; the compile-time rejection remains the right fix.
- **Item 10 (empty migration-enveloped instructions need the version byte): unchanged, by design.** `migration_version: [u8; N]` is inserted at field index 1 (`instruction.rs:109-111`), so zero-field instructions require `vec![disc, 0]`. Fail-closed and harmless; the client-friction ask (`EMPTY_DATA`/`DISCRIMINATED_SIZE` const or accepting the bare discriminator for zero-payload payloads) stands.

## Proposed pina_lints rules

1. `require_trailing_optional_accounts` — error on `Option<&AccountView>`/`Option<&mut AccountView>` fields followed by another positional field (interim until the macro-level rejection lands; the message should name the program-id filler sentinel).
2. `deny_empty_pda_seeds` — warn on `#[pda(seeds = [])]` (N4).
3. UI-test pins (not lints): `discriminator_duplicate_value.rs` (E0081), `discriminator_reserved_value.rs`, and an expansion snapshot for an interleaved `#[pda]` seed list (N1's regression pin).

## Verification status

Probes live in `worktrees/security-sweep-2026-09-19/tmp/sweep/scratch-macros/` (untracked): `src/probe.rs` (seed order + empty-seed behavior, 3 passing), `src/probe_dup.rs` (E0081 evidence), `tests/optional_shift.rs` (mid-list optional shift, passing with the assertions quoted above). All probes run with `devenv shell -- cargo test`. No tracked files were modified.
