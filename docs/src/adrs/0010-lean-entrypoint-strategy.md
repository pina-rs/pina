# ADR 0010: Lean entrypoint strategy for deployed-size parity

- Status: Accepted (decisions 1–2 shipped or measured; the dispatcher is decided against)
- Date: 2026-09-27 (lean-dispatcher measurement added 2026-09-28; decision 1 amended 2026-09-29; decision 2 revisited by ADR 0011 on 2026-09-30)
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

**The budget is the cheapest lever and it already exists.** `nostd_entrypoint!` has always accepted a second argument for the account budget; the fixtures never passed one. The prototype (`benchmarks/framework-comparison/programs/*/pina_lean`, built with the exact comparison profile and proven by the comparison verifier) measured it: hello 4,680 → 2,736 bytes (−41.5%) with `budget = 1`, counter 11,400 → 9,976 (−12.5%) with `budget = 3`, at +6 CU and −93/+4 CU respectively. Any program can take this today with no framework change.

**The seed-and-signer slicing fix shipped alongside it.** The PDA-creation CPI spine sliced its seed and signer arrays with inclusive ranges (`[a..=len]`), monomorphizing a 216-byte `RangeInclusive<usize>::index` copy per element type — three copies, ~1.3 KB, in every PDA-creating program. Exclusive bounds (`[a..len + 1]`, guarded by the `len < MAX` checks that already ran) measured −1,320 bytes and −92 CU on the counter's `initialize` with byte-identical behavior, and now live in the shipped crate.

**Two candidate levers measured out and were dropped.** A compact cold error converter: the exhaustive `ProgramError → u64` switch costs 864 bytes, a small-immediate-plus-shared-shift reformulation costs 832, and Anchor v2 pays the same 864 — the conversion is table stakes for the ABI, not a differentiator. Unifying `assert_owner`/`assert_address` instantiations: their bulk is the inlined 32-byte address comparison on the success path, which cannot be outlined without adding compute units to every check; the shareable log tails are ~24 bytes each.

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
| counter (`nostd_entrypoint!(process_instruction, 3)`) |              11,400 |             **9,976** | **−12.5%** | 3,203 → 3,202; 1,753 → 1,757 |

The bounded budget alone brings hello to within 216 bytes of Quasar's 2,520 — with zero unsafe code, zero new macros, only the documented second argument of `nostd_entrypoint!`. The counter gains less because its `.text` is dominated by derive-generated dispatch and the `RangeInclusive` envelope triplication, not the deserializer; the symbol table shows its `entrypoint` fell 7,736 → ~1,256 bytes, but the freed budget re-surfaced as extra arms in the inlined router.

