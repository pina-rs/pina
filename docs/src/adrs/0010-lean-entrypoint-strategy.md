# ADR 0010: Lean entrypoint strategy for deployed-size parity

- Status: Proposed
- Date: 2026-09-27
- Deciders: Pina maintainers
- Related: [Framework comparison](../framework-comparison.md), [Program size](../program-size.md)

## Context

After the fat-LTO conversion, the comparison fixtures measure:

| Fixture     |   Pina | Pinocchio | Quasar | Anchor v2 |
| ----------- | -----: | --------: | -----: | --------: |
| hello world |  4,680 |     3,160 |  2,520 |     1,880 |
| counter     | 12,720 |     6,512 |  7,808 |     8,696 |

Pina is the largest program in both rows. The question this ADR answers: **what would the project look like if it targeted the same ELF size as Anchor v2 and Quasar, and is pinocchio the bottleneck?**

Disassembly answers the second part directly: **pinocchio is not the bottleneck.** Anchor v2 itself is built on pinocchio 0.11 and still ships a 1,880-byte hello world. The bottleneck is _how the entrypoint uses pinocchio_, plus layers of cost that are pina's own.

### Where the bytes actually are

Symbol-level breakdown of the counter fixture's `.text`:

| Symbol                                                                 | Bytes | Whose cost is this?           |
| ---------------------------------------------------------------------- | ----: | ----------------------------- |
| `entrypoint` (pinocchio `program_entrypoint!` + inlined pina dispatch) | 7,736 | mixed — see below             |
| `program_error_to_u64`                                                 |   864 | pinocchio error conversion    |
| 3 × `RangeInclusive<usize>::index`                                     |   648 | pina/pinapod envelope slicing |
| `assert_owner` / `assert_address`                                      |   400 | pina validation               |
| `combine_seeds_with_bump`                                              |   304 | pina CPI                      |
| slice-index panic plumbing                                             |  ~450 | `RangeInclusive` support      |

The 7,736-byte `entrypoint` decomposes further. Pinocchio's deserializer walks the account region with a **compile-time-unrolled** `process_n_accounts!` macro — five accounts per macro expansion, remainder handled by match arms — so a program that declares `MAX_TX_ACCOUNTS` (the default, 255) carries unrolled walking code for 255 accounts even when every instruction uses two. Quasar's hello-world `entrypoint` is 1,096 bytes; pina's is 2,560. Anchor v2's is **32 bytes**: a stub that forwards to an `#[inline(never)]` dispatcher.

**The budget is the cheapest lever and it already exists.** `nostd_entrypoint!` has always accepted a second argument for the account budget; the fixtures never passed one. The prototype (`benchmarks/framework-comparison/programs/*/pina_lean`, built with the exact comparison profile and proven by the comparison verifier) measured it: hello 4,680 → 2,736 bytes (−41.5%) with `budget = 1`, counter 12,720 → 11,680 (−8.2%) with `budget = 4`, at +6 CU and +1/+4 CU respectively. Any program can take this today with no framework change.

### What Quasar and Anchor v2 do differently

Five techniques, all compatible with keeping pinocchio where it earns its keep:

1. **Do not eager-deserialize all accounts.** Pinocchio's `program_entrypoint!` builds `AccountView`s for every account before the program's code runs. Anchor v2 never calls it: its `__anchor_dispatch` stub (32 bytes) receives the raw `*mut u8` input and hands it to an `#[inline(never)]` dispatcher that walks accounts **lazily** through an `AccountCursor` — exactly `HEADER_SIZE` accounts per instruction, and the remaining-account walk is a tight runtime loop, not an unrolled macro. Quasar's `dispatch!` is per-arm: each instruction arm parses precisely `COUNT` accounts with one shared loop, in place, into a `[MaybeUninit<AccountView>; COUNT]` on the stack.

2. **32-byte entrypoint stubs.** Both frameworks generate `entrypoint` as a thin `extern "C"` function that jumps to the real dispatcher, keeping the BPF loader's required symbol tiny and isolating the stack-heavy body out of the frame the loader sees.

3. **Per-instruction account-count constants.** Quasar's `dispatch!` arms each know `<$accounts_ty as AccountCount>::COUNT` — a compile-time constant per arm — so `Initialize` in the counter fixture parses 3 accounts, `Increment` parses 2, and no code for a 64-account instruction exists anywhere.

4. **Shared, loop-based account walking.** Where pina inherits pinocchio's unrolled walk, both competitors walk accounts with a single compact loop that LLVM keeps small. The unrolled form is deliberately CU-cheaper (fewer jump instructions) but costs roughly 1.4 KB of `.text` for MAX_TX_ACCOUNTS-scale arrays — a size/CU trade pinocchio resolved in favor of CU.

5. **Error-path outlining.** Anchor v2 and Quasar funnel every failure through one `#[cold]` conversion function (`ProgramError → u64`), one panic handler, and per-arm dispatch that never inlines. Pina already does this in places (`remap_custom_error` is `#[cold]`), but its `Accounts` derive still inlines per-field assertions into the caller's frame.

