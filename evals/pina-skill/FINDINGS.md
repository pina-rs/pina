# Findings: evaluating the Pina skill against real migration tasks

This is the record of what the evaluation established, including the parts that did not confirm the initial hypothesis. It is written so a later reader can tell which claims are measured and which are not.

## Question

The migrations flow is a differentiator with no equivalent elsewhere in the Solana ecosystem, so the skill has to carry it. The question was whether an agent with the skill can complete realistic migration work — including the parts that are easy to get subtly wrong — and where the skill leaves it guessing.

## Method

Nine scenarios, each a real change to a throwaway Pina program, graded by the real `pina` CLI plus targeted file and transcript assertions. Two skill variants were compared: `baseline` (the skill before this work) and `improved` (after). Round one ran both variants on a tree that also carried an uncommitted validation-separator change; that change is not part of this branch, so the `declarative-validation` scenario and both variants were later reconciled to the single `=` separator the shipped crates accept.

Run with `pnpm exec tsx evals/pina-skill/run.ts --all`; see `README.md`.

## Result

**Round one found no task the baseline skill failed to finish** (both variants passed every migration scenario). What it found was friction, and three outright errors in the skill's text. Round two, with the grader defects fixed, found two tasks where the corrected guidance changes the outcome — see the round-two section below.

## Errors found in the skill (not judgement calls)

1. **The `no_std` idiom was wrong.** `references/project-setup.md` told agents to write `#![cfg_attr(not(test), no_std)]`. Of the 27 example program libraries in this repository, 26 use plain `#![no_std]`; the single exception gates it on `not(feature = "fuzzing")` rather than on `test`. An agent following the skill writes code that does not match the codebase it is being asked to extend.

2. **The manifest format description was stale.** The skill described "manifest format 3" and "format 4" and a "publication-ledger format 3". Those integer counters were retired; both documents now carry a single `abiVersion` string (`"0.20"`), and a pre-`abiVersion` document fails to decode rather than upgrading itself. The skill was describing a scheme the CLI no longer has.

3. **`tests/abi_layout.rs` was undocumented.** `create` generates this committed test file, and `check` fails with "the generated ABI layout test … is stale" whenever it no longer matches the manifest. The skill never mentioned it. An agent that finishes a change on a passing `cargo check` can leave this gate failing and report success — which is exactly what one baseline run did.

## Gaps that cost the agent work but not the task

- **The transition ABI was undocumented.** The skill explained that a manual transition must "fully initialize every active destination byte" without saying what `data` contains or where the offsets in the generated comment point. One run grepped the CLI's own Rust source (`crates/pina_cli/src/ migrations/`) to learn that the printed offsets are payload-relative while `data` includes the discriminator and version header. The fix documents the ABI: `data` includes the header, the printed `32..40` is therefore `data[34..42]`, and `FROM_VERSION`/`TO_VERSION` appear in automatic transitions only.

- **The compact transition shape was undersold.** Compact contracts get a different stub (`target_size`, `working_size`, `migrate`) because the destination length depends on stored values. The skill described this in one clause. It now documents all three functions, that `target_size` must not mutate, and that returning worst-case capacity allocates every stale account at maximum size.

- **`--envelope-ack` was undocumented.** `create` fails closed with `EnvelopeAcknowledgementRequired` when a published contract would gain an envelope — including a brand-new contract of a published kind. One run hit this and recovered by reading the error, but the flag is a gate the skill should predict rather than one the agent discovers.

- **Rent budgeting was under-specified.** Adding one `u64` field to a 42-byte account grows it 8 bytes and quotes ~55,680 lamports; the fixture's 20,000 budget would fail at runtime with `MigrationLamportBudgetExceeded`. The skill mentioned the error and the remedy but not the arithmetic. It now states the ~6,960 lamports per grown byte and that the budget must cover the whole ladder.

## What this means for the skill

The improvements are justified by **errors and friction, not pass rate**. No claim should be made that the improved skill turns a failing task into a passing one, because no such task was found in this suite.

The suite's value is as a regression gate: it exercises the migration surfaces most likely to break (version freezing, disambiguation, manual transitions, compact transitions, process-contract compatibility, envelope acknowledgement, version-width changes) against the real CLI, so a future change to the skill, the CLI, or the macros that breaks one of them fails loudly.

## Limits

- **One model, one run per cell** for most cells. Three repeats on the two scenarios that flipped between runs showed the flip was model variance, not a stable discriminator. Treat single-run pass rates in this suite as noise.

