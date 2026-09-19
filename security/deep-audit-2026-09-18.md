# Deep security audit and production-readiness review

Audit date: 2026-09-18 · Base: `main` @ `f757a669` (workspace 0.18.0, plus the uncommitted Kani-harness tweak to `crates/pina/src/verification/compact.rs`, which is test-only and benign)

Method: four parallel deep reads (core runtime `crates/pina/src/**`, macro codegen `crates/pina_macros/**` including the diffs of #441/#442, host tooling `pina_cli` + renderers + scripts, supply chain/CI/publish), each cross-checked against upstream dependency source where soundness depends on it; independent verification of every Medium+ finding against the actual code; one adversarial Surfpool/SBF probe executed end-to-end (reproduction crate retained at `tmp/vesting-adversarial/`). This document consolidates all of it with file:line evidence.

---

## Verdict

**The on-chain core (`crates/pina`) is production-grade.** No Critical or High findings in the runtime crate. The 2026-03 loader-audit escape-borrow fix is sound (verified against `solana-account-view 2.0.0`'s actual `Ref::try_map` implementation, not just signatures); owner-before-deref ordering holds on every typed path; arithmetic is checked or saturating everywhere it touches value; the compact codec is Kani-proven; `unsafe` is one 5-line, SAFETY-commented zeroing primitive plus test/proof-harness escapes.

**The residual risk sits in four places, in priority order:**

1. A silent upgrade hazard in freshly shipped macro semantics (`with_pda`, #442) that can reintroduce the shadow-account class downstream with zero compiler signal.
2. Host-tooling supply-chain gaps: unverified lint-driver binary downloads, generated-Rust injection from malicious IDL names, and an unvalidated `[lib] name` traversal.
3. Value-bearing **example templates with broken economics** — empirically proven: the vesting template strands 100% of deposited funds, and the staking template's reward accounting is a repeatable no-op that becomes an infinite-draw bug the moment someone "completes" it.
4. Process hygiene: an advisory-ignore list with no expiry, dead `cargo-deny` advisories config, and a personal-Gmail-only disclosure channel.

SECURITY.md already says the framework is pre-1.0 and unaudited; this audit substantiates that the core is ahead of that disclaimer while the items below keep the "not yet production-hardened" label honest for the tooling and templates.

---

## Findings

| ID  | Severity                 | Area                               | Summary                                                                                                                                                                                                                                                                                                                                                                                                                              |
| --- | ------------------------ | ---------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| H1  | High (template)          | `examples/vesting_program`         | Vesting template strands 100% of funds: `Claim` moves no tokens, the schedule is never enforced against the Clock, `Cancel` refunds nothing. **Empirically proven on SBF.**                                                                                                                                                                                                                                                          |
| H2  | High (template)          | `examples/staking_rewards_program` | Reward economics are vestigial: `reward_index` is never settable, `pending_rewards` accrual is repeatable with no checkpoint, no payout transfer exists. A completed-in-good-faith copy is an infinite-draw bug.                                                                                                                                                                                                                     |
| M1  | Medium-High (downstream) | `pina_macros` #442                 | `with_pda` silently weakened from canonical-bump verification to stored-bump-only across a breaking release; same name, same signature, no deprecation, and the IDL cannot express the difference.                                                                                                                                                                                                                                   |
| M2  | Medium                   | `pina_cpi_renderer`                | Raw IDL-derived names interpolated into single-line `///` doc comments — a `\n` in a name injects arbitrary Rust into the generated crate.                                                                                                                                                                                                                                                                                           |
| M3  | Medium                   | `pina_cli`                         | Unvalidated Cargo `[lib] name` flows into output paths (`idl/`, client crate dirs, `target/deploy/`) — `../../` in a manifest traverses outside the project on `pina generate`/`pina idl`/`pina keys`.                                                                                                                                                                                                                               |
| M4  | Medium                   | `pina_cli` lint driver             | Prebuilt driver binary downloaded over HTTPS with no checksum/attestation verification, no size cap, env-overridable origin, then installed 0755 and executed.                                                                                                                                                                                                                                                                       |
| L1  | Low                      | `pina_macros`                      | A 16-seed `#[pda(..., bump)]` declaration compiles but can never load at runtime (17 seeds with the bump always exceed `MAX_SEEDS`). Fail-closed availability bug; should be a compile error.                                                                                                                                                                                                                                        |
| L2  | Low                      | `pina_macros`                      | Generated PDA signatures emit unqualified `Address`; a `#[pda]`-only struct has no identity-proof guard (schema structs do). Worst case is a self-inflicted compile error, but the guard should cover both.                                                                                                                                                                                                                          |
| L3  | Low                      | `pina`                             | `emit` allocates `[0u8; Self::SIZE]` on the SBF stack with no compile-time budget assert (the migration workspace has one at `migration.rs:384-391`). Self-DoS.                                                                                                                                                                                                                                                                      |
| L4  | Low (documented)         | `pina`                             | `close_with_recipient` leaves stale bytes until end-of-instruction; same-transaction revival requires the user to pick `close_account_zeroed` (lesson 09 covers it; residual risk is misuse).                                                                                                                                                                                                                                        |
| L5  | Low                      | `pina`                             | `HasDiscriminator::write_discriminator` silently no-ops on an undersized buffer (`debug_assert` only) — foot-gun for manual trait implementers.                                                                                                                                                                                                                                                                                      |
| L6  | Low                      | `pina_macros`                      | Migrate prelude always matches the 1-byte `0xFF` trigger while `#[discriminator]` reserves the width-matched all-ones value for `u16`+ enums; a width-matched reserved instruction is rejected instead of routed. Fail-closed.                                                                                                                                                                                                       |
| L7  | Low                      | CI                                 | `security:audit` carries a 7-entry advisory ignore list with no expiry/review mechanism; `cargo deny` never runs the `advisories` check (deny.toml's section is dead config); the two tools' ignore lists already differ.                                                                                                                                                                                                            |
| L8  | Low                      | CI/publish                         | `publish.yml` dispatch trusts a caller-supplied `checkout_ref` behind `id-token:write`, gated only by the unverifiable `publisher` environment; publisher job runs `pnpm install` without `--ignore-scripts` (PR CI uses it).                                                                                                                                                                                                        |
| L9  | Low                      | process                            | SECURITY.md's only private disclosure channel is a personal Gmail address; no GitHub Security Advisories / private vulnerability reporting referenced.                                                                                                                                                                                                                                                                               |
| L10 | Low                      | examples                           | Escrow: `amount_b == 0` offers are accepted (maker self-harm) and `maker` writability is only implicitly enforced by the runtime on Take's closes. `role_registry`: one-step `RotateAdmin` (already flagged in `incidents-2023-2026.md` as blocking the two-phase-rotation lint).                                                                                                                                                    |
| L11 | Low                      | hygiene                            | **Corrected — see the correction note below.** The advisory-affected versions in the root lockfile are _reachable_ (via the dev/test stack), not orphans; there is nothing to prune. The surviving items are real and low risk: unused `tar` dependency, orphan `require_empty_before_init.rs` lint unregistered, an 8 MiB limit whose error text says "4 MiB", `pina docs` topic not basename-sanitized, no `packageManager` field. |

No Critical findings anywhere. No High findings in shipped framework code (`crates/*`).

---

## Detailed findings and fixes

### H1 — Vesting template strands all funds (proven)

**Evidence (static):** `examples/vesting_program/src/lib.rs`

- `Claim` (`:230-301`): validates accounts, enforces only `claimed + amount ≤ total_amount` (`:279-284`), writes `claimed_amount` (`:286-288`), then merely creates the beneficiary ATA (`:290-298`). **No token transfer anywhere.** The stored `start_ts`/`cliff_ts`/`end_ts` are never read after `Initialize`; no Clock sysvar is consulted, so even the intended schedule gate does not exist.
- `Cancel` (`:304-349`): sets `cancelled = true` and returns. No refund to the admin, no vault close.
- The shipped Surfpool suite asserts only the state fields (`tests/surfpool/src/lib.rs:200-218`) and never funds the vault, which is why the gap is invisible to CI.

**Evidence (dynamic):** `tmp/vesting-adversarial/` (kept as reproduction) funds the vault with 1,000,000,000 real SPL tokens and asserts balances around each instruction, against the SBF artifact. Result (test passes, 2026-09-18):

- `Claim(400_000_000)` succeeds; `claimed_amount` → 400M; beneficiary ATA balance stays **0**; vault stays at the full 1B.
- A second `Claim` advances the counter to 800M — still zero tokens moved.
- `Cancel` succeeds; vault still holds the entire allocation; admin ATA unchanged.

**Why it matters:** examples are the templates downstream users copy. A team that copies this program and adds the "obviously missing" transfer inherits two latent bugs: claim-before-cliff (no Clock check exists to copy) and a bounded-but-meaningless accounting model.

**Fix:** implement the economics — on `Claim`, enforce `now ≥ cliff_ts` (and pro-rata vesting between cliff and end if intended) via the Clock sysvar, then `TransferChecked` from vault to beneficiary ATA; on `Cancel`, return the unclaimed vault balance to the admin and close the vault. Add balance assertions to the Surfpool suite (the adversarial probe is a ready-made template for them).

### H2 — Staking reward accounting is a repeatable no-op

**Evidence:** `examples/staking_rewards_program/src/lib.rs`

- `reward_index` is written exactly once, to `0`, at pool init (`:251`); no instruction ever updates it.
- `Claim` (`:503-511`) does `pending_rewards += reward_index` with **no drip checkpoint** (no `last_claim_slot`, no elapsed-time term), then creates an ATA and returns — no transfer.
- The pool/position creation comments (`:227-313`) document canonical-bump singleton reasoning well, but the reward path itself is a counter that counts nothing, repeatedly.

**Fix:** either complete the design (authority drip instruction updating `reward_index`; per-position `last_accrued_index` checkpoint; payout transfer; Surfpool balance assertions) or strip the reward fields entirely and rename the example to what it actually demonstrates (staking deposits/withdrawals). A half-implemented value path is the most dangerous kind of template.

### M1 — `with_pda` silent weakening (#442)

**Evidence:**

- `crates/pina_macros/src/pda.rs:255-352`: post-#442 `with_pda` derives the address once from the account's **stored** bump (`:304-311`); `with_checked_pda` (`:334-344`) keeps the canonical search. Pre-#442 `with_pda` did the canonical search (confirmed via `git show acf6061d^`).
- The method keeps its name and signature, so a downstream program upgrading across this release gets **no compiler signal** while losing canonical-bump verification at every existing call site.
- The changeset (`.changeset/compact-with-checked-pda.md`) is candid: it documents the exact shadow-pool/shadow-singleton exploit chains that canonical checking prevents (staking duplicate positions doubling flat reward accrual; `pina_bpf_program` unlimited shadow states) and the creator-gated cases deemed safe. Defense-in-depth on the creation side (`CreateCompactProgramAccount*` reject non-canonical bumps) is what keeps this at Medium rather than High.
- `crates/pina_cli/src/parse/validation.rs:647-653` maps both `with_pda` and `with_checked_pda` to the same IDL property `is_pda = true` — the published ABI cannot tell a canonical-verified account from a stored-bump one, so downstream auditors reading the IDL can't see the difference either.
- Note `load_pda`/`load_pda_mut` (`pda.rs:195-254`) and `assert_seeds` (`:161-194`) have always been stored-bump-only — same caveat class, documented.

**Fix (pick all):**

1. `#[deprecated]`-style rename cycle for the weak variant (e.g. `with_stored_bump_pda`) in the next breaking window, or at minimum a generated doc banner. The changeset says "the change is a rename for that case" — make the compiler enforce that sentence.
2. Extend the IDL schema so `is_pda` gains a `bumpVerification: "canonical" | "stored"` (or a second boolean) distinguishing the two loaders; `apply_assertion` already receives the method name.
3. Add a `pina_lints` rule flagging `with_pda` in handlers whose seeds do not bind a required signer (the creator-gated safety argument), which is exactly the lexical shape of the three examples the changeset had to fix.

### M2 — Generated-Rust injection via IDL names

**Evidence:** `crates/pina_cpi_renderer/src/render/instructions.rs:350` and `:416-419` interpolate `account.name` / `argument.name` raw into a single `///` line. `render_doc` (`render/helpers.rs:97-105`) is the safe path — it splits on `\n` and re-prefixes every line — but these two sites bypass it. `codama-nodes 0.13.2`'s name types deserialize plain strings without normalizing, so `pina cpi --idl evil.json` keeps embedded newlines. The `validate_generated_sources` syn parse only proves syntactic validity, which an attacker satisfies by closing and reopening the struct inside the injected text. Identifiers, field names, and file names are safe (heck normalization + `rust_identifier`).

**Attack:** malicious IDL author → victim runs `pina cpi --idl` → arbitrary Rust items in the generated crate, executing under the victim's `cargo test` (`#[cfg(test)]` payloads) or hanging builds via const-eval, inside code that looks generated-and-trusted.

**Fix:** route both sites through the newline-splitting helper (one line each), and add a fixture test with a newline-bearing name asserting the emitted docs are single-line.

### M3 — Path traversal via `[lib] name`

**Evidence:** `crates/pina_cli/src/project.rs:729-750` (`library_details`) returns `target.name` from `cargo metadata` verbatim — cargo itself imposes no charset (verified empirically: `[lib] name = "../../pwned"` round-trips through `cargo metadata --no-deps`). `pina.toml` paths are strictly validated (`:547-707`), but this Cargo-derived name is not. Sinks: `crates/pina_cli/src/codama.rs:610-616` (`idl/../../pwned.json` via `write_idl_atomic` — atomic-replace truncates whatever exists at the traversal target), `:625/:644/:690` (client crate dirs), and `project.rs:360-372` + `build.rs:243-281` + `deploy.rs:611-629` (`target/deploy/` artifact/keypair paths). `validate_render_target` uses `std::path::absolute`, which does not collapse `..`.

**Attack:** malicious repo author; victim runs `pina generate` / `pina idl --path .` / `pina keys` inside the clone. Marginal over the baseline "cloned repo = build-script RCE" only for the commands that don't compile — but those are exactly the ones users consider read-only.

**Fix:** validate `library_name` once in `library_details` against `[A-Za-z0-9_-]+` (cargo's own lib naming rules), closing every sink.

### M4 — Lint-driver download integrity gap

**Evidence:** `crates/pina_cli/src/lint_driver.rs:549-614`: `fetch` has a 120 s timeout but no size cap (`read_to_vec` unbounded) and no digest verification; `install` writes 0755 and `probe_output` executes the binary as its version check. Release assets carry published sha256/sha512 checksums and OIDC provenance attestations (`publish.yml:176,218-290`) — none verified client-side. `PINA_LINT_DRIVER_BASE_URL/REPO/RELEASE` env overrides steer the origin. Corroborated independently by the CI audit (its F3).

**Fix:** embed a per-release sha256 (or fetch and verify the published checksum over the same trusted channel), add `take(256 MiB)` to the body read, set `https_only(true)` on the agent, and document the env overrides as operator-only.

### L7/L11 — Advisory tooling drift and lockfile hygiene (details)

`devenv.nix:1271` runs `cargo-deny check bans licenses sources` — advisories omitted, so `deny.toml:4-21` never executes; `security:audit` (`devenv.nix:1276-1301`) runs cargo-audit with a different 7-ID ignore list. Meanwhile the root lock pins orphaned vulnerable versions (nothing references them — verified by lockfile parse; Actions: run `cargo-deny check advisories` with `--workspace` (the default graph excludes the dev/test stack entirely, so the check must be scoped to see these), give every audit ignore an expiry issue, and drop the unused `tar` dependency. **Do not prune lock entries** — see the correction note.

---

## What was verified sound (highlights, so the next audit doesn't re-derive it)

- **Loader lifetimes**: every typed view is guard-backed (`Ref::try_map`/`RefMut::try_map` preserve the borrow-state transfer — verified in `solana-account-view 2.0.0` source); compact loaders are closure-scoped; token loaders wrap upstream guard-preserving parsers; no borrow crosses a resize/migrate effect (`cpi.rs:1647-1695`, `migration.rs:671-674,946-977`).
- **Check ordering**: owner → length → discriminator → validation, before any typed deref, on every generated and hand-written loader path (`impls.rs:152-212,584-640`).
- **Entrypoint dispatch (#441)**: `program_id` checked before parsing; short data rejected by exact-width slice; unknown discriminators rejected; discriminators are enum-width, not 8-byte Anchor hashes; duplicate values are a compile error (though unpinned by a UI test — see below).
- **Arithmetic/value movement**: `checked_send_balances`/`checked_close_balance` Kani-proven conservative-or-reject; realloc plans computed before any lamport movement with the 10 KiB single-growth cap enforced pre-CPI and the cumulative-growth residual honestly documented; `MAX_PERMITTED_DATA_INCREASE` pinned by test.
- **Schema grammar**: closed by construction (generics, manual `ZcField`s, non-literal array lengths all rejected); `SIZE == size_of::<Zc>() == discriminator + Σ fields` asserted at compile time; `fixed` exact-pinned.
- **Migrations**: envelope classification Kani-proven; manifest reads recompute per-version sha256 and physical layout (fail-closed); ledger hash-chained; transitions hash-pinned and TODO-scanned at compile time.
- **Host FS/keys**: cap-std capability sandbox on every CPI-renderer write; heck-normalized names + symlink sweeps in the codama renderer; keypairs via `getrandom`, 0600 atomic writes, permission/symlink/TOCTOU checks, refusal to generate on Windows; RPC URLs hardened (no userinfo, loopback-only HTTP); subprocesses argv-only; npx packages version-pinned with an anti-hijack test.
- **CI**: all actions SHA-pinned, `persist-credentials: false` everywhere, no `pull_request_target`, zizmor + gitleaks + pnpm-audit gating, OIDC crates.io/npm publishing with provenance, pre-tag publish dry-run. Reachable shipped-graph dependencies are advisory-clean (`cargo deny` advisories pass; ureq 3.4.1, rustls 0.23.45 — exactly the RUSTSEC-2026-0285 patched release — tar 0.4.46, webpki 0.103.15 all patched).

---

## Adversarial testing performed

1. **Vesting fund-stranding probe (SBF, executed)** — `tmp/vesting-adversarial/`: built `vesting_program.so` (`cargo build-sbf --features bpf-entrypoint`), deployed via `pina_test::ProgramTest`, funded the vault through a real SPL mint, and asserted balances around Claim/Claim/Cancel. All assertions held → H1 proven, not hypothesized.
2. **Recon of existing adversarial coverage**: `crates/pina/tests/adversarial_invariants.rs` (1,055 lines) already pins duplicate-mutable rejection, non-canonical-bump explicitness, token-loader owner spoofing, ATA address/owner/wallet/mint spoofing, overflow-preserving transfers, and exact-size errors; `compact_account.rs:595-672` pins the `with_pda`/`with_checked_pda` split behavior; Kani covers discriminator/compact/arithmetic/CPI proofs; `pina_fuzz` runs three 30 s smoke targets in CI. The classic sealevel-attacks probes are covered — the examples' economics (H1/H2) were the uncovered gap.
3. **Dependency adversarial check**: `cargo audit` (7 stale-entry hits, all unreachable orphans), `cargo deny check` (advisories/bans/licenses/sources all ok), lockfile parse proving zero dependents on the vulnerable versions.

---

## Top-exploit-class mapping

Solana framework classes (sealevel-attacks / Neodyme / 2023-2026 incident research already in-repo):

| Class                                                            | Status                                                                                                                                                                       |
| ---------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 00 Signer authorization                                          | Covered (lesson + lints + assert_signer ordering verified)                                                                                                                   |
| 01 Account data matching                                         | Covered (lesson; address equality on decoded fields in examples)                                                                                                             |
| 02 Owner checks                                                  | Covered; owner-before-deref verified on all typed paths                                                                                                                      |
| 03 Type cosplay                                                  | Covered (guard-backed typed loading; discriminator + length + owner chain)                                                                                                   |
| 04 Initialization / init front-running                           | Covered (PDA creation only via owning program; `assert_empty` before create; system-account creation validated)                                                              |
| 05 Arbitrary CPI                                                 | Covered (`assert_program`/`Program::try_new` address+executable checks; allowlisted SPL program IDs in examples)                                                             |
| 06 Duplicate mutable accounts                                    | Covered (pairwise rejection + `remaining_mut_distinct` default; runtime-tested)                                                                                              |
| 07 Bump canonicalization                                         | Covered for creation + compact loaders; **residual**: stored-bump-only `with_pda`/`load_pda`/`assert_seeds` with no lint/IDL distinction (M1)                                |
| 08 PDA sharing                                                   | Covered (namespaced seeds; singleton-via-canonical-creation documented in staking)                                                                                           |
| 09 Closing accounts                                              | Covered (`close_account_zeroed`, zero-before-close; residual user-misuse path L4)                                                                                            |
| 10 Sysvar address checking                                       | Covered (`assert_sysvar` validates address)                                                                                                                                  |
| 11 Admin key compromise containment                              | Lesson exists; **example gap**: `role_registry` still one-step rotation (L10, known)                                                                                         |
| 12 Oracle integrity                                              | Lesson exists (prop_amm = pinned authority demo); oracle-staleness lint still future work (known)                                                                            |
| Arithmetic overflow (incl. shifts)                               | `require_checked_asset_arithmetic` denies + checked math in framework; Cetus/Balancer classes mapped in incidents doc                                                        |
| Rounding-direction (Balancer class)                              | No AMM value math exists in-framework or in examples — nothing to get wrong yet, but nothing tested either; if an AMM example lands, add directional-rounding property tests |
| Guarded drains (Drift/Raydium class)                             | `require_guarded_full_balance_drain` warn-lint + lesson 11                                                                                                                   |
| Rust-side: codegen injection, path traversal, unsigned downloads | **Open: M2, M3, M4**                                                                                                                                                         |
| Supply chain (web3.js class)                                     | Strong (SHA-pinned actions, OIDC, provenance); **client-side gap M4**                                                                                                        |

---

---

## Correction (2026-09-18, found while implementing the fixes)

**The L11 "orphan lockfile entries" claim in the first version of this audit was wrong.** It said the advisory-affected versions in the root `Cargo.lock` had zero dependents and could be deleted. They are genuinely reachable, and the check that "verified" otherwise was vacuous.

What went wrong: the verification used `rg -n '<crate> v<version>' Cargo.lock`. Cargo.lock dependency specs are written `"curve25519-dalek 3.2.0"` — _without_ a `v` prefix — so that pattern could never match anything, and "no matches" was misread as "no dependents". Measured correctly:

```
$ rg -q '"curve25519-dalek 3\.2\.0"' Cargo.lock && echo REACHABLE
REACHABLE
```

`cargo tree -i` confirms the chain through `pina_test` → Surfpool/litesvm/agave, and deleting the blocks makes `cargo metadata --locked` fail outright. The implementing agent caught this, restored the lockfile, and proved the reachability independently; I reproduced both the vacuous pattern and the real reachability before writing this note.

Consequences for the recommendations:

- **Do not delete lock entries.** There are no orphans to prune; the entries exist because the dev/test stack uses them.
- The real fix for the advisory gate is scoping, not pruning: `cargo-deny`'s default dependency graph contains roughly a hundred crates and excludes the dev/test stack, so `cargo-deny check advisories` as written would have reported nothing regardless of the ignore list. The working configuration uses `--workspace` for the advisories check, which surfaces all seven findings and makes the ignore list load-bearing (verified by removing one ignore and watching the gate fail).
- The lesson recorded here is about verification method, not about these specific crates: a negative grep result is only evidence when the pattern is proven to match the positive case.

## Recommended remediation order

1. **M3** (one validator in `library_details`) and **M2** (two `render_doc` routings) — tiny diffs, close host-tool attack surface now.
2. **H1/H2** — fix or gut the vesting/staking economics; keep the balance-assertion pattern from the probe in their Surfpool suites. Templates are the product's front door.
3. **M1** — decide the migration story before the next release cuts: deprecation/rename of weak `with_pda`, IDL `bumpVerification`, and the seeds-don't-bind-a-signer lint. Cheapest while #442 is still fresh.
4. **M4** — checksum verification + size cap + `https_only` on the driver fetch; document env overrides.
5. **L7/L11** — align deny/audit advisory tooling (scope the advisories check with `--workspace`), expiry-tag the ignores, drop the unused `tar`. Do **not** prune lock entries — see the correction note.
6. **L1/L2/L3/L6** — small codegen/runtime hardening PRs (15-seed cap when `bump` declared; qualify `Address` or add the identity proof to `#[pda]` structs; `emit` stack-budget assert; width-matched migrate prelude).
7. **L9** — enable GitHub private vulnerability reporting alongside the Gmail channel.
8. Carry the known items already tracked in `production-readiness-work-orders.md` (WO1 error distinguishability, WO5 driver distribution, two-phase rotation example) — this audit independently confirms their priority.
