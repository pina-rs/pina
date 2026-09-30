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
| counter | 8,648 |  7,808 |     8,696 |

Every check the fixtures ran before still runs. The counter is now 48 bytes under Anchor v2 and 840 bytes over Quasar.

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
3. Parses exactly the routed instruction's account count from the input into a stack array, which is right for the counter because both of its routes are exact (decision 3 covers the other shapes). Fewer accounts fail with `NotEnoughAccountKeys`, more fail with `TooManyAccountKeys`, and duplicate markers copy the earlier view the way pinocchio does.
4. Runs the unchanged derived `TryFrom` (writable, duplicate-mutable, and exact-count checks) and the unchanged `process`.

| Build                                            | Counter bytes | `initialize` CU | `increment` CU |
| ------------------------------------------------ | ------------: | --------------: | -------------: |
| Before the address-check change                  |         8,712 |           3,080 |          1,738 |
| The same, with the dispatch-first entrypoint     |         8,048 |               — |              — |
| After the address-check change                   |         8,632 |           3,073 |          1,738 |
| The same, with the dispatch-first entrypoint     |         7,960 |           3,059 |          1,728 |
| With the arithmetic error conversion             |         8,056 |           3,079 |          1,740 |
| The same, with the dispatch-first entrypoint     |     **7,248** |           3,059 |          1,728 |
| The same, re-verifying the counter with `sha256` |         7,344 |           3,059 |        **376** |
| Quasar                                           |         7,808 |           3,488 |            330 |

The per-instruction walk replaces about 1,400 bytes with 536: 328 for the three-account route and 208 for the two-account route. Building the account structs from a fixed-size array instead of the cursor saved nothing (+8 bytes), because LLVM already folds the cursor once the slice length is a constant.

An arithmetic error conversion removes the rest of the gap. LLVM merges a program's many constant errors into one value in front of `solana_program_error`'s 26-way comparison tree, so the counter carries about 900 bytes of it even though every error it returns is a constant. Computing the status from the enum's tag instead, with the layout proven at compile time, measured 8,056 bytes on the counter and 13,160 bytes smaller across the examples, but it cost 1 to 8 compute units on 53 example instructions because LLVM hoists the now-cheap tag constants onto success paths. It is held until that trade-off is decided. Together with the dispatch-first entrypoint, the counter measures 7,248 bytes, 560 under Quasar.

Every build in the table ran in the comparison verifier, which executes each instruction in Mollusk and checks the account state it leaves.

### Compute units

Instruction traces from Mollusk's register tracing account for the rest of the compute-unit gap.

- **`increment`** spends about 1,500 of its 1,738 compute units in `sol_create_program_address`, which `load_pda_mut` calls to re-derive the counter's address from its stored bump. Quasar and Anchor v2 hash the same inputs with `sol_sha256` and compare. With that one change and every pina check intact, the prototype's `increment` measured 376.
- **Without the program-ID comparison and the loader's two 32-byte address copies**, the prototype's `increment` measured 353 against Quasar's 330. Its 106 instructions against Quasar's 83 trace to four causes:
  - LLVM hoists error codes onto the success path (about 7).
  - The prototype's hand-written conversion re-checked the result on the way out (about 8); the arithmetic conversion's exit is three instructions.
  - Pina reads the duplicate marker, signer, writable, and executable bytes one at a time, where Quasar compares all four as one `u32` (about 4).
  - Pina's borrow guard marks the account borrowed and releases it (3), which ADR 0003 requires.
- **`hello`** measured 132 with the dispatch-first entrypoint, against Quasar's 115. Its success path is 32 instructions plus the 100-unit log call, and 16 of those compare the program ID with `ID`. Without that comparison it measured 117. Most of the last two instructions over Quasar are checks Quasar does not make: pina requires exactly one account and exactly one byte of instruction data, where Quasar accepts extra accounts and trailing data.

## Decision

This is a proposal. It is not accepted until the fleet measurements and runtime checks below are done.

1. **Add a dispatch-first entrypoint for programs routed by `#[discriminator(entrypoint)]`.** The generated router gains an entry that takes the raw input and the `r2` data pointer, validates the program ID and discriminator exactly as `process_instruction` does today, then parses only the routed accounts struct's accounts. A new macro, for example `dispatch_entrypoint!(CounterInstruction)`, declares `extern "C" fn entrypoint(input: *mut u8, instruction_data: *const u8) -> u64` and installs the allocator and panic handler the way `nostd_entrypoint!` does.

