# Remediation plan — deep security audit 2026-09-18

Companion to [`deep-audit-2026-09-18.md`](./deep-audit-2026-09-18.md). Written to be handed to an implementing agent item by item. Base: `main` @ `f757a669`.

---

## Working agreement for the implementing agent (read first)

1. **Line numbers drift.** Every citation below was verified on 2026-09-18; before editing, re-locate the code by the quoted snippet, not only the line number. If the snippet cannot be found, stop that item and report — do not improvise.
2. **One item = one PR** with a Conventional Commit title (e.g. `fix(pina_cli): validate the cargo library name before path joins`) and a changeset (`.changeset/*.md`) for anything user-facing. Never touch a `chore(release)` PR.
3. All commands run inside `devenv shell`. Format with `fix:format` (dprint), never bare `rustfmt`.
4. Before pushing any PR: `lint:all`, and `coverage:all` when Rust changed (100% patch-coverage gate — every added/changed line needs a test in the same PR). If you add an example instruction, exercise it through that example's ignored `tests/surfpool` suite (`ProgramTest::send*`). Performance-sensitive host-operation changes need a case in `crates/pina/tests/benchmarks.rs`; new CLI hot paths need a `scripts/benchmark-cli.ts` `COMMANDS` entry. Expect the consolidated benchmark comment before merge; any CU/size/CLI-time increase is negative.
5. UI/trybuild pins: `TRYBUILD=overwrite cargo test -p pina_root --test ui` to refresh stderr pins; `MACROTEST=overwrite` (pina_root `tests/expand`) for expansion snapshots. Regenerate `codama/idls` + clients when a public program surface changes and commit the regenerated output.
6. Preserve `no_std` for on-chain code; `unsafe_code`/`unstable_features` are denied; do not add `unsafe`.
7. Ignore the uncommitted working-tree diff in `crates/pina/src/verification/compact.rs` (owner's Kani-harness tweak) and the scratch crate `tmp/vesting-adversarial/` (audit reproduction; leave as-is).
8. Unverified-in-person citations from the audit are marked **[agent-cited]** — treat as strong hints and re-locate.

Priority order: **M3 → M2 → H1 → H2 → M1 → M4 → L7 → L11 → L1 → L6 → L3 → L2 → L5 → L9 → L8 → L10 → L4**. Items needing an owner decision before work starts are marked **⚠ DECISION**.

---

## M3 — Path traversal via unvalidated Cargo `[lib] name` (Medium)

**Problem.** `crates/pina_cli/src/project.rs`, `library_details` (around `:729-750`) returns `target.name` from `cargo metadata` verbatim. Cargo itself imposes no charset, so `[lib] name = "../../pwned"` round-trips. The name flows unescaped into output paths: `crates/pina_cli/src/codama.rs:610-616` (`plan.idls_dir.join(format!("{example}.json"))` then `write_idl_atomic` — atomic replace truncates whatever exists at the traversal target), `codama.rs:625/:644/:690` (renderer crate dirs), and `project.rs:360-372` (sbf artifact/keypair paths under `target/deploy/`). `validate_render_target` uses `std::path::absolute`, which does not collapse `..`.

**Fix (single choke point).**

1. In `crates/pina_cli/src/project.rs`, add a validator next to `library_details`:

   ```rust
   fn validate_library_name(name: &str) -> Result<(), ProjectError> {
   	if name.is_empty()
   		|| !name
   			.chars()
   			.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
   	{
   		return Err(ProjectError::InvalidLibraryName {
   			name: name.to_string(),
   		});
   	}
   	Ok(())
   }
   ```

2. Add the `InvalidLibraryName` variant to the `ProjectError` enum in the same file (follow the existing variant style — thiserror, doc comment). Reject empty, and anything outside `[A-Za-z0-9_-]` — cargo's own package/library naming rules; this makes `..`, `/`, `\`, NULs, and unicode homoglyphs all impossible at the source.
3. Call it at the top of `library_details` before returning `target.name`, so every downstream consumer (`library_name()`, codama sinks, build/deploy/key paths) is covered by construction. Do **not** fix the individual sinks; a missed sink would silently reintroduce the bug.
4. Tests: unit tests in `project.rs`'s test module — `../../pwned`, `a/b`, `a\\b`, `.hidden` (dot is illegal under the charset — intentional), empty, and a valid `my_program_2` all covered. Add an integration-style test that loads a fixture manifest with a malicious lib name via the project-loading path and asserts the new error (there are existing `project.rs` tests using `cargo metadata` fixtures — follow them).

**Why it resolves it.** Traversal requires a path separator or `..` in the name; the charset excludes both (and everything else that could escape). Validating once at ingestion means every sink — current and future — receives a provably separator-free identifier.

**Obligations.** `cargo test -p pina_cli`, `coverage:all`, changeset (`fix(pina_cli): reject unsafe cargo library names before path joins`). Not a CLI hot path (metadata parse happens once per invocation; three extra microseconds are noise — note that in the PR so benchmark review doesn't stall).

No decisions needed. No opportunity cost: cargo cannot produce a valid lib name outside this charset for compilable projects, so no legitimate user is rejected.

---

## M2 — Generated-Rust injection via raw IDL names (Medium)

**Problem.** `crates/pina_cpi_renderer/src/render/instructions.rs` has two sites that interpolate an IDL-provided name straight into a single-line `///` comment: `render_account` (`:350`, `format!("\t/// CPI account`{name}`.")`) and `render_argument` (`:416-419`). The safe helper `render_doc` (`crates/pina_cpi_renderer/src/render/helpers.rs:97-105`) already exists — it splits on `\n` and re-prefixes every line — but these two sites bypass it. `codama-nodes` deserializes names without normalizing, so a name containing `\n` lands everything after the newline as uncommented Rust source. The generated-source validation (`validate_generated_sources`, `lib.rs:358-369` **[agent-cited]**) is `syn::parse_file` — syntactic only, so an injection that closes and reopens the struct parses cleanly.

**Fix.**

1. Line `:350` — replace with:

   ```rust
   let mut docs = render_doc(&format!("CPI account `{name}`."), 1);
   ```

   (i.e. go through the splitter; it returns the `\t///`-prefixed lines). Match the existing import surface — `render_doc` is `pub(crate)` in the same `render` module tree; adjust the call shape to whatever `render_docs`/`render_doc` signatures actually are after re-reading them.
2. Lines `:416-419` — same treatment:

   ```rust
   rendered.docs = render_doc(
       &format!("Instruction argument `{}`.", argument.name.as_ref()),
       1,
   );
   ```

3. Sweep the crate for any other raw `format!` producing a `///` line from IDL input: `rg -n '"///|/// \{' crates/pina_cpi_renderer/src` — route every hit through `render_doc`. (The audit found only these two; the sweep is to prove it stays that way.)
4. Tests: add a fixture test that builds a `RenderedInstruction` (or a full codama document) with `name = "a\n}\nconst _: () = loop {};\npub struct X"` and asserts the emitted docs contain no line starting with `}` and no `\n` outside a `///` prefix — plus a golden-file test pinning the sanitized output. Place next to the existing renderer tests (`crates/pina_cpi_renderer/tests/` or in-module, follow convention).

**Why it resolves it.** Injection requires the newline to survive into the output positionally outside a comment. `render_doc` re-prefixes every physical line with `///`, so any embedded newline produces an additional comment line, never code. Backticks, `*/`, and CR are inert inside a `///` line for rustc's purposes; also strip `\r` in the same pass if trivial (`doc.split('\n')` leaves `\r` at line ends — harmless in comments, but normalize if the helper doesn't).

**Obligations.** `cargo test -p pina_cpi_renderer`, `coverage:all`, changeset (`fix(pina_cpi_renderer): sanitize idl names in generated doc comments`). Benchmark: renderer change, not a CLI hot path (one-shot generation) — note rationale in PR.

**Optional hardening (owner decision, low value now):** make `validate_generated_sources` also reject any generated line that is not inside an item — impractical; skip. The charset fix at M3 already covers library names; you could additionally heck-normalize the _displayed_ name (`name` shown in the doc), which changes rendered output cosmetically for exotic-but-legal names — **opportunity cost:** golden-file churn. Recommended: leave the display name raw, only neutralize newlines.

---

## H1 — Vesting example strands 100% of funds (High, template)

**Problem.** `examples/vesting_program/src/lib.rs`:

- `Claim` (`:230-301`) enforces only `claimed + amount ≤ total_amount`, writes `claimed_amount`, creates the beneficiary ATA — **no token transfer**. The stored `start_ts`/`cliff_ts`/`end_ts` are never read; there is no Clock check, so no cliff or schedule gate exists at all.
- `Cancel` (`:304-349`) sets `cancelled = true` — no refund, no vault close.
- Empirically proven on SBF by `tmp/vesting-adversarial/` (see audit): claim succeeds, balances do not move, cancel strands the full vault.
- The example's own `readme.md:18,22` already lists "Clock-sysvar validation and cliff or linear-unlock calculations" and token release/cancellation refunds as missing. So this is a **documented gap**, now demonstrated to be a stuck-funds template.

**⚠ DECISION — pick the direction before implementing:**

- **Option A (recommended): implement the economics properly.** The example becomes a correct template; the readme promises become true.
- Option B: delete the example (and its workspace entries, codama clients, benchmarks baselines, surfpool suite). Cheap, but loses the most-copied vesting template and creates IDL/client/benchmark churn.
- Option C: minimal fix only (cliff gate + transfer + refund, no linear unlock). Smallest diff, but the readme says "cliff or linear-unlock" — linear is what downstream users expect of a vesting program; a cliff-only template invites naive completion again.

**Fix under Option A (cliff + linear vesting):**

1. **Claim** — after the existing validations and before mutating state:
   - Require a Clock sysvar account. Add `clock: &'a AccountView` to `ClaimAccounts`, validate with `clock.assert_sysvar()` (see `examples/sysvar_checks_program` and `security/12-oracle-integrity/secure/src/lib.rs:100` for the pattern `sysvars::clock::Clock::from_account_view(self.clock)?`). Alternatively use `Clock::get()` — **prefer the account-view form** because the repo's own lint `require_sysvar_assert_before_sysvar_use` is built around it.
   - Compute vested amount with checked math, rounding **down** in favor of the protocol (Balancer lesson; rounding direction must never favor the claimant):

     ```rust
     let now = clock.unix_timestamp;
     if now < cliff_ts { return Err(VestingError::NothingVested.into()); }
     // elapsed is bounded by end_ts - start_ts at most, since now <= end_ts is
     // not guaranteed by the chain: cap it.
     let elapsed = now.checked_sub(start_ts).unwrap_or(0).min(end_ts.saturating_sub(start_ts));
     let span = end_ts.saturating_sub(start_ts);
     // u128 intermediate, floor division
     let vested = if now >= end_ts {
         total_amount
     } else if span == 0 {
         total_amount
     } else {
         u128::from(total_amount)
             .checked_mul(u128::from(elapsed))
             .and_then(|v| v.checked_div(u128::from(span)))
             .map(|v| v.min(u128::from(u64::MAX)))
             .and_then(|v| u64::try_from(v).ok())
             .ok_or(ProgramError::ArithmeticOverflow)?
     };
     if next_claimed > vested { return Err(VestingError::ClaimTooLarge.into()); }
     ```

     Keep `ClaimTooLarge` for exceeding `total_amount` if you want the distinction, or reuse it — but keep error codes stable where the existing surfpool suite pins them.
   - After `claimed_amount` is updated, **transfer** from vault to beneficiary ATA with `token::instructions::TransferChecked` signed by the vesting PDA, exactly as `examples/escrow_program/src/lib.rs:322-335` does (`seeds.with_bump(bump)` → `to_signer()` → `invoke_signed_with_program`). Mint/decimals handling mirrors the escrow Take path. Update `ClaimAccounts` to include the mint and vault writability you need; the vault must be validated as the ATA of `(vesting_state, mint)` — the checks already exist in the current Claim (`:242-250`), keep them.
   - Ordering discipline (matches escrow Take): all validations → state mutation → CPIs → close. On failure the instruction reverts atomically; keep `?` propagation everywhere.
2. **Cancel** — after `cancelled.set(true)`: transfer the **entire remaining vault balance** (`vault.amount()`) back to the admin's ATA for the mint (`CreateIdempotent` for admin ATA if missing — see escrow Take `:300-308`), then close the vault (`token::instructions::CloseAccount::new(vault, admin, vesting).invoke_signed_with_program(&signers)` — escrow `:338-339`). Keep `cancelled` checked at Claim entry (`AlreadyCancelled`) so cancel-then-claim cannot double-pay.
3. **Schedule validation** — `validate_schedule` (`:155-161`) also rejects `start_ts == end_ts == 0`-style fully-elapsed nonsense only if you want; minimal: keep as-is. Guard `total_amount == 0` at Initialize (currently allowed; a zero-total vesting is harmless once Claim transfers, but reject it for template hygiene).
4. **Tests**:
   - Extend `examples/vesting_program/tests/surfpool/src/lib.rs`: fund the vault (the probe at `tmp/vesting-adversarial/src/lib.rs` is the ready-made pattern — `mint_into` via SPL `MintTo` tag 7), then assert: claim before cliff fails; claim at 50% elapsed transfers exactly the floored amount (vault and ATA balance assertions via the `token_amount` offset-64 helper); over-claim rejected without balance change; claim-after-cancel rejected; cancel refunds remaining balance to admin and closes the vault. The clock: Surfpool cheatcodes may allow time warp (`pina_test` — check `crates/pina_test/src/lib.rs` for a warp/cheatcode passthrough; if none exists, drive schedule boundaries by initializing schedules with past/future timestamps relative to the fixed chain time instead — deterministic either way).
   - Keep fixed `Keypair::new_from_array` seeds per repo benchmark convention.
   - The example's `tests/e2e.rs` (readme mentions it) may need updating to the new account lists.
5. **Surface changes:** new accounts on Claim/Cancel ⇒ regenerate IDL + clients (repo generation flow), changelog via changeset with a breaking note for the example (`feat(vesting_program)!:` … examples are versioned `0.0.0` locally; still changeset it for the docs).

**Why it resolves it.** Every value path becomes: time-gated (Clock), amount-capped (vested formula, floor-rounded against the claimant), and actually settled (transfer + refund + close). The stuck-fund outcome disappears because Cancel now drains the vault and Claim pays out; the repeatable counter becomes harmless because each claim both increments the cap and moves the same amount.

**Opportunity costs.** Linear vesting adds auditable math (u128 intermediate) — that is the cost of being a real template; cliff-only (Option C) avoids the formula but ships a vesting program that can pay 100% at the cliff instant, which downstream users will also copy uncritically. Option A's new accounts also grow Claim's account list (CU + ~50/account — must be visible in the benchmark report and accepted).

---

## H2 — Staking reward accounting is a repeatable no-op (High, template)

**Problem.** `examples/staking_rewards_program/src/lib.rs`: `reward_index` written only at init to `0` (`:251`); no instruction ever updates it; `Claim` (`:503-511`) does `pending_rewards += reward_index` with no per-position checkpoint and no transfer. Today it inflates a meaningless counter; completed naively (add a drip + a transfer) it becomes claim-repeatable infinite rewards, because there is no `last_accrued_index` per position.

**⚠ DECISION:**

- **Option A (recommended): complete the design with global-index accounting.** Pool tracks a monotonically increasing `reward_index` (rewards per staked token, scaled); each position stores `last_index`; claim computes `(index - last_index) * staked` (u128, floor) — the standard rewards-per-token pattern. Needs: an authority drip instruction (or drip-on-deposit), checkpoint on deposit/withdraw too, payout transfer on claim.
- Option B: strip reward fields entirely (`reward_index`, `pending_rewards`, reward mint/vault from the pool) and rename the example to what remains (staking deposits/withdrawals). Less audited surface; but the example is CU-pinned in benchmarks and referenced as the staking template — stripping removes its reason to exist.

**Fix under Option A (sketch — scale the values, e.g. `reward_index` scaled by 1e12):**

1. `PoolState`: document `reward_index` as rewards-per-token × 10^12. Add `SetRewardIndex` instruction (authority-only: `assert_address(&pool.authority)` + signer) taking `new_index: u64` with `new_index >= current` enforced (monotonic; an index that goes backwards would let positions re-claim — this check is the security core).
2. `PositionState`: add `last_index: u64` set to the pool's index at `OpenPosition` (`:326` vicinity).
3. Accrual helper (shared by Claim/Deposit/Withdraw):

   ```rust
   fn accrued(pending: u64, staked: u64, last: u64, current: u64) -> Result<u64, ProgramError> {
   	let delta = current
   		.checked_sub(last)
   		.ok_or(ProgramError::ArithmeticOverflow)?;
   	let extra = u128::from(delta)
   		.checked_mul(u128::from(staked))
   		.ok_or(ProgramError::ArithmeticOverflow)?
   		.checked_div(1_000_000_000_000u128)
   		.ok_or(ProgramError::ArithmeticOverflow)?;
   	u64::try_from(u128::from(pending) + extra).map_err(|_| ProgramError::ArithmeticOverflow)
   }
   ```

   Floor division (against the claimant), u128 intermediate, checked throughout.
4. `Claim`: accrue into `pending_rewards`, then **transfer** `pending_rewards` from the reward vault to the user's reward ATA (`TransferChecked` signed by the pool PDA — the escrow vesting-signer pattern), zeroing `pending_rewards` and setting `last_index = current` in the same instruction. Deposit/Withdraw: accrue and checkpoint `last_index` so moving stake can't replay an old index.
5. Monotonicity test is the adversarial one: attempt `SetRewardIndex` lower than current → rejected; attempt claim twice without index change → second claim pays 0; deposit→drip→withdraw→drip→claim pays only the second window.
6. Tests: extend the example's surfpool suite with SPL balance assertions (fund the reward vault via `MintTo` as in the H1 probe). Every new instruction goes through the ignored surfpool suite (repo rule) and appears in the benchmark report.

**Why it resolves it.** The exploit shape is "stateless accrual" — reward credited per call rather than per index delta. Per-position checkpointing makes accrual a pure function of `(index delta × stake)` that each unit of reward can satisfy exactly once; monotone index prevents replay of a high index after a reset; the actual transfer makes the template's wires (vault, ATAs) do what they visibly exist to do.

**Opportunity cost.** One more instruction (`SetRewardIndex`) ⇒ IDL regen, client regen, surfpool additions, benchmark entries; and a scaling constant future readers must not misinterpret — document `× 10^12` in the schema docs and the readme.

---

## M1 — `with_pda` silent weakening in #442 (Medium-High, downstream)

**Problem.** `crates/pina_macros/src/pda.rs:255-352`: post-#442 `with_pda` verifies only that the account address derives from the **stored** bump (`:304-311`); `with_checked_pda` (`:334-344`) keeps the canonical search. Pre-#442 `with_pda` did the canonical search (verify with `git show acf6061d^:crates/pina_macros/src/pda.rs`). Same name, same signature ⇒ downstream upgraders lose shadow-account rejection with **no compiler signal**. Additionally `crates/pina_cli/src/parse/validation.rs:647-653` maps both methods to the same IDL property (`is_pda = true`), so the published ABI can't express the difference. `load_pda`/`load_pda_mut` (`pda.rs:195-254`) and `assert_seeds` (`:161-194`) are stored-bump-only by long-standing design (documented).

**⚠ DECISION — how hard to cut over (not mutually exclusive):**

- **Option A (recommended, breaking): rename the weak variant and deprecate the old name.** Next breaking window (this is cheapest now while #442 is one release old): `with_pda` → `with_stored_bump_pda` (docs already call it "single derivation"); keep `pub fn with_pda` as a `#[deprecated(note = "renamed to with_stored_bump_pda; with_checked_pda is the trustless loader")]` delegation for one release. Every existing call site now gets a warning that names the semantic change — the compiler signal that is missing today.
- Option B (non-breaking): keep both names, add the IDL distinction + a lint (below). Weaker: warnings in audits rather than at compile time.
- Option C: also gate generation — `#[pda(..., bump = bump, unchecked_load)]` opt-in required to even emit the stored-bump loader. Most opinionated; largest migration cost for programs with creator-gated namespaces (a legitimate use per the changeset).

**Fix parts (do B regardless of A):**

1. **IDL distinction.** `crates/pina_cli/src/parse/validation.rs` — in `apply_assertion` (`:647-653` region), split the arm:

   ```rust
   "assert_canonical_bump" | "with_checked_pda" => {
       props.is_pda = true;
       props.is_canonical_bump = true;   // new field on the props struct
   }
   "assert_seeds" | "assert_seeds_with_bump" | "load_pda" | "load_pda" | "with_pda" => {
       props.is_pda = true;
   }
   ```

   Add `is_canonical_bump` (default `false`) to the account-properties IR, thread it into the IDL emission where `is_pda` is already emitted (codama account node metadata — find the sink with `rg -n "is_pda" crates/pina_cli/src`), regenerate all `codama/idls` + clients, and snapshot-update the affected IDL JSONs. Downstream consumers (indexers, auditors) can then tell a canonical-checked PDA from a stored-bump one without reading Rust.
2. **Lint.** `crates/pina_lints/src/lints/` — new lint `require_checked_pda_for_untrusted_loaders` (warn): fire when a handler calls `with_pda`/`load_pda*`/`assert_seeds`-only on an account whose `#[pda]` seeds do **not** include a field of the required signer (the creator-gated safety argument from the changeset), suggesting `with_checked_pda`. Follow an existing lint's structure (e.g. `require_guarded_full_balance_drain`), add its UI fixture (secure trigger + insecure + `#[allow]` case), register it in `LINT_NAMES` (`crates/pina_lints/src/lints/mod.rs`), and sync `lints.json` count test. Note the orphan `require_empty_before_init.rs` **[agent-cited]** — register or delete it in the same PR while you are in there (separate commit).
3. **Macro test.** Expansion snapshot already shows both methods (`tests/expand/pda.expanded.rs:310-347` region); add a UI fail pin for the 17-seed class is separate (L1) — here just add a compile-pass test that a `#[pda]` struct with `bump = bump` emits both loaders and the deprecated alias (if Option A).
4. **Docs.** The rustdoc on both methods is already honest; under Option A update the changeset (`docs(pda): rename with_pda to with_stored_bump_pda`).

**Why it resolves it.** The vulnerability is the _silent_ semantic change; each layer removes silence for a different audience: rename/deprecation for authors (compile-time), IDL property for consumers/auditors (artifact-level), lint for the creator-gated reasoning itself (pattern-level). The shadow-account attack needs a program that both creates with an unchecked bump and reads with the weak loader while assuming uniqueness — after this, every one of those states is visible somewhere a CI, auditor, or compiler can fail.

**Opportunity costs.** Option A: a second breaking rename wave for programs that already adopted post-#442 `with_pda` deliberately (creator-gated) — mitigated by the deprecated alias for one release. IDL property: all IDLs change ⇒ every downstream client regen shows a diff (cosmetic but noisy); worth a changelog line telling consumers the new key is informational until they choose to gate on it.

---

## M4 — Lint-driver download: no integrity verification, unbounded read (Medium)

**Problem.** `crates/pina_cli/src/lint_driver.rs` — `fetch` (`:549-577`) has a 120 s timeout but no size cap and verifies nothing; `install` (`:585-614`) writes the bytes 0755; `probe_output` executes the binary as its version probe. Origin is overridable via `PINA_LINT_DRIVER_BASE_URL/REPO/RELEASE`. Release assets do ship sha256/sha512 checksums and OIDC provenance (`publish.yml:176,218-290` **[agent-cited]**) — none checked client-side.

**⚠ DECISION — trust root:**

- **Option A (recommended): verify the sibling checksum file over the same HTTPS channel.** Fetch `{asset}.sha256` (name derivable from `driver_asset_name`, `:540-546`) from the same release URL, parse, compare against the sha256 of the downloaded bytes (crate `sha2` is already a workspace dep). Binds the executable to the release's published digest; defeats CDN/CDN-cache corruption, partial origin compromise of the binary path, and stale-cache substitution. It does **not** defend against full release-account compromise (attacker rewrites both files) — see Option B for that.
- Option B (stronger, more moving parts): embed a digest map in the CLI for the pinned-nightly default driver and verify SLSA provenance via `gh attestation verify` equivalent. Cost: the CLI can't embed digests for _negotiated_ per-compiler-commit assets (unknown at CLI build time), so coverage is partial; attestation verification adds a gh-API dependency and network trust of its own. Defer unless the threat model demands it.
- Option C (no network trust): default to `--build-driver` from vendored source, download only on explicit opt-in. Biggest UX regression (WO5 in `production-readiness-work-orders.md` exists precisely because users want the download).

**Fix under Option A:**

1. In `fetch`: add a response-size cap while reading — take a bounded reader, e.g. `let mut limited = response.body_mut().take(MAX_DRIVER_BYTES)` (`const MAX_DRIVER_BYTES: u64 = 256 * 1024 * 1024;` — real drivers are a few MB; 256 MiB is absurd-headroom), read into a `Vec` with capacity checks, error `DriverError::Download { message: "driver artifact exceeded the 256 MiB cap" }` on overflow. `read_to_vec` on the capped reader plus a length check after is fine.
2. Enable HTTPS-only on the agent: in the `Agent::config_builder()` chain add the https-only option (check the ureq 3.4 builder method name — `https_only(true)`/`http_only(false)` — against the vendored source; the workspace features already pull rustls).
3. New `verify_checksum(asset_url, bytes, identity)` step between fetch and install: fetch `{asset_url}.sha256` with the same agent (same cap), parse lowercase-hex (trim whitespace), `Sha256::digest(bytes)` compare constant-time-ish (plain `==` on digests is fine — no secrecy), `DriverError::ChecksumMismatch` on mismatch with a message naming both the URL and the digest seen/expected. Wire it into the `install()` caller so **every** download path verifies (including cache-miss refetches).
4. Tests: unit test the hex parser (mixed case, whitespace, truncated); an integration test with a local `file://`... ureq won't do file URLs — instead factor the verification into a pure `fn verify_bytes(bytes: &[u8], expected: &str)` and unit-test it; for the wiring, a test that `install` is only called from the verifying path (compile-level via private fn signature returning verified bytes — make `fetch` return `VerifiedDriver(Vec<u8>)` a newtype that only `install` accepts, so an unverified path can't typecheck).
5. Document `PINA_LINT_DRIVER_*` env overrides in the CLI docs as operator/testing-only; consider gating the base-URL override behind an env like `PINA_ALLOW_INSECURE_DRIVER_URL=1` (**sub-decision:** adds friction for the WO5 testing story — if the test suite relies on the override, gate it and set the gate in tests).

**Why it resolves it.** The download stops being "execute whatever the URL returns" and becomes "execute exactly the bytes the release digest names", with the memory-DoS and downgrade angles closed by the size cap and https-only. The newtype ensures future edits can't silently reintroduce an unverified path.

**Obligations.** `cargo test -p pina_cli`, `coverage:all`, changeset. CLI cold path (driver install happens once) — no benchmark entry; note in PR.

---

## L7 — Advisory tooling drift (Low)

**Problem.** `devenv.nix:1271` **[agent-cited]** runs `cargo-deny check bans licenses sources` — advisories omitted, so `deny.toml`'s `[advisories]` section (4 commented ignores) is dead config. Separately `security:audit` (`devenv.nix:1276-1301` **[agent-cited]**) ignores 7 advisory IDs with no expiry mechanism. Two lists, two tools, drift already visible.

**Fix.**

1. Add `advisories` to the `cargo-deny` invocation in the `security:deny` task (`devenv.nix` ~`:1266-1275`), so `deny.toml` governs the advisory gate. Then **reconcile the ignore lists**: move every currently-needed ignore into `deny.toml [advisories] ignore` with its existing comment, each annotated with a tracking-issue URL and a review date, and reduce `security:audit`'s ignore list to empty (keep `--deny yanked`).
2. Create one GitHub issue per ignored advisory titled `Review ignore <RUSTSEC-ID>: <crate>` with the reason (dev-stack-only reachability, per the audit table) and a reminder to re-check on the next minor bump of the implicated dep. (Issue titles in title case per repo convention.)
3. Run `devenv shell -- security:deny` and `security:audit` locally and confirm both pass with the consolidated config.

**Why it resolves it.** Single source of truth for advisory policy; every suppression becomes a dated, tracked decision instead of an accreting list; a future advisory that touches shipped code cannot hide in a stale ignore because deny's list is short, commented, and issue-linked.

**Opportunity cost:** none of substance — it is pure hygiene. Only cost is the issue bookkeeping.

---

## L11 — Dependency and tooling hygiene (Low) — **CORRECTED**

**Correction first.** The original version of this item claimed the advisory-affected versions in the root `Cargo.lock` were unreachable orphans that should be deleted. That was **wrong**. The verification pattern (`<crate> v<version>`) could never match Cargo.lock's actual dependency-spec syntax (`"<crate> <version>"`, no `v` prefix), so "no matches" was misread as "no dependents". Measured correctly, all of them are reachable through the dev/test stack (`pina_test` → Surfpool/litesvm/agave), and deleting them makes `cargo metadata --locked` fail. **Do not delete lockfile entries, and do not regenerate the lock** — there is nothing to prune and a regeneration would churn unrelated pins.

**What remains real and worth doing (all small, all independent):**

1. Remove the unused `tar` dependency from `crates/pina_cli/Cargo.toml` (confirm with `rg -n "tar::" crates/pina_cli/src`; only CI shell scripts use system `tar`). If the workspace entry in the root `Cargo.toml` becomes unused too, remove it as well.
2. Fix the misleading error text: `crates/pina_cli/src/idl_metadata.rs` enforces an 8 MiB limit but reports it as a "4 MiB safety limit". Use one named constant for both the check and the message so they cannot diverge.
3. Basename-sanitize the `pina docs` topic lookup in `crates/pina_cli/src/commands.rs` (it joins `{topic}.t.md` under the templates directory, so a topic containing `../` reads outside it). Take only the file-name component, and error clearly when there is none.
4. Add `"packageManager": "pnpm@<version>"` to the root `package.json`, reading the exact version devenv pins, so contributors outside devenv get the same one through Corepack.

**Why it resolves it.** Each item removes a concrete small problem: an unused dependency, a message that misstates the limit a user must satisfy, a read that escapes its directory, and a missing tool version pin. None of them is a correctness or security emergency, which is why the corrected priority of this item is lower than it first appeared.

**Lesson for the next audit:** a negative grep is only evidence when the pattern can match the positive case. Check the pattern against known-present data first.

## L1 — 16-seed `#[pda]` compiles but can never load (Low)

**Problem.** `crates/pina_macros/src/args.rs:167` `MAX_SEEDS: usize = 16`; `:341-350` rejects only `seeds.len() > 16` _before the bump_. Every loader appends the bump: `with_bump`/`to_signer` paths end at `crates/pina/src/cpi.rs:1046/1216`, which reject `seeds.len() >= MAX_SEEDS` (i.e. max 15 seeds pre-bump). So a 16-seed declaration (16 pre-bump, 17 with bump) — and even the 15-seed + bump = 16 case at the signer builder — is dead at runtime while compiling cleanly.

**Fix.**

1. In `parse_pda_seeds` (the function containing the `:341` check — it parses seeds and the caller attaches `bump = <field>`; every `#[pda]` declaration currently requires a bump field for the loaders under discussion — confirm while editing that non-bump `#[pda]` declarations exist or not; if they do, keep 16 for them): tighten the bound to the effective limit, and fix the message to state both numbers:

   ```rust
   let effective_max = SeedType::MAX_SEEDS - 1; // the bump occupies the final slot
   if seeds.len() > effective_max { /* error: "PDA seed list has {} seeds; with the bump seed the \
      derivation would exceed the {}-seed limit; the maximum before the bump is {}" */ }
   ```

2. Check `crates/pina_macros/src/pda.rs` seed-array types (`as_seed_array`/`to_signer` **[agent-cited ~`:421-428`]**) for any parallel constant that must match.
3. Tests: unit test in `args.rs` parsing a 16-seed declaration expecting the compile error message; a `tests/ui/fail/pda_too_many_seeds.rs` trybuild pin (and `TRYBUILD=overwrite` to generate); a pass-case at 15 seeds compiling.

**Why it resolves it.** The failure moves from runtime (instruction permanently bricked with `InvalidSeeds`, discoverable only on-chain) to the macro parse (author sees it before deploy). Fail-closed becomes fail-early.

---

## L6 — Migrate prelude ignores discriminator width (Low)

**Problem.** `crates/pina_macros/src/entrypoint.rs` prelude (`~:451-455`) always emits `pina::is_migrate_instruction(data)`, which matches only `len == 1 && 0xFF` (`crates/pina/src/migration.rs:66-70`). But `#[discriminator]` reserves the all-ones value of the enum's primitive (`u16::MAX` = `[FF FF]`, `u32::MAX` = `[FF FF FF FF]`; constants `MIGRATE_DISCRIMINATOR_U16/U32` exist at `migration.rs:51-55`). For a `u16`/`u32` discriminator program with migrations: `[FF]` is rejected by `parse_instruction` (wrong width), `[FF FF]` is rejected too (reserved variant doesn't parse) — the migrate path is unreachable.

