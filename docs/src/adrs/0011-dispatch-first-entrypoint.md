# ADR 0011: Dispatch-first entrypoint on the instruction-data pointer

- Status: Accepted
- Date: 2026-09-30
- Deciders: Pina maintainers
- Related: [ADR 0010](./0010-lean-entrypoint-strategy.md) (supersedes its decision 2), [Program size](../program-size.md), [Framework comparison](../framework-comparison.md), [SIMD-0321](https://github.com/solana-foundation/solana-improvement-documents/blob/main/proposals/0321-vm-r2-instruction-data-pointer.md)

## Context

The size work that followed ADR 0010, recorded in the [program-size guide](../program-size.md), brought the comparison fixtures to:

| Fixture |  Pina | Quasar | Anchor v2 |
| ------- | ----: | -----: | --------: |
| hello   | 1,984 |  2,520 |     1,880 |
| counter | 8,424 |  7,808 |     8,696 |

Every check the fixtures ran before still runs. The counter is now 272 bytes under Anchor v2 and 616 bytes over Quasar.

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

Accepted. Decisions 1 to 5 and 10 are implemented as described below, which refines the proposal in two places: the reserved `Migrate` route reads its accounts like an unbounded route while `process_migrate` enforces its slot limit (decision 3), and a program with many routes shares one copy of the walk (decision 10). The [results](#results) record the fleet measurements and runtime checks the proposal required.

1. **Add a dispatch-first entrypoint for programs routed by `#[discriminator(entrypoint)]`.** The generated router gains `__dispatch`, which takes the loader's input and the instruction data, validates the program ID and discriminator exactly as `process_instruction` does, then parses only the routed accounts struct's accounts. `dispatch_entrypoint!(CounterInstruction)` declares `extern "C" fn entrypoint(input: *mut u8, instruction_data: *const u8) -> u64`, reads the data and program ID through the `r2` pointer, and installs the allocator and panic handler the way `nostd_entrypoint!` does.

2. **Pina owns the account walk.** Private functions in `pina::entry` walk the serialized accounts into a `[MaybeUninit<AccountView>; N]` with the record layout, alignment, and duplicate handling of pinocchio's `deserialize`. They also check that each duplicate index names an earlier slot before copying it, a cold check that pinocchio leaves to the runtime.
   - The entrypoint wraps the raw input in `EntrypointInput`, which only pina can create. Its parse methods hold the `unsafe` and take the input by value, so the generated router contains no `unsafe` block and cannot walk the input twice. A generated block would carry the program's spans and trip a program's `unsafe_code` lint, which `#[allow]` cannot override under `forbid`. A second walk after a handler resized an account would read the changed data length.
   - A property test serializes random account layouts, with duplicates and varied data lengths, and requires the walk to find the same views as `deserialize` and leave the input byte for byte as it does. It replaces the fuzz target this ADR proposed; the walk's unit tests and an end-to-end `dispatch_entrypoint!` program also run under Miri.

3. **Route shapes decide the walk.** `ACCOUNT_BOUND` is a declared count, not a walk length: it counts a `#[pina(remaining)]` slice as one slot and an `Option` field like a required one. `#[derive(Accounts)]` therefore declares `ParseAccounts::ACCOUNT_LIMIT`, the most accounts a struct accepts, and `ACCOUNT_MINIMUM`, the fewest. Hand-written parsers keep an unbounded limit and a zero minimum. The two model three shapes:
   - **Exact.** When the limit equals the minimum, the route parses a fixed-length array and rejects any other count before the derived checks run.
   - **Optional.** When trailing `Option` fields make them differ, the walk reads the accounts present, rejects more than the limit, and leaves a missing required account to the derived parse, so an omitted optional account keeps working.
   - **Unbounded.** A struct with a `#[pina(remaining)]` field, directly or through a nested account group, and a hand-written parser with no limit read the first `ENTRYPOINT_ACCOUNT_CAPACITY` accounts, which is the transaction maximum whenever such a route exists and is what `nostd_entrypoint!` hands them. The walk stops at the array's end, since the entrypoint already has the data from `r2`, so pinocchio's deserializer is not linked at all.
   - **The reserved `Migrate` route is optional, not exact.** `run_optional` treats a missing trailing slot as omitted, so a partial migration sends fewer accounts than the slot count. The route reads the accounts present like an unbounded route, and `process_migrate` reads only its slots and fails on an account past the last one.
   - Tests through the new entrypoint cover an omitted optional account and a partial migration.

4. **Error precedence stays deterministic, with one documented change.** The program-ID and discriminator checks still run before any account is read. For an instruction that has both a wrong account count and a failing per-account check, the count error now wins, because the count is checked before the derived parse runs. Today the derived parse may report a per-account error first.

5. **Roll out per program.** `dispatch_entrypoint!` ships alongside `nostd_entrypoint!`.
   - Every runtime pina tests on passes the `r2` pointer: Mollusk, which the comparison verifier and `tests/compute_units.rs` use, and Surfpool, whose suites for every converted example pass on the new entrypoint. The TypeScript LiteSVM suite (`codama/tests/litesvm`) loads only `role_registry_program` and `optional_accounts_program`, which keep `nostd_entrypoint!`, so it does not run the new entrypoint yet.
   - The comparison fixtures and the `counter`, `migrations`, `escrow`, and `staking_rewards` examples use it. `multisig_program` keeps `nostd_entrypoint!`, because its twenty routes measured larger (see the [results](#results)).
   - `pina init` generates a hand-written `process_instruction`, not the `#[discriminator(entrypoint)]` router, so it keeps `nostd_entrypoint!` until its template moves to the router.

6. **Supersede ADR 0010 decision 2** and the claim in its decision 3 that going below Anchor v2's size requires removing checks. The counter already measures under Anchor v2 with every check intact.

7. **Re-verify existing PDAs with `sha256`.** The stored-bump loaders of accounts the program already owns and has initialized (`load_pda` and `load_pda_mut`) compare the account's address with `sha256(seeds ‖ bump ‖ program_id ‖ "ProgramDerivedAddress")` instead of calling `sol_create_program_address`.
   - **What is skipped:** the syscall is the same hash plus a check that the result is off the ed25519 curve.
   - **Why it is sound for these loaders:** every account a pina program initializes at a seed-derived address went through `invoke_signed` with those seeds, and the runtime only signs for an off-curve address, so the stored bump already produced a valid PDA. Matching the hash identifies the same account. The only address the skipped check would add is an on-curve address equal to the hash, whose private key no one can derive and which the program never created.
   - **Creation:** the builders' pre-check before the create-account CPI uses the same hash. There the runtime repeats the curve check itself, because it only signs the allocation for an off-curve address; an on-curve bump fails with "Could not create program address with signer seeds".
   - **Where it does not apply:** checks of a caller-supplied bump that no `invoke_signed` follows keep `create_program_address`, and the canonical-bump loaders (`load_checked_pda` and `load_checked_pda_mut`) keep `try_find_program_address`, because proving a bump is the highest valid one needs the curve check.
   - Anchor v2 and Quasar verify stored-bump PDAs this way.
   - The stored-bump loaders and the creation pre-check now do this. On the counter, `increment` measured 1,738 → 378 and `initialize` 3,073 → 1,713.

8. **Keep the program-ID check unless the maintainers decide otherwise.** It costs 16 instructions per call. Its main protection is a clear error when the same bytecode runs at another address, since owner and PDA checks against `ID` already fail there. Making it opt-out is a product decision this ADR leaves open.

9. **Compare account header flags as one word where the derive knows them.** When an accounts struct states whether a field must be a non-duplicate signer, writable, or non-executable, the parser can check all four header bytes with one `u32` comparison, as Quasar does, instead of four byte reads.

10. **Share the walk in a router with more than two routes.** An inlined walk is the faster one: a fixed-length route unrolls it and folds the checks its positions rule out. Every route carries its own copy, though, 150 to 300 bytes each. A router with more than two routes calls one out-of-line copy instead, which returns `bool` so its result stays in a register. The reserved `Migrate` route follows its program's choice without counting toward it. On the counter fixture the shared walk measured +64 bytes and +35 compute units on `increment`; on the examples with seven or more routes it is what makes the new entrypoint smaller than `nostd_entrypoint!`.

## Results

Measured against `main` after the `sha256` loaders, the creation check, and the capacity fix had merged.

| Fixture                               | Pina before |      Pina after |      Quasar |     Anchor v2 |
| ------------------------------------- | ----------: | --------------: | ----------: | ------------: |
| hello bytes                           |       1,984 |       **1,616** |       2,520 |         1,880 |
| hello CU                              |         146 |         **136** |         115 |           127 |
| counter bytes                         |       8,424 |       **7,592** |       7,808 |         8,696 |
| counter `initialize` / `increment` CU | 1,713 / 378 | **1,694 / 360** | 3,488 / 330 | 3,458 / 2,117 |

The hello fixture is now the smallest of the four frameworks, and the counter is 216 bytes under Quasar with every check pina made before.

| Program                    | `nostd_entrypoint!` | `dispatch_entrypoint!` | Change |
| -------------------------- | ------------------: | ---------------------: | -----: |
| hello comparison fixture   |               1,984 |                  1,616 |   −368 |
| counter comparison fixture |               8,424 |                  7,592 |   −832 |
| `counter_program`          |              13,192 |                 12,984 |   −208 |
| `migrations_program`       |              39,512 |                 37,952 | −1,560 |
| `escrow_program`           |              41,912 |                 38,912 | −3,000 |
| `staking_rewards_program`  |              52,504 |                 50,192 | −2,312 |

Across the fleet, 13 programs got smaller (−16 to −3,000 bytes) and two larger: `transfer_sol_program` (+40) and `profile_program` (+16), both on `nostd_entrypoint!` and changed only by the inlined account cursor. 61 instructions got cheaper, most by 30 to 170 compute units. The increases are:

- `migrations_program/update`, +34 (+28 for a historical payload). The route runs its instruction through `process_versioned`, and LLVM compiles that call less well inside the dispatch-first arm; without it the historical update measured 157 compute units against `nostd_entrypoint!`'s 171.
- `pina_bpf_program/createPda` and `todo_program/initialize` +4, `compact_accounts_program/rename` +2, and three instructions +1, all on `nostd_entrypoint!` and all from the inlined cursor, without which the counter measured 800 bytes larger and its instructions 40 compute units dearer.

## Consequences

- **The counter is below Quasar with pina's checks.** It measures 7,592 bytes against Quasar's 7,808, without the arithmetic error conversion, and the hello fixture is the smallest of the four frameworks.
- **`increment` approaches Quasar's compute units.** It measures 360 against Quasar's 330. The rest of the gap is the hoisted error codes, the byte-wise header checks, and pina's borrow guard.
- **Instructions touch fewer accounts.** Only the routed instruction's accounts are walked, and no 255-slot array is framed.
- **Pina maintains an account walk.** Walking serialized input is the trust boundary pinocchio's deserializer crossed, now in pina's code. Unit tests over loader-format input, Miri, and the property test against `deserialize` guard it.
- **Hard dependency on SIMD-0321.** A program built with the new entrypoint reads an undefined `r2` on a runtime without the feature. Every public cluster, Surfpool, and Mollusk have it.
- **Two entrypoint paths coexist.** Both keep identical validation, and the macro tests expand both.
- **A large router can grow.** Each route parses a fixed-length struct, which folds its checks but stops LLVM merging the routes' identical parsing code. `multisig_program` stays on `nostd_entrypoint!` for that reason.
- **The account cursor inlines everywhere.** `AccountsCursor::next` and `next_mut` are `#[inline(always)]`, without which the counter measured 8,392 bytes and 40 compute units more. It also changes programs on `nostd_entrypoint!`, mostly for the better (see the [results](#results)).

## Alternatives considered

- **Keep pinocchio's walk with a bounded array (ADR 0010, as amended).** This is what ships today. It only removes code when the bound is five or fewer, and the counter stops at 8,632 bytes.
- **Walk lazily through pinocchio's `InstructionContext`.** Measured in ADR 0010 at +384 bytes over the bounded array, because it still walks every account before the data.
- **Build account structs from fixed arrays in the derive.** Measured at +8 bytes: LLVM already folds the cursor for constant-length slices.
- **Fold the error conversion by inlining.** Folding only happens when LLVM threads every constant error to its own status, which it stops doing once a program has many error sites. The arithmetic conversion reads the `ProgramError` tag through a layout proven by const-evaluated assertions over every variant, then computes `(tag + 1) << 32`. Reading the tag by value instead of through a pointer produced identical binaries.
- **Wait for pinocchio to ship an `r2` entrypoint.** Pina would drop its own walk and adopt pinocchio's if one appears. Nothing in pinocchio 0.11.2 suggests it is imminent, and the size gap is measurable now.
- **Inline the walk in every route.** The fastest code, but before the leading walk replaced pinocchio's deserializer it grew `escrow_program` by 1,560 bytes, `staking_rewards_program` by 2,536, and `multisig_program` by 4,488 over `nostd_entrypoint!`.
- **Share the walk in every program.** The smallest `counter_program`, but the hello fixture grew 400 bytes and 13 compute units, and the counter fixture 64 bytes and 22 to 35 compute units.
- **Walk bounded instead of exact in routers that share the walk,** so routes keep a variable-length slice whose parsing LLVM can merge. It measured 448 to 768 bytes larger on every example with seven or more routes: the fixed-length fold is worth more than the merge.
- **Drop the exact account-count and data-length checks.** They are the last two `hello` instructions over Quasar. Rejecting extra accounts and trailing instruction data is part of pina's validation contract, so this ADR keeps them.