- **Grader bugs are the dominant risk.** Four checks initially failed _correct_ work: two matched the assistant's prose instead of its artifacts (one run was failed for writing "no `TODO(pina-manual-migration)` left"), one demanded a literal offset where a `HEADER_SIZE` constant was equally right, and one demanded numeric literals where named constants were better. Every one was a false negative. `README.md` records the three patterns to avoid.

- **The fixtures are not the repository.** They are small standalone crates using `pina` by path with no SBF build, no Surfpool run, and no on-chain state. The runtime behaviour of a migration is therefore not exercised here; the existing `surfpool` suites and `scripts/migrations-walkthrough.ts` cover that ground.

- **The environment had to be pinned.** The `pina` on `PATH` was 0.17.0, whose subcommands differ (`make`, not `create`). The harness prepends the workspace binary and refuses to fall back to `PATH`. Without that, runs measure the operator's machine rather than the skill.

## Round two: the rest of the skill, and the CLI as a guidance channel

The first round covered migrations only. Round two added scenarios for the other four reference files — project setup, program authoring (PDA creation, account closing), CLI and codegen (IDL regeneration, diagnostics), and testing — plus a deterministic contract test for the new `pina skill` command.

### New findings

- **`pina init`'s identity flow works as taught.** Scaffolded projects ship with a placeholder address and deliberately no `migrations/` directory. The agent under evaluation ran `pina keys new`, bound the fresh identity into `declare_id!`, and only then snapshotted the baseline — the manifest's `programId` matched the source. A first-draft check of ours flagged the snapshot as premature; the agent was right and the check was wrong.

- **Codama renders instruction names in lower case.** An `IncrementInstruction` struct becomes an `increment` instruction node in the IDL. A grader that greps the generated JSON for the Rust name fails a correct regeneration.

- **A skill-less agent can still finish the hardest migration task.** With no skill installed at all, the agent completed a manual `u64` → `u32` transition correctly, including the version-header offset that tripped earlier runs. The macro errors themselves name the remedy, which is presumably how it recovered. Whether the guidance is _reachable_ through the CLI alone is therefore graded deterministically (`cli-self-sufficiency`) rather than by asking whether a strong model bothered to look.

- **The guidance channel is now part of the toolkit.** `pina skill` (list), `pina skill read <topic>` (raw Markdown, byte-identical to the published package), and `pina skill install --dir <dir> [--force]` ship inside the binary, embedded by `scripts/docs/sync-skill.mjs` and drift-checked by `verify:docs`. `pina --help` and `pina docs` both advertise it.

### Round-two pass rates

Against the committed branch, **every scenario passes for both variants except `manual-transition-offsets`, where the baseline fails and the improved skill passes.** That single difference is real and was confirmed by reading the artifact, not by re-grading:

- `manual-transition-offsets` — merging `count` into a packed field needs a hand-written conversion, because the tag's high 16 bits must read as zero on legacy accounts. The baseline let `create` record an automatic byte copy (`copy_within`), which preserves those bits; the improved run recorded a manual transition and masked them off. The check that catches it grades the recorded mode in the manifest, which is exactly the decision the task turns on.

`init-new-project` is **not** a stable difference. One run had the baseline skipping `pina migrations create` (leaving no version-0 history) while the improved run completed it; a three-repeat measurement of both variants passed 3/3 each. Treat that cell as run-to-run variance, which is why the suite needs repeats before any single cell is quoted as evidence.

So the honest claim is narrower than a general improvement: the corrected guidance changes the outcome on one task class — a change that is not a byte copy — and everywhere else the baseline already passed. The suite's primary value remains as a regression gate over the whole skill surface.

### Third-round grader corrections

Re-running the full matrix against the committed branch exposed five grader defects that had been failing correct work, plus one scenario premise that was too narrow. All were fixed before the results above were recorded:

- `add-regression-test` demanded the new test in `src/lib.rs`, though the scenario's own expectation allows a `tests/*.rs` integration test — which is what both runs wrote, under two different file names. The check now accepts either native test surface.
- `version-exhaustion` pinned `"versionsRemaining": 65535`, a `u16`-only figure. Both runs correctly chose `u32` (4294967295). The check now derives the expected budget from the width the agent actually chose.
- `init-new-project` hardcoded `escrow_program/` paths, but `pina init <name> --path .` scaffolds into the current directory — an equally valid reading of "in this directory" one run took. The checks now accept either layout.
- `add-optional-field` asked for the raised lamport budget through the transcript, but the transcript only carries assistant-authored content; the constant lived in the file. It also hardcoded one requirement figure, where the real requirement depends on the account shape the agent chose. The check now reads the declared budget from `src/lib.rs` and the requirement from `pina migrations status`.
- `pda-account` graded PDA derivation and the create builder through the transcript. The macros expand to `try_find_program_address`, so the check fired on generated code the agent merely read, and on a comment saying the call was avoided. It also demanded `invoke_with_bump`, which is only correct when the account stores a bump — a design choice the task leaves open. Both now grade the artifact.
- `declarative-validation` taught and graded `==` as the validation separator, which came from an uncommitted sibling change rather than this branch. It now grades the single `=` separator these crates ship.