A second measurement isolated the router lever: outlining the router with `#[inline(never)]` (the stub-plus-dispatcher split Anchor v2's 32-byte `entrypoint` uses) _grew_ the counter to 11,800 bytes — LLVM merges more when the router inlines into one frame. The budget is the dominant lever; the router split is neutral-to-negative until the per-arm walking of the lean dispatcher exists.

## Decision

1. **Adopt the account budget now** (no framework change): document the second argument of `nostd_entrypoint!` in the program-size guide, and have `pina init` emit a program-specific bound — the program's widest instruction's account count plus headroom — instead of the 255 default.

   _Amended 2026-09-29._ A bound equal to the widest instruction is not safe: the loader skips accounts past the entrypoint's array instead of rejecting them, so the `budget = 1` and `budget = 3` fixtures measured above no longer rejected an extra trailing account with `TooManyAccountKeys` — `finish_exact` never saw it. The bound must keep one spare slot. `#[discriminator(entrypoint)]` now generates it as `ENTRYPOINT_ACCOUNT_CAPACITY` (the widest routed accounts struct or reserved `Migrate` route, plus one, or 255 when a route accepts unbounded trailing accounts), and the [program-size guide](../program-size.md#bound-the-entrypoint-account-array) documents it. The mechanism is also narrower than this ADR's context states: pinocchio walks accounts five at a time rather than unrolling one body per slot, so a bounded array only removes code when it has five slots or fewer, and a larger bound grows the program by the loop that skips accounts past it. With the spare slot and the seed-capacity change, the fixtures measure hello 4,680 → 2,944 and counter 10,456 → 9,416, at +1 compute unit on `hello` and `initialize` and +4 on `increment`.
2. **Do not build the lean dispatcher — it was prototyped and measured out.** A working prototype dispatching through pinocchio's public `InstructionContext` (lazy walk, duplicate mapping, exact-count views) measured **10,360 bytes against the budget lever's 9,976** on the counter fixture: `.text` 7,936 vs 7,736 plus 48 more relocations. Two architectural facts close the question. First, the instruction data sits _after_ the account region in the loader's input, so any dispatcher must walk every account before it can read the discriminator — "parse only the matched arm's accounts" is unreachable under the entrypoint ABI. Second, pinocchio's compile-time-unrolled `deserialize::<N>` for a small bound is tighter than any general loop with duplicate handling; the lazy walk duplicates that work in a less specialized shape. The dispatcher layer was never the remaining gap: after the budget and slicing levers, the counter's ~1,030 bytes of `.text` over Anchor v2 sit in pina's semantic surface — the seed-and-signer assembly, the outlined address-comparison asserts, and the envelope plumbing — each individually load-bearing and each CU-cheaper than Anchor's runtime equivalent.

   _Amended 2026-09-30._ [ADR 0011](./0011-dispatch-first-entrypoint.md) proposes reversing this decision. Its first premise no longer holds: since SIMD-0321 the loader passes a pointer to the instruction data at entry, so a dispatcher can read the discriminator before walking any account. The prototype there measured the counter fixture at 7,960 bytes with every check intact.
3. **Stop here and keep the CU lead.** The counter at 9,976 bytes (−21.6% from stock) with `initialize` at 3,203 CU versus Anchor v2's 8,696 bytes at 3,458 CU is the measured optimum for pina's feature set. Going below Anchor's byte count requires removing checks (an opt-out envelope mode, weaker derivation verification), which is a product decision with security trade-offs, not an engineering lever; this ADR records the measurement so the dispatcher is not re-attempted without new constraints.

The comparison fixtures keep publishing stock numbers; the `pina_lean` fixture remains as the measurement bed documenting the budget and slicing levers. _Amended 2026-09-29:_ the published Pina fixtures now use the generated router with `ENTRYPOINT_ACCOUNT_CAPACITY` — the configuration this ADR recommends — and the `pina_lean` fixtures, which measured the unsafe bound, were removed.

## Consequences

- **Deployed sizes landed at Quasar's class neighborhood without losing checks.** Measured: hello −41.5% (to 2,736, near Quasar's 2,520); counter −21.6% from stock (12,720 → 9,976 with the budget and slicing levers shipped here). The remaining 1,280 bytes to Anchor v2's 8,696 are the feature surface itemized in decision 2; the dispatcher prototype proved they are not recoverable by entrypoint architecture.
- **The CU lead is retained.** The budget lever costs +6 CU on hello and +1/+4 CU on the counter; pina's `initialize` stays under Anchor v2's 3,458 and Quasar's 3,488. Per-arm exact parsing can reduce CU further by skipping 255-slot framing entirely.
- **Two entrypoint paths exist until the default flips.** Both must stay feature-complete; the migration-aware discriminator envelope checks keep their ordering (envelope → program ID → accounts) on both paths.
- **Programs with many remaining-accounts instructions** (for example account-array sweeps) must keep the unrolled path or pass a large budget — the lean dispatcher's per-arm walking must be measured on such a program before the flip (multisig first).
- **Anchor v2's 1,880-byte hello is not reachable while pina keeps its discriminant model**, and that trade is deliberate: Anchor's hello checks nothing beyond the program ID; pina's hello validates a program ID, a discriminator envelope, and a signer, and its CU numbers are the payoff (hello 145 CU vs Anchor's 127, initialize 3,295 vs 3,458). The committed target is **Quasar-class size with pina's CU advantage intact**, not Anchor's absolute minimum.

## Alternatives considered

- **Drop pinocchio entirely (Quasar's route: `solana-account-view` + static syscalls).** Rejected as a first step: Anchor v2 proves pinocchio the library is compatible with 32-byte entrypoints, and dropping it forfeits audited CPI/sysvar code and the `AccountView` semantics every pina program already targets. Revisit only if the lean dispatcher still leaves a gap.
- **Lower the default budget for everyone.** Rejected: the budget is a program-level contract (more accounts than the bound are ignored, not rejected), so a global default would silently change behavior for programs that accept many remaining accounts. The bound must be chosen per program.
- **Only outline the router (`#[inline(never)]`), keep the 255-slot deserializer.** Measured and rejected: it grew the counter fixture by 120 bytes; the budget and per-arm parsing are where the bytes are.
- **`opt-level = "z"`/`"s"` for size.** Previously measured in the program-size guide: shrank `.text` slightly but grew deployed ELFs (alignment and section-layout side effects) and regressed CU; LTO with `opt-level = 3` dominates.
- **Lazy dispatch through pinocchio's `InstructionContext` (the lean dispatcher of decision 2's first draft).** Prototyped and measured: +384 bytes over the budget lever on the counter fixture. The data-after-accounts input layout forces a full account walk before the discriminator is readable, and the specialized unrolled walk beats a general loop. Also measured alongside it: the `#[inline]` hint on the router changes nothing (9,976 either way; LLVM already picks the same layout), and a shared cold `ProgramError → u64` converter is table stakes — the exhaustive switch costs 864 bytes in every formulation and Anchor v2 carries the identical 864.
- **Unifying the seed-and-signer assemblies in the PDA-creation spine.** Estimated at ~100–150 bytes across two private function variants; the double assembly is real but small, recorded here as the largest known remaining purely-mechanical lever if the trade ever becomes worth it.