### Measured prototype

The prototype fixtures measured the budget lever in isolation — everything else stock — with the verifier proving each instruction ran and left the expected state:

| Fixture                                               | Stock (default 255) | Lean (bounded budget) | Δ          | CU stock → lean              |
| ----------------------------------------------------- | ------------------: | --------------------: | ---------- | ---------------------------- |
| hello (`nostd_entrypoint!(process_instruction, 1)`)   |               4,680 |             **2,736** | **−41.5%** | 145 → 151                    |
| counter (`nostd_entrypoint!(process_instruction, 4)`) |              12,720 |            **11,680** | **−8.2%**  | 3,295 → 3,296; 1,753 → 1,757 |

The bounded budget alone brings hello to within 216 bytes of Quasar's 2,520 — with zero unsafe code, zero new macros, only the documented second argument of `nostd_entrypoint!`. The counter gains less because its `.text` is dominated by derive-generated dispatch and the `RangeInclusive` envelope triplication, not the deserializer; the symbol table shows its `entrypoint` fell 7,736 → ~1,256 bytes, but the freed budget re-surfaced as extra arms in the inlined router.

A second measurement isolated the router lever: outlining the router with `#[inline(never)]` (the stub-plus-dispatcher split Anchor v2's 32-byte `entrypoint` uses) _grew_ the counter to 11,800 bytes — LLVM merges more when the router inlines into one frame. The budget is the dominant lever; the router split is neutral-to-negative until the per-arm walking of the lean dispatcher exists.

## Decision

1. **Adopt the account budget now** (no framework change): document the second argument of `nostd_entrypoint!` in the program-size guide, and have `pina init` emit a program-specific bound — the program's widest instruction's account count plus headroom — instead of the 255 default.
2. **Build the lean dispatcher as an opt-in macro** (`lean_entrypoint!`) that replaces pinocchio's eager entrypoint deserialization with a stub-plus-per-arm dispatcher: per-instruction `COUNT` parsing, one loop-based account walk with pinocchio's duplicate-marker semantics, and shared outlined error paths. Keep pinocchio as a _library_ (CPI, sysvars, `AccountView`); stop using it as the eager 255-account entrypoint deserializer on the lean path.
3. **Gate the default flip on equivalence**: lean entrypoint becomes the `pina init` default only after Surfpool equivalence suites and the CU ratchet record both paths on every example program, with multisig — whose entrypoint frame sits exactly at the 4 KB stack limit — measured first.

The comparison fixtures keep publishing stock numbers; the `pina_lean` fixtures remain as the measurement bed for the next phase.

## Consequences

- **Deployed sizes approach Quasar's class without losing checks.** Measured: hello −41.5% (to 2,736, near Quasar's 2,520); projected for the full lean dispatcher: hello ≈ 2.3–2.6 KB, counter ≈ 7.8–8.4 KB — between Quasar (7,808) and Anchor v2 (8,696), with every validation pina performs today still performed.
- **The CU lead is retained.** The budget lever costs +6 CU on hello and +1/+4 CU on the counter; pina's `initialize` stays under Anchor v2's 3,458 and Quasar's 3,488. Per-arm exact parsing can reduce CU further by skipping 255-slot framing entirely.
- **Two entrypoint paths exist until the default flips.** Both must stay feature-complete; the migration-aware discriminator envelope checks keep their ordering (envelope → program ID → accounts) on both paths.
- **Programs with many remaining-accounts instructions** (for example account-array sweeps) must keep the unrolled path or pass a large budget — the lean dispatcher's per-arm walking must be measured on such a program before the flip (multisig first).
- **Anchor v2's 1,880-byte hello is not reachable while pina keeps its discriminant model**, and that trade is deliberate: Anchor's hello checks nothing beyond the program ID; pina's hello validates a program ID, a discriminator envelope, and a signer, and its CU numbers are the payoff (hello 145 CU vs Anchor's 127, initialize 3,295 vs 3,458). The committed target is **Quasar-class size with pina's CU advantage intact**, not Anchor's absolute minimum.

## Alternatives considered

- **Drop pinocchio entirely (Quasar's route: `solana-account-view` + static syscalls).** Rejected as a first step: Anchor v2 proves pinocchio the library is compatible with 32-byte entrypoints, and dropping it forfeits audited CPI/sysvar code and the `AccountView` semantics every pina program already targets. Revisit only if the lean dispatcher still leaves a gap.
- **Lower the default budget for everyone.** Rejected: the budget is a program-level contract (more accounts than the bound are ignored, not rejected), so a global default would silently change behavior for programs that accept many remaining accounts. The bound must be chosen per program.
- **Only outline the router (`#[inline(never)]`), keep the 255-slot deserializer.** Measured and rejected: it grew the counter fixture by 120 bytes; the budget and per-arm parsing are where the bytes are.
- **`opt-level = "z"`/`"s"` for size.** Previously measured in the program-size guide: shrank `.text` slightly but grew deployed ELFs (alignment and section-layout side effects) and regressed CU; LTO with `opt-level = 3` dominates.