2. **Pina owns the account walk.** A `#[doc(hidden)] unsafe fn` in `pina::entry`, next to `__process_entrypoint`, walks exactly `N` serialized accounts into a `[MaybeUninit<AccountView>; N]`. It uses the same record layout, alignment, and duplicate handling as pinocchio's `deserialize`. It also checks that each duplicate index names an earlier slot before copying it, a cold check that pinocchio leaves to the runtime. The `unsafe` stays in that one function, the way `__process_entrypoint` holds the current entrypoint's ([#558](https://github.com/pina-rs/pina/pull/558)).

3. **Route shapes decide the walk.** `ACCOUNT_BOUND` is a declared count, not a walk length: it counts a `#[pina(remaining)]` slice as one slot and an `Option` field like a required one. The walk needs the most accounts a struct accepts (`ACCOUNT_LIMIT`) and the fewest (`ACCOUNT_MINIMUM`), which model three shapes:
   - **Exact.** A struct with only required fields, whose limit equals its minimum, parses exactly that many accounts and rejects any other count before the derived checks run.
   - **Optional.** A struct whose trailing `Option` fields may be omitted walks the accounts present, rejects more than its limit, and leaves a missing required account to the derived parse. An omitted optional account keeps working: a route with `ACCOUNT_BOUND = 2` whose second field is optional accepts one account today, and must still.
   - **Unbounded.** A struct with a `#[pina(remaining)]` field, directly or through a nested account group, and a hand-written parser that declares no limit, walk every account `nostd_entrypoint!` would hand them. Programs whose routes are mostly of this shape gain little and may keep `nostd_entrypoint!`.
   - **The reserved `Migrate` route is optional, not exact.** `run_optional` treats a missing trailing slot as omitted, so a partial migration sends fewer accounts than the slot count. It walks the accounts present, up to its slot count, and rejects an account past its last slot.
   - Tests through the new entrypoint must cover an omitted optional account and a partial migration.

4. **Error precedence stays deterministic, with one documented change.** The program-ID and discriminator checks still run before any account is read. For an instruction that has both a wrong account count and a failing per-account check, the count error now wins, because the count is checked before the derived parse runs. Today the derived parse may report a per-account error first.

5. **Roll out opt-in, then flip.** Ship `dispatch_entrypoint!` alongside `nostd_entrypoint!`. Make it the default output of `pina init` and of the comparison fixtures only after:
   - the whole-fleet comparison (all tracked examples, size, static CU, and runtime CU) shows no unapproved regression;
   - every runtime pina tests on passes the `r2` pointer. The comparison verifier's Mollusk 0.14 already does, since the Anchor v2 and Quasar fixtures depend on it. Surfpool and any LiteSVM-based harness still need to be confirmed.

6. **Supersede ADR 0010 decision 2** and the claim in its decision 3 that going below Anchor v2's size requires removing checks. The counter already measures under Anchor v2 with every check intact.

7. **Re-verify existing PDAs with `sha256`.** The stored-bump loaders of accounts the program already owns and has initialized (`load_pda` and `load_pda_mut`) compare the account's address with `sha256(seeds ‖ bump ‖ program_id ‖ "ProgramDerivedAddress")` instead of calling `sol_create_program_address`.
   - **What is skipped:** the syscall is the same hash plus a check that the result is off the ed25519 curve.
   - **Why it is sound for these loaders:** every account a pina program initializes at a seed-derived address went through `invoke_signed` with those seeds, and the runtime only signs for an off-curve address, so the stored bump already produced a valid PDA. Matching the hash identifies the same account. The only address the skipped check would add is an on-curve address equal to the hash, whose private key no one can derive and which the program never created.
   - **Where it does not apply:** account creation and checks against a caller-supplied bump keep `create_program_address`, and the canonical-bump loaders (`load_checked_pda` and `load_checked_pda_mut`) keep `try_find_program_address`, because proving a bump is the highest valid one needs the curve check.
   - Anchor v2 and Quasar verify stored-bump PDAs this way.
   - The stored-bump loaders now do this (`pina::is_derived_address`); the counter's `increment` measured 1,738 → 378.

8. **Keep the program-ID check unless the maintainers decide otherwise.** It costs 16 instructions per call. Its main protection is a clear error when the same bytecode runs at another address, since owner and PDA checks against `ID` already fail there. Making it opt-out is a product decision this ADR leaves open.

9. **Compare account header flags as one word where the derive knows them.** When an accounts struct states whether a field must be a non-duplicate signer, writable, or non-executable, the parser can check all four header bytes with one `u32` comparison, as Quasar does, instead of four byte reads.

## Consequences

- **The counter goes below Quasar with pina's checks.** With the arithmetic error conversion, the prototype measures 7,248 bytes against Quasar's 7,808.
- **`increment` approaches Quasar's compute units.** `sha256` re-verification takes it from 1,728 to 376, against Quasar's 330. The rest of the gap is the hoisted error codes, the byte-wise header checks, and pina's borrow guard.
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
- **Fold the error conversion by inlining.** Folding only happens when LLVM threads every constant error to its own status, which it stops doing once a program has many error sites. The arithmetic conversion reads the `ProgramError` tag through a layout proven by const-evaluated assertions over every variant, then computes `(tag + 1) << 32`. Reading the tag by value instead of through a pointer produced identical binaries.
- **Wait for pinocchio to ship an `r2` entrypoint.** Pina would drop its own walk and adopt pinocchio's if one appears. Nothing in pinocchio 0.11.2 suggests it is imminent, and the size gap is measurable now.
- **Drop the exact account-count and data-length checks.** They are the last two `hello` instructions over Quasar. Rejecting extra accounts and trailing instruction data is part of pina's validation contract, so this ADR keeps them.