The lesson matches round one: grader defects are the dominant risk, and every one of them failed work that was correct. The recurring form is a check that asks a property of the artifact through the transcript, or that pins one of several correct answers.

### Second-round grader lessons

- **A scenario must not assume an account the fixture does not have.** The first close-account prompt invented a `Journal` the fixture never declared; the agent correctly stopped and asked. Prompts are graded against the fixture, so they must describe the fixture.

- **An assignment prefix does not survive `&&`.** `PATH=… pina a && pina b` shadows `pina` for `a` only; `b` silently resolved to a stale release. The grader now injects `PATH` through the child environment instead of the command string.

- **Soft goals need deterministic homes.** "The agent should find the guidance through the CLI" sounded like a scenario check, but a model that already knows the answer will not go looking — grading its transcript fails correct work. Reachability is now a property test of the CLI; the agent scenario grades only the work.

## Round four: the end-to-end flows and the generated clients

Rounds one to three graded the skill against migration tasks. Round four asked a different question: when an agent follows the skill, the docs, and the CLI literally through the whole lifecycle (`pina init`, day-to-day usage, the migration flow, deployment, and the generated clients an application is built on), does each step produce the outcome the documentation promises, and is that outcome safe?

### Method

Four independent evaluations ran in parallel against the workspace CLI, each in throwaway projects, each told to reproduce every finding before reporting it:

- **init and general usage:** a fresh `pina init` project taken through every documented command, plus hostile inputs (names, destinations, existing workspaces);
- **migrations:** every schema-change class on the three fixtures, drift and tamper attempts on the manifest and ledger, and a native harness that ran generated and hand-written transition bodies on real account bytes;
- **deployment:** 35 `--cluster` spellings, malformed and mis-permissioned keypairs, shell-injection paths, `--remote-command`, and the publication ledger around failed deploys;
- **generated clients:** the TypeScript, Dart, Rust, and CPI output of every example, scrutinized against the program's validation, plus crafted malicious IDLs.

Every defect below was fixed in this round and is pinned by a unit, integration, or contract test. The ones an agent is most likely to hit are also pinned by the deterministic `migration-safety-gates` scenario. The whole agent suite was then re-run on both skill variants.

### Defects that produced wrong outcomes

- **A fresh project could not complete the documented loop.**
  - `pina lint` asked for a driver built for a nightly no release ships, because the scaffold pinned an old toolchain.
  - TypeScript, Dart, `cli-ts`, and `cli-dart` generation failed outside this repository. The stdin render script could not resolve `npx -p` packages, and the published `@pina-rs/codama-renderer-cli` tarball had no `dist/`.
  - Every generated Rust, CPI, and `cli-rust` client inherited dependencies from a workspace the scaffold does not have.
  - The skill's own setup sequence omitted `pina keys new` and `pina migrations create`, so `pina build` failed on its first run.
- **A finished manual transition corrupted accounts silently.** A body written for one draft layout survived a later change to that draft, compiled, and passed `check`. In the native harness it moved `count` into the new `revision` field and shifted `authority`. `create` now moves a stale body to `vN_to_vM.rs.stale` behind a build-blocking stub.
- **Published history could be rewritten.** A receipt with an empty `history` skipped the published-schema check, so blanking it let a published `u64` become a `u32` while `check` reported "consistent". Unpinned receipts are now refused until `reconcile --pin-legacy` pins them, and the committed fixtures and `examples/migrations_program` were pinned. Pinning an untampered ledger reproduces the original receipts byte for byte.
- **Generated clients encoded the wrong wire format.** Before a baseline existed, `pina idl` and `pina generate` emitted an IDL without the version byte, so clients were one byte short of the program the next `create` produces. Both now refuse.
- **The generated `Migrate` call always failed on chain.** The TypeScript, Dart, and Rust composers put the program address in the system-program slot the program asserts, and a test asserted that broken layout.
- **Payers were read-only in the IDL.** A payer passed as `&AccountView` to a creation builder was emitted read-only in `counter_program`, `todo_program`, and `float_accounts_program`, so any transaction with a separate fee payer, and any CPI, failed with a privilege escalation.
- **Event parsers trusted any program.** The TypeScript and Dart log parsers decoded every `Program data:` line, so a program invoked through CPI could forge events with the right discriminator, and one foreign future-version line made the whole parse throw. They now attribute each line through the transaction's invocation frames.
- **Generated code could be injected.** A `*/` in a Rust doc comment became live TypeScript, a multi-line `#[doc]` value could end a `///` comment, and `$` in IDL text became an evaluated expression in the Dart CLI.
- **`--manual` was silently ignored** on a draft already recorded as automatic.
- **Changing the program ID stranded the history.** `pina keys new` left every migrations and deploy command failing with no recovery short of deleting `migrations/`. An unpublished history now rebinds on the next `create`.
- **A deploy that never started froze drafts.** With `solana` missing, the pending record could only be abandoned, which freezes the versions forever. A record written by an attempt that provably never started is now discarded.