**Fix.**

1. In the entrypoint codegen, thread the discriminator repr through to the prelude (the generator already knows the enum's primitive from `#[discriminator]` parsing — locate where the enum repr is available in `entrypoint.rs`'s generation function). Emit a width-matched check: for `u8` keep `is_migrate_instruction`; for wider reprs generate e.g.

   ```rust
   if data.len() == <N> && data[..<N>] == <MIGRATE_DISCRIMINATOR_U16 as u16>::to_le_bytes() { ... }
   ```

   Simplest robust shape: add to `crates/pina/src/migration.rs` a generic `pub fn is_migrate_instruction_of<T: IntoDiscriminator>(data: &[u8]) -> bool` implemented via `matches_discriminator(&T::MAX-value, data) && data.len() == T::BYTES` — or three narrow functions mirroring the existing one (`is_migrate_instruction_u16`, `_u32`) and select in the macro. Prefer whichever keeps the SBF hot path a `memcmp` (the existing function's comment explains why it stays inline — preserve that property; a slice-eq against an array still compiles to memcmp).
2. Update the doc at `migration.rs:41-46` region that describes the reserved-value/trigger relationship so the two agree for all widths.
3. Tests: unit tests in `migration.rs` for each width's trigger ([FF] vs [FF FF] vs [FF FF FF FF], plus wrong-length tails rejected); an expansion snapshot showing the width-matched prelude for a u16 program (refresh via `MACROTEST=overwrite`); if a `migrations_program` example uses a wider discriminator, extend its surfpool migrate test — it doesn't today (u8), so add the u16 case as a small trybuild/expansion pin instead.

**Why it resolves it.** The trigger and the reservation are currently two different wire patterns; making the trigger the width-matched reservation value means the reserved slot is reachable exactly as documented, for every supported width. Fail-closed both before and after — this is a correctness/uniformity fix, not an exploit closure.

**Changeset:** minor (`fix(pina_macros): match the migrate trigger to the reserved discriminator width`) — note it in the changeset that u16/u32 + migrations programs previously had no working migrate trigger.

---

## L3 — `emit` stack record has no compile-time budget (Low)

**Problem.** `crates/pina_macros/src/event.rs` `emit` generation (`~:192`): `let mut record = [0u8; Self::SIZE];` — `Self::SIZE` is a schema-derived const; a large event struct compiles and then blows the 4 KiB SBF stack at runtime. The migration workspace already has the precedent assert (`crates/pina_macros/src/migration.rs:384-391`, `MAX_MIGRATION_WORKSPACE` **[agent-cited]**).

**Fix.**

1. In the `emit` codegen, after the existing record line, emit a compile-time assertion into the generated impl:

   ```rust
   const _: () = assert!(Self::SIZE <= pina::MAX_EVENT_RECORD_BYTES);
   ```

   with `pub const MAX_EVENT_RECORD_BYTES: usize = 4096 - <reasonable frame allowance, e.g. 512>;` added to `crates/pina/src/event.rs` (document that SBF stack is 4 KiB and the margin covers the call frame). Follow the exact shape of the migration workspace assert for consistency (same file conventions).
2. Tests: a UI fail pin `tests/ui/fail/event_too_large.rs` declaring a `[u8; 8192]`-ish event (pick a field combo that exceeds the budget) expecting the const-assert error; a pass case near (but under) the budget.

**Why it resolves it.** The overflow becomes a compile error proportional to the _declared_ schema instead of a runtime stack probe on-chain. Cost: a new public const (docs + semver note) and one extra const-eval per event type (zero runtime).

---

## L2 — Unqualified `Address` in generated PDA signatures (Low)

**Problem.** `crates/pina_macros/src/pda.rs:156` (`program_id: &Address` in `try_find_pda` params) and `args.rs:179/193/201` **[agent-cited]** emit unqualified `Address` in generated signatures (`tests/expand/pda.expanded.rs:310-347` shows `try_find_pda(authority: &Address, ...)` next to fully-qualified `pina::Address` elsewhere). Schema structs carry an identity proof (`const _: fn(Address) -> pina::Address = |v| v;`, UI-tested); `#[pda]`-only structs don't. A user type named `Address` in scope would shadow the seed-param type — usually a compile error at a typed boundary, worst case a wrong-but-`AsRef<[u8]>` type becomes the seed source.

**Fix.**

1. Qualify every `&Address`/`&'a Address` in pda.rs/args.rs generation with `#crate_path::Address` (the crate-path variable already exists in those generators — match how `:242` writes `#crate_path::create_program_address`).
2. Add the same identity-proof `const` for `#[pda]`-generated impls as schemas have (or extend the existing proof emission to cover the pda module), plus a UI fail pin `pda_shadowed_address.rs` mirroring `tests/ui/fail/account_shadowed_address.rs`.
3. Refresh expansion snapshots (`MACROTEST=overwrite`).

**Why it resolves it.** Generated code stops depending on the user's namespace hygiene; the proof makes any future regression into shadowing a compile error inside the user's crate rather than a silent seed-type substitution.

**Opportunity cost:** snapshots churn (cosmetic diff across `tests/expand/*.expanded.rs`) — expected and acceptable; no runtime change (same types, spelled absolutely).

---

## L5 — `write_discriminator` silently no-ops on short buffers (Low)

**Problem.** `crates/pina/src/traits.rs:500-506`: `debug_assert!` then early `return` — a manual `IntoDiscriminator` implementer passing an undersized buffer gets no write and no error. Generated code always sizes buffers first, so this is unreachable through the macro contract.

**⚠ DECISION — API shape:**

- **Option A (recommended, additive):** add `fn try_write_discriminator(&self, bytes: &mut [u8]) -> Result<(), ProgramError>` to the trait with a default implementation returning `Err(ProgramError::InvalidInstructionData)`-family when short (pick the error in concert with `error.rs` — there is an `InvalidDiscriminator` code; a short buffer is closer to `InvalidInstructionData`/`InvalidAccountData` depending on caller; document the choice), and switch **generated** write sites (`initialize` closures' discriminator writes in macro output) to call the checked variant. Keep `write_discriminator` as-is (deprecated-in-docs) for compatibility.
- Option B (breaking): change `write_discriminator` to return `Result` — cleaner single API, but a trait-method signature change forces every manual implementer to update in a major bump; the foot-gun doesn't merit the break.

**Fix under A:** implement the default in the `primitive_into_discriminator!` macro block (and the derive-generated impls route through it), add unit tests for short/exact/long buffers, add the migration of generated call sites (macro crate) with snapshot refresh. Changeset: minor (pina + pina_macros).

**Why it resolves it.** Manual implementations get a loud, typed failure at the misuse site, and generated code demonstrates the correct pattern; silent divergence between "wrote" and "claimed to write" disappears for anyone using the checked form.

---

## L9 — Disclosure channel is a personal Gmail only (Low, process)

**Problem.** `SECURITY.md:69-96`: private reporting is by personal email only; no GitHub Security Advisories / private vulnerability reporting referenced; email has no intake tracking or SLA beyond good intentions.

**Fix.**

1. Enable GitHub **private vulnerability reporting** on the repo (Settings → Code security and analysis → Private vulnerability reporting) — this is a repo-settings action, not a code change; requires owner.
2. Rewrite `SECURITY.md`'s "Reporting a vulnerability" to lead with the GitHub form (works for researchers without trusting an external mailbox, gives tracking and CVSS tooling), keep the email as a secondary channel, and keep the "no public issues" instruction.
3. Note the supported-versions table stays as-is.

**Why it resolves it.** Reporting moves onto infrastructure with guaranteed intake (notifications to maintainers), no dependency on one person's mailbox deliverability, and structured advisory publication (GHSA IDs) users can subscribe to.

**Opportunity cost:** none meaningful; GitHub's channel requires the repo owner to respond there too — that is the point.

---

## L8 — Publish pipeline hardening (Low)

**Problem.** `publish.yml:20-23,320-324` **[agent-cited]**: `workflow_dispatch` takes `tag` + `checkout_ref`; the publish job (with `id-token: write`, OIDC crates.io/npm trusted publishing) checks out that ref. The `publisher` environment is the only gate, and its protection rules are not verifiable from the repo. Also `publish.yml:350` runs `pnpm install --frozen-lockfile` (no `--ignore-scripts`) inside the publisher environment, while PR CI uses `--ignore-scripts`.

**Fix.**

1. Constrain `checkout_ref`: resolve it to the **git tag** after it exists rather than checking out a caller-chosen ref directly — e.g. resolve `checkout_ref` must equal `refs/tags/{tag}` (the tag job already validates `v*` and dry-runs the exact tree before tagging; reuse that). Implement as a pre-step: `git ls-remote origin refs/tags/${TAG}` → compare SHAs → fail on mismatch; then checkout the SHA, not the symbolic ref. This makes "publish arbitrary branch" require moving the tag, which the release dry-run gate already covers.
2. Add `--ignore-scripts` to the publisher's `pnpm install` (matching `ci.yml:165`). Then run the publish dry-run (`release-pr.yml`'s tag job) locally-equivalent — the repo's pre-tag dry run exercises the same install; if any pinned dep legitimately requires install scripts, the dry run fails and the dep is surfaced for explicit review instead of silently executing (**that is the trade**: occasional legit-script deps now need a conscious decision — record any such dep in the PR).
3. While in the file: document above the `publisher` environment reference what its protection rule must be (required reviewers) so the assumption is at least written down next to the gate that depends on it.

**Why it resolves it.** Publishing becomes reachable only through the already-gated path (tag + dry-run + environment approval) instead of any writer being able to dispatch an arbitrary ref; lifecycle scripts stop executing with publish credentials in scope unless explicitly reviewed.

**Opportunity cost:** tighter `checkout_ref` removes the "re-publish an old tag at a moved branch" convenience — re-publishing should go through a new tag anyway.

---

## L10 — Example nits (Low)

1. **Escrow `amount_b == 0`** (`examples/escrow_program/src/lib.rs`, Make handler): reject zero offers in `MakeInstruction` validation — `if args.amount_b.get() == 0 { return Err(EscrowError::OfferKeyMismatch...) }` — no, add a dedicated `EmptyOffer = 2` variant (documented) rather than mis-mapping an existing code; update the error enum docs and IDL regen. Protects makers from fat-finger free-giveaways. Test: surfpool case asserting the error and unchanged balances.
2. **Escrow `maker` writability** (Take handler, `~:206-344`): `self.maker.assert_writable()?` alongside the existing `assert_address` — Take credits lamports to `maker` (vault close + escrow close). Today a read-only maker fails at runtime with a generic runtime error; an explicit assert produces the framework's clear error instead. One line + a surfpool negative test.
3. **`role_registry` one-step `RotateAdmin`** (`examples/role_registry_program/src/lib.rs:317-329`): upgrade to two-phase rotation — add `pending_admin: Address` (+ optional `effective_slot`/delay) to `RegistryConfig`, split into `ProposeAdmin` (current admin signs) and `AcceptAdmin` (new admin signs); this is prerequisite work the repo's own incident research (`security/incidents-2023-2026.md`, "Two-phase authority rotation" future lint) already calls for. New instruction ⇒ surfpool suite coverage + IDL regen + benchmark entries. **⚠ minor decision:** plain two-phase (accept anytime) vs timelocked (effective after N slots) — timelock mirrors the Drift durable-nonce lesson and gives the future lint its model, at the cost of a Clock sysvar account in the instruction.

---

## L4 — `close_with_recipient` stale bytes (Low, documented — mostly no action)

**Problem.** `crates/pina/src/impls.rs:918-948` region: `CloseAccountWithRecipient` moves lamports and closes but does not zero data; `close_account_zeroed` is the guarded variant. Documented; lesson 09 teaches it.

**⚠ DECISION.**

- **Option A (recommended): keep behavior, add the lint.** Implement `require_close_zeroization_before_close` in `pina_lints` (already proposed in `security/loaders-audit.md:264-270`): warn when a handler calls `close_with_recipient`/`CloseAccount` (non-zeroing) on an account whose data was not previously zeroed (`zeroed()`/`close_account_zeroed`) in the same handler, unless an `#[allow]` is present. Zero CU cost on-chain; catches the misuse class at review time.
- Option B: make `close_with_recipient` zero by default — removes the foot-gun entirely but **adds CU to a hot path** (a data-length-proportional memset on every close) which the benchmark gate treats as a regression, and doubles an API pina already has. Not recommended.

**Fix under A:** follow the M1 lint instructions (UI fixtures, `LINT_NAMES`, lints.json sync), plus apply the lint to all examples/lessons and bless intentional sites.

---

## Post-fix verification (all items)

After each PR and before handoff, from the repo root:

```sh
devenv shell -- lint:all
devenv shell -- coverage:all          # any Rust change
devenv shell -- test:all              # touched crates' suites
devenv shell -- test:idl              # any IDL/client regen
```

Plus, item-specific: surfpool suites (`pina test` per example or the `test:surfpool` task), `TRYBUILD=overwrite`/`MACROTEST=overwrite` only when snapshots are _expected_ to change (never to force a pass), and `cargo audit` + `devenv shell -- security:deny` after L7/L11. Any job you cannot reproduce locally (SBF, Surfpool, Kani tiers) must be named as unverified in the handoff — per repo policy an unproven job is a known failure, not a pass.
