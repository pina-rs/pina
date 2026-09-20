# Fix retrospective — were the shipped fixes the best available?

Written 2026-09-20, after the sweep fixes merged (#472). For each confirmed finding: what shipped, what the real alternatives were, whether the shipped fix was the right call, and — where it was not the ceiling — the durable fix worth building. Ordered by how much design depth the finding has, not by severity.

---

## E2/E3 (multisig consent re-binding, spending-limit revival) — shipped fix is a patch; the durable fix is one mechanism

**Shipped:** `VaultExecute` and `ConfigExecute` now run the expiry and stale-index guards every other path runs; `RemoveMember` prunes spending-limit rosters it receives; `SpendingLimitUse` refuses any drawer who is not a current member.

**Why that is not enough.** All three patches defend _positions_ (this handler, that action), and the CodeRabbit residual proved the class survives them: removal only prunes the limit accounts its execution receives, and `AddMember` later re-creates the precondition (address is a member) while the stale roster entry supplies the authority. A guard that asks "is this address a member right now?" cannot distinguish _member again_ from _member who never left_, because membership has no identity across roster changes.

**The durable fix — a membership generation.** Add `membership_epoch: u64` to the multisig state, bumped by every roster mutation (add and remove). Record the epoch in each spending limit at grant time and in each proposal's approval record. Draw and execute then require `grant.epoch == multisig.membership_epoch` (or: the recorded per-member approvals must all name the _current_ epoch). One mechanism closes:

- the E3 re-add residual (a stale grant's epoch cannot match);
- the E2 mask re-binding (approvals recorded per epoch cannot re-bind, because compaction cannot forge an epoch); and
- the freeze-as-only-defense property: today a roster change must freeze _every_ prior proposal because bitmasks are positional; with per-epoch recorded approvals, unaffected proposals could stay live — a governance-liveness win, not just a safety one.

It is a layout change (migration + IDL regeneration), which is exactly why it was not bundled into the sweep PR. This is the single highest-value follow-up from the sweep. An interim hardening that needs no layout change: make `RemoveMember` **fail** when a limit naming the removed address was not passed to its execution — turning "prune what you were given" into "require what you need" — at the cost of making removal executions carry every affected limit.

---

## F-2 (zero-field envelope fail-open) — shipped fix is right, but it lives one layer too high

**Shipped:** a dispatch-time `IntoDiscriminator` gate that, for zero-field instruction contracts recorded in the manifest, rejects a missing version byte (`InvalidInstructionData`) and a future version (`InvalidMigrationVersion`).

**The alternatives, honestly weighed:**

1. _Generate a `try_from_bytes` call per dispatch arm._ Correct but strictly worse on cost: payload instructions would parse twice (the handler parses again), and the macro cannot know which handlers skip parsing — which is the whole problem.
2. _Fold the version into the dispatch key_ — match on `[discriminator, version]` as one unit so "wrong version" is simply "no matching arm." This is the _semantically_ correct home: the version byte is part of the instruction's identity, not a post-hoc validity check. The gate approximates this with an extra const-table lookup because the dispatch matchers today are width-typed on the discriminator alone.
3. _Gate every enveloped instruction_ (the first shipped shape). Removed: it taxed all 83 benchmarked instructions to close a hole that only zero-field instructions have.

The gate's residual cost (3–9 CU on 38 zero-field instructions, recorded as approved totals) is the honest minimum for option 2's semantics until the discriminator grammar can carry the version. A cheaper _equivalent_ worth considering: for a contract whose entire history is `version 0` — which is every zero-field instruction today — the check is a constant compare against a literal `0`; the codegen could specialize the single-version case and drop the table lookup.

**What would make this a non-issue:** the ledger already knows each instruction's current version at build time. Emitting the version check into the same expression that produces the arm (option 2) is the end state; the gate is a defensible bridge.

---

## N2 (mid-list optional slot-shift) — the compile error is correct; the sentinel is the deeper defect

**Shipped:** compile-time rejection of a positional field after an optional field.

**Why the obvious alternatives fail:** a runtime check cannot detect the attack — the shifted list is indistinguishable from an honest filler list at the same slot count (that _is_ the exploit); silently reordering fields in the macro would change wire order under the author; changing the None-sentinel scheme is wire-breaking. So a compile error is the only local fix, and it is the right one — the ordering rule already existed for `remaining` slices.

**The deeper defect is the sentinel itself.** `next_opt` treats _the program's own address_ as "absent." That overloading is what makes filler-dropping invisible: an honest `None` and an attacker's shifted binding produce identical parse results. The durable design is an explicit length discipline — either count-prefixed optional regions or a dedicated filler identity that cannot collide with a real account the program might legitimately want to pass (a program-owned PDA is off-curve so it can never collide today, but only by accident of curve math, not by contract). If the wire ever gets a breaking window, replacing the sentinel with an explicit optional-count is the change that retires this class rather than fencing it.

A friendlier migration than the hard break: accept mid-list optionals that carry an explicit `#[pina(filler)]`-style opt-in, so a program with a genuine mid-list optional documents the hazard instead of being forced to reorder — the same shape as `distinct = false` for remaining slices.

---

## N1 (PDA seed order) — shipped fix chose the _user expectation_ over the _deployed reality_; that is right pre-1.0, and only pre-1.0

**Shipped:** codegen emits seed slices in declaration order, matching the IDL.

**The alternative nobody should dismiss:** change the _IDL renderer_ to publish constants-first, matching the old codegen. That keeps every already-derived address stable — for any program already deployed with interleaved seeds, the shipped fix silently changes the addresses clients derive. Pre-1.0 with no mainnet deployments, expectation wins and the fix is correct; the moment downstream deployments exist, this exact fix becomes the dangerous option and the renderer-side fix becomes the only safe one. The changeset marks it breaking, which is the load-bearing mitigation — but it is worth a line in the migration guide saying plainly: _interleaved-seed programs redeploying across this change derive different PDA addresses._

---

## ECON-F1 (staking unbacked deposit) — shipped fix is the house idiom, and the guard stack makes it sound

**Shipped:** `Deposit` runs `TransferChecked` from the depositor's ATA into the pool's stake vault and credits the **observed** vault delta (before/after balance read), not the instruction argument. `assert_no_extensions` on the stake mint excludes fee-on-transfer mints, so observed-delta equals requested in every supported configuration — the delta read is defense against a future mint-policy change, not a live case.

The stack is exactly the escrow-pinned pattern (`require_post_cpi_balance_reload` enforces it mechanically), the derived-vault check closes the "deposit into an account I still control" variant, and the surfpool suite pins `stake_vault == total_staked` as an invariant. I do not see a materially better fix at this scope. The one _structural_ improvement: a Kani or fuzz harness over the invariant "ledger total ≤ vault balance in all reachable states" would generalize beyond the tested paths — the repo's Kani infrastructure is the right home.

---

## RC-1 (duplicate-writable blind spot) — deliberately not "fixed," and the reasoning matters

**What happened:** the natural fix — reject any two same-address slots where either is mutable-declared and both are writable-in-instruction — false-positives on the legitimate _creator-pays-own-rent_ idiom (the same wallet bound readonly+signer in one slot and mutable+writable in another, which Solana's same-key privilege merging makes indistinguishable from two writable slots). The multisig suite exercises that idiom, so the rule broke nine real tests. It was removed.

**Why no single-layer fix exists:** the runtime cannot see _declarations_ (a `next()` slot carries no mutability intent), and the macro cannot see _runtime privilege merging_ (that the SVM made the readonly-declared slot writable because its twin was). The missing information is exactly the crossing of the two layers — which is why `distinct_from` (declaration-level, opt-in, per-pair) is the only sound guard today, and it shipped with tests.

**The durable fix, if the framework wants one:** a consumed-slot record in the cursor. The macro knows the positional bound; it can emit `cursor.note_consumed()` after each `next()`/`next_opt()`, and the cursor can retain the last `ACCOUNT_BOUND` consumed `(address, writable)` pairs as copied values — no borrow retained, no allocation, `no_std`-clean. `track_mutable_account` then scans both the future slice and the recorded past, flagging an alias where _either_ side was writable. That distinguishes the attack (immutable-declared slot, writable in instruction, aliases a mutable slot) from... nothing, actually — it is the same predicate, and it would still fire on creator-pays-rent. The genuinely distinguishing signal is _declaration intent_, which only `distinct_from` encodes. So the honest conclusion: **the lint is the fix** — flag immutable-typed slots that alias-check nothing and whose seeds do not bind a signer — and the cursor machinery above would be complexity without added precision. Ship the lint.

---

## D2 (escrow vault grief) — shipped fix is the stricter of two sound options

**Shipped:** `Make` tolerates a pre-created vault only when it is the correctly derived ATA, zero balance, no delegate; everything else keeps the old `assert_empty` failure.

**The alternative:** always create the vault with `CreateIdempotent`. Then a pre-existing empty vault is a no-op and a pre-_funded_ one is simply absorbed — the escrow records `amount_a` from the observed delta, so attacker-donated funds do not corrupt accounting. That is one line of code instead of a four-condition tolerance check. The reason the stricter version is still the better template: a vault that can hold un-invited tokens at creation invites the next question ("whose are they when `Take` closes the account?"), and the zero-balance precondition answers it before it is asked. For an example whose job is to teach the pattern, explicit beats minimal.

---

## The tail (E5, D1, F-1, N1-mig, ABI-F1/F2, M4) — correct at their scope; each has an obvious ceiling

- **E5 (zero-address admin)** — rejecting the zero address stops the brick. The real fix is the repo's already-tracked two-phase rotation (propose + accept-by-new-admin-signature), which also stops the mistype-a-real-address brick that the zero-check cannot see.
- **D1 (prop_amm authority)** — reseeding out of the uniform-seed space kills the copy-paste footgun, and the 256-seed regression test pins the class. The ceiling: any committed fixture key is public, so the _example's_ shape is the defect — an authority configured in program state (the role_registry shape) would make the lesson the right one.
- **F-1 (floats)** — handler-level NaN/Inf rejection is a patch per instruction. The ceiling is schema-level: a generated finiteness rule on `f32`/`f64` fields so it cannot be forgotten, which is also the natural lint (`float_finiteness`). The deeper question — whether floats belong in the schema grammar at all, given the repo's fixed-point work — is the real answer.
- **N1-mig (funding cap)** — raising the cap to cover the ladder is arithmetic, now documented at the constant. The ceiling is making the cap _computed_ from the manifest's layout growth at `pina migrations make` time, so the two cannot drift again — the exact drift that stranded the accounts.
- **ABI-F1/F2** — decode-time validation is unambiguously correct; nothing to improve.
- **M4 (pre-zero)** — defense-in-depth as intended; the ceiling is the static companion (Kani or lint proving every added destination byte is written by its transition), which turns the defense-in-depth into an enforced contract.

---

## What I would build, in order

1. **Membership generation in the multisig** (E2 + E3 + the re-add residual, one mechanism; also unfreezes unaffected proposals).
2. **Version-in-the-dispatch-key** for enveloped instructions (retires the gate and its approved CU cost).
3. **`float_finiteness` + schema-level finite rule**, or the decision to drop floats from the grammar.
4. **The RC-1 lint** (declaration-level `distinct_from` guidance) — not the cursor machinery.
5. **Computed migration funding caps** at `migrations make` time.
6. **Two-phase authority rotation** in role_registry (already tracked; the sweep's zero-address test becomes its negative case).