### Defects that only misled

These made no outcome wrong, but each cost an agent a turn or pointed it at the wrong fix: help text with literal tabs and backslashes (the repository's `format_strings = true` rustfmt setting splits long string literals in the middle of escape sequences, which also turned one test's source rewrite into a silent no-op; the setting is now off); an invalid `rustup --toolchain` remedy; a URL-parser error for a mistyped cluster name; a Surfpool test template with doubled braces; compact `MAX_SIZE` in `tests/abi_layout.rs` that excluded the envelope `MIN_SIZE` included; an envelope-acknowledgement message that claimed bytes shift for brand-new contracts; `pina init` accepting names Cargo rejects; and a placeholder program ID that `pina doctor` reported as a pass.

The docs and the skill carried the same kind of error. They claimed `create` persists answers to `pina.toml` (it only reads them), that reordering is automatic (it is manual), that a missing manifest always fails the build (an `auto` policy builds envelope-free), that decoders enforce exact lengths (TypeScript accepts trailing bytes), that `pina deploy` never runs a shell string (`--remote-command` does), and that the scaffold ships Mollusk and an SBF linker configuration (it ships neither). The flow page showed a `pina deploy --program-id` flag that does not exist, and the migration flow showed a `.make()` call on an object that has no such method. Each claim was corrected against the CLI's actual behavior.

### Agent-suite results

After the fixes, both variants were run through every scenario with one agent run per cell:

| Variant    | Pass rate | Notes                                                    |
| ---------- | --------- | -------------------------------------------------------- |
| `improved` | 18/18     | after regrading `close-counter` with the corrected check |
| `baseline` | 18/18     | after regrading `pda-account` with the corrected check   |

The suite still does not separate the variants, which matches the earlier rounds: a strong model completes these tasks with either skill. This round's value is the CLI and documentation fixes above and the gates that now pin them, not a pass-rate delta. Do not quote the table as evidence that the updated skill is better.

### Grader and harness defects

- `close-counter` anchored its security checks on the literal `CloseCounter`. The improved run named the instruction `Close`, and its handler checked the signer and the stored authority before `CloseAccountZeroed` exactly as required. The checks now anchor on the close handler's `ProcessAccountInfos` impl, and a mutation that removes its signer check still fails them.
- `pda-account` required `CreateProgramAccount`, so the baseline run failed for choosing `CreateCompactProgramAccount`, the correct builder for its compact account. The check now accepts either family.
- The harness graded 18 runs as ordinary failures when the agent runtime refused to start at all (bypassed permissions under root), with zero tokens used. That report looked like a skill regression. The harness now aborts when the runtime exits without a transcript, and the README documents `IS_SANDBOX=1` for disposable root containers.

Both grader defects repeat round three's lesson: they pinned one correct answer, and both failed correct work.

### Not fixed in this round

These were reproduced and are documented in the skill or the docs, but need design work of their own:

- CPI crates have no owner-checked account entry point, and they report `LEN`/`MAX_LEN` that are wrong when fields are dropped or capacities ignored.
- `pina migrations inspect` checks neither the account owner nor its length, and the cost preview can name an instruction that cannot fund growth as the most expensive one.
- A loopback deploy URL may be a tunnel to a live cluster; the documented remedy is to use the real URL until the CLI checks the genesis hash.
- `tests/abi_layout.rs` contains constants but no `#[test]` functions.
- Retiring a never-published contract still requires deleting `migrations/`, and TypeScript decoders still accept trailing bytes (upstream Codama behavior).
