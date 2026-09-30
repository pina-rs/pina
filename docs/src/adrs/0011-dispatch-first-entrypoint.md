# ADR 0011: Dispatch-first entrypoint on the instruction-data pointer

- Status: Proposed
- Date: 2026-09-30
- Deciders: Pina maintainers
- Related: [ADR 0010](./0010-lean-entrypoint-strategy.md) (supersedes its decision 2 if accepted), [Program size](../program-size.md), [Framework comparison](../framework-comparison.md), [SIMD-0321](https://github.com/solana-foundation/solana-improvement-documents/blob/main/proposals/0321-vm-r2-instruction-data-pointer.md)

## Context

The size work that followed ADR 0010, recorded in the [program-size guide](../program-size.md), brought the comparison fixtures to:

| Fixture |  Pina | Quasar | Anchor v2 |
| ------- | ----: | -----: | --------: |
| hello   | 1,984 |  2,520 |     1,880 |
| counter | 8,632 |  7,808 |     8,696 |

Every check the fixtures ran before still runs. The counter is now 64 bytes under Anchor v2 and 824 bytes over Quasar.

### ADR 0010's reason for rejecting a dispatcher no longer holds

ADR 0010 decision 2 rejected per-instruction account parsing because "the instruction data sits _after_ the account region in the loader's input, so any dispatcher must walk every account before it can read the discriminator". That was true of the original entrypoint ABI. [SIMD-0321](https://github.com/solana-foundation/solana-improvement-documents/blob/main/proposals/0321-vm-r2-instruction-data-pointer.md) changed it: the loader now passes a pointer to the instruction data in `r2`, the entrypoint's second argument. The data's length is the `u64` stored in the eight bytes before it, the program ID follows the data, and the pointer always lands inside the input region, even for empty data. The proposal applies it under every loader. The feature (`5xXZc66h4UdB6Yq7FzdBxBiRAFMMScMLwHxk2QZDaNZL`) is active on mainnet (since epoch 950), devnet (1034), and testnet (911).

Both frameworks that beat or match pina already build on it. Quasar's generated `entrypoint(ptr, instruction_data)` slices the data from `r2` and dispatches before touching an account. Anchor v2's `__anchor_dispatch(input, ix_data_ptr)` does the same. Pinocchio 0.11.2 has no `r2` entrypoint, so pina's programs still run `deserialize::<N>` over every account before the router reads the discriminator. The ADR 0010 dispatcher prototype went through pinocchio's `InstructionContext`, which also walks accounts first, so it measured the same constraint rather than a way around it.

### Where the counter's remaining bytes are

Line-level attribution of the counter fixture, before the address-check change, against Quasar's, both built with the comparison profile:

| Cost                                               | Pina bytes | Quasar bytes |
| -------------------------------------------------- | ---------: | -----------: |
| Account parsing (walk plus typed parse)            |     ~1,400 |         ~600 |
| `initialize` (PDA check and account creation)      |     ~2,800 |       ~3,100 |
| `increment` (PDA, owner, and discriminator checks) |       ~760 |         ~540 |
| Entry, dispatch, and `ProgramError` conversion     |     ~1,480 |       ~1,650 |

Account parsing is the largest difference, and the only one in Quasar's favor worth an architectural change: pina is already smaller in `initialize` and in its entry code. Pina walks every account through pinocchio's generic deserializer, including its duplicate-account path, then parses the typed struct from that slice. Quasar parses exactly the accounts of the routed instruction.

### Measured prototype

A scratch copy of the counter fixture replaced `nostd_entrypoint!` with an entrypoint that:

1. Reads the instruction data from `r2` and the program ID after it.
2. Runs the generated router's `parse_instruction`, so the program-ID and discriminator checks are unchanged and still come first.
3. Parses exactly the routed instruction's account count from the input into a stack array. Fewer accounts fail with `NotEnoughAccountKeys`, more fail with `TooManyAccountKeys`, and duplicate markers copy the earlier view the way pinocchio does.
4. Runs the unchanged derived `TryFrom` (writable, duplicate-mutable, and exact-count checks) and the unchanged `process`.

| Build                                                     | Counter bytes |
| --------------------------------------------------------- | ------------: |
| Counter before the address-check change                   |         8,712 |
| The same, with the dispatch-first entrypoint              |         8,048 |
| Counter after the address-check change                    |         8,632 |
| The same, with the dispatch-first entrypoint              |         7,960 |
| Dispatch-first, with the error conversion folded entirely |     **7,720** |
| Quasar                                                    |         7,808 |

The per-instruction walk replaces about 1,400 bytes with 536: 328 for the three-account route and 208 for the two-account route. Building the account structs from a fixed-size array instead of the cursor saved nothing (+8 bytes), because LLVM already folds the cursor once the slice length is a constant.

The remaining 152 bytes over Quasar are smaller than the `ProgramError` → `u64` conversion the counter still carries. The error sources traced into it pass compile-time constants, but LLVM merges them into one phi in front of a shared 26-way switch instead of threading each constant to its status code. When the one remaining call that returned its result through memory was inlined by hand, LLVM folded the switch and the program measured 7,720 bytes. Folding it without hand edits is a separate lever, listed under alternatives.

The prototype measured size only. It was not executed, and its compute units were not measured.

## Decision

This is a proposal. It is not accepted until the fleet measurements and runtime checks below are done.

1. **Add a dispatch-first entrypoint for programs routed by `#[discriminator(entrypoint)]`.** The generated router gains an entry that takes the raw input and the `r2` data pointer, validates the program ID and discriminator exactly as `process_instruction` does today, then parses only the routed accounts struct's accounts. A new macro, for example `dispatch_entrypoint!(CounterInstruction)`, declares `extern "C" fn entrypoint(input: *mut u8, instruction_data: *const u8) -> u64` and installs the allocator and panic handler the way `nostd_entrypoint!` does.

2. **Pina owns the account walk.** A `#[doc(hidden)] unsafe fn` in `pina::entry`, next to `__process_entrypoint`, walks exactly `N` serialized accounts into a `[MaybeUninit<AccountView>; N]`. It uses the same record layout, alignment, and duplicate handling as pinocchio's `deserialize`. It also checks that each duplicate index names an earlier slot before copying it, a cold check that pinocchio leaves to the runtime. The `unsafe` stays in that one function, the way `__process_entrypoint` holds the current entrypoint's ([#558](https://github.com/pina-rs/pina/pull/558)).

3. **Route shapes decide the walk.**
   - Exact-count structs parse `ACCOUNT_BOUND` accounts and reject any other count before the derived checks run.
   - Structs with a `#[pina(remaining)]` field or a hand-written parser walk the transaction's actual account count, bounded by `MAX_TX_ACCOUNTS`. Programs whose routes are mostly of this shape gain little and may keep `nostd_entrypoint!`.
   - The reserved `Migrate` route parses its generated slot count like any other exact route.

4. **Error precedence stays deterministic, with one documented change.** The program-ID and discriminator checks still run before any account is read. For an instruction that has both a wrong account count and a failing per-account check, the count error now wins, because the count is checked before the derived parse runs. Today the derived parse may report a per-account error first.

5. **Roll out opt-in, then flip.** Ship `dispatch_entrypoint!` alongside `nostd_entrypoint!`. Make it the default output of `pina init` and of the comparison fixtures only after:
   - the whole-fleet comparison (all tracked examples, size, static CU, and runtime CU) shows no unapproved regression;
   - every runtime pina tests on passes the `r2` pointer. The comparison verifier's Mollusk 0.14 already does, since the Anchor v2 and Quasar fixtures depend on it. Surfpool and any LiteSVM-based harness still need to be confirmed.

6. **Supersede ADR 0010 decision 2** and the claim in its decision 3 that going below Anchor v2's size requires removing checks. The counter already measures under Anchor v2 with every check intact.

## Consequences

- **The counter can reach Quasar's size class with pina's checks.** The prototype puts it at 7,960 bytes with the fleet-safe address-check change and 7,720 when the error conversion folds, against Quasar's 7,808.
- **Instructions touch fewer accounts.** Only the routed instruction's accounts are walked, and no 255-slot array is framed. Compute units are expected to drop and must be measured before the flip.
- **New `unsafe` surface.** Walking serialized input is the same trust boundary pinocchio's deserializer crosses today, but the code becomes pina's to maintain. It needs:
  - loader-format unit tests, reusing the serializer from `entry.rs`'s tests, covering duplicates, zero accounts, data lengths, and alignment;
  - Miri coverage in `test:miri`;
  - a fuzz target over serialized inputs.
- **Hard dependency on SIMD-0321.** A program built with the new entrypoint reads an undefined `r2` on a runtime without the feature. Every public cluster has it. Local validators and test harnesses must too, which is why the default flip waits on them.
- **Two entrypoint paths coexist during the opt-in period.** Both must keep identical validation, and the macro tests must expand both.

## Alternatives considered

- **Keep pinocchio's walk with a bounded array (ADR 0010, as amended).** This is what ships today. It only removes code when the bound is five or fewer, and the counter stops at 8,632 bytes.
- **Walk lazily through pinocchio's `InstructionContext`.** Measured in ADR 0010 at +384 bytes over the bounded array, because it still walks every account before the data.
- **Build account structs from fixed arrays in the derive.** Measured at +8 bytes: LLVM already folds the cursor for constant-length slices.
- **Fold the error conversion directly.** Two formulations remain unmeasured:
  - read the `ProgramError` tag through a layout proven by const-evaluated assertions over every variant, then compute `(tag + 1) << 32`;
  - restructure the entrypoint so each route converts its own error.

  Either could remove about 900 bytes from every program that keeps the shared switch, not only the counter. Both belong in their own measurement, and the first adds `unsafe` of its own.
- **Wait for pinocchio to ship an `r2` entrypoint.** Pina would drop its own walk and adopt pinocchio's if one appears. Nothing in pinocchio 0.11.2 suggests it is imminent, and the size gap is measurable now.
- **Verify existing PDAs with `sha256` instead of `sol_create_program_address`**, as Anchor v2 and Quasar do for stored bumps. This saves compute units rather than bytes, skips the off-curve check (safe only against an address created through a verified derivation), and is a separate decision.
