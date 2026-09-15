# AGENTS.md

Pina is a Rust workspace for building performant, `no_std` Solana programs on top of `pinocchio`.

## Repo defaults

- **Always run commands inside `devenv shell`** so the nix-managed toolchain (cargo, mdt, dprint, clippy, etc.) is used instead of stale cargo-installed or system binaries.
- Use `devenv` for the development shell and repo task runner.
- Use `cargo` for workspace tasks; use `pnpm` only for JS/Codama subprojects.
- Write committed scripts in the repository's dominant dynamic language. Rust is not a scripting-language exception: in this Rust workspace, use TypeScript rather than Rust for scripts. Reserve Python scripts for predominantly Python projects, and fall back to TypeScript when no dynamic language dominates.
- Format with `fix:format` or `dprint fmt`; do not run `rustfmt` directly.
- Workspace code must preserve `no_std` compatibility where applicable.
- `unsafe_code` and `unstable_features` are denied workspace-wide.

## Contribution conventions

- GitHub issue titles must be written in title case (e.g. `Add Miri Coverage for Account Loader Aliasing Rules`). Do not use commit-style prefixes like `fix:` / `feat:` / `docs:` in issue titles.
- Pull request titles must follow Conventional Commits (e.g. `feat(loaders): preserve borrow guard lifetime`).
- Never merge a `chore(release): prepare release` pull request. Release pull requests must remain open until Ifiok Jr. (`@ifiokjr`) explicitly decides to merge them himself.

## Verify locally before pushing

- Before pushing any branch, run every CI job that the change can trigger and confirm it passes on this machine. Do not push a branch whose CI you have not reproduced locally, unless Ifiok Jr. explicitly tells you to skip that step.
- Reproduce the job with the same command CI uses rather than an approximation. `devenv shell -- <task>` matches the workflow step for most jobs; the SBF, Kani, and Surfpool tiers need their own wrappers. See [Testing and SBF builds](./docs/agents/testing-and-sbf.md) and [Git workflow](./docs/agents/git-workflow.md).
- Account for the path filters that decide which jobs run. A change under `crates/**` or `examples/**` reaches the Surfpool workflow, the performance benchmarks, and the `pina` feature matrix; a change under `.github/**` reaches the workflow audit. When in doubt, run the superset: `lint:all`, `verify:security`, `test:all`, and `test:idl`.
- A job that needs the SBF toolchain, Surfpool, or a network service still counts as reproducible locally. Build the artifact and run the tier instead of pushing and waiting for CI to tell you.
- Treat a locally skipped or unverified job as a known failure, and say which jobs are unverified when you hand off the branch. Never describe a branch as ready while a job it can trigger is unproven.

## Benchmark maintenance

- Before merging any pull request, wait for and read the consolidated performance benchmark comment. Do not merge while the report is missing, incomplete, or contains measurement errors.
- Treat every measured increase in instruction compute units, program compute units, build size, CLI time, or core host time as negative and investigate it. Rerun a measurement only with evidence that it is noise. Fix confirmed regressions or keep the pull request unmerged until Ifiok Jr. explicitly approves the trade-off.
- Confirm that every new top-level example appears in the benchmark report. A current-only result is acceptable when the example has no base-branch baseline.
- Every top-level example crate with a `bpf-entrypoint` feature is part of the performance inventory. Do not maintain a second allowlist for new examples.
- When an example gains an instruction, exercise that instruction through its ignored `tests/surfpool` suite with `pina_test::ProgramTest::send`, `send_instruction`, or `send_with_signers`. The PR benchmark records those calls automatically.
- Keep benchmark fixtures deterministic. Use fixed `Keypair::new_from_array` seeds and stable account addresses; never use random signers or accounts in a recorded instruction path.
- A program or instruction that does not exist at the PR base is a new baseline, not an error. Missing measurements for code that does exist at the head are errors.
- Add a focused case to `crates/pina/tests/benchmarks.rs` when changing a performance-sensitive host operation. Add a command to `scripts/benchmark-cli.ts` when introducing a representative CLI hot path.

## Coverage

- Patch coverage is a pull request gate with a **100% target** (`codecov.yml`). Every line a pull request adds or changes must be executed by a test, not merely present in a covered file. Add the test in the same pull request; do not defer it to a follow-up.
- `coverage` is the pull request's Rust gate and `test` is the post-merge and `ci-full` tier. The two must stay equivalent: `coverage:all` is a superset of `test:all`, so a suite added to one belongs in the other. `pina_abi` is tested nowhere else, so removing it from either silently stops testing it.
- Run `coverage:all` before pushing when a change touches Rust. It is the only way to see a patch-coverage failure before CI does, because the profiling build is not what `cargo test` runs.
- Paths excluded from the patch target are listed in `codecov.yml`: `codama/clients` (verified by deterministic regeneration), `crates/pina_test`, and `examples` (exercised by real-runtime suites the coverage job cannot run). Excluded is not untested — those paths still need tests, they just do not gate on patch percentage. Adding a path to that list to clear a failure is not a fix.
- The root package's `src/` and `tests/` are **not** ignored, so a change there gates on patch coverage. The stale `pina_root` entry in `codecov.yml` matches no file, because the package's code lives at the repository root rather than in a `pina_root/` directory.
- Some suites cannot run under instrumentation: `trybuild` compares compiler output byte for byte, and `crates/pina_cli/tests/generated_surfpool.rs` and `pina_root`'s `tests/ui.rs` are excluded with `#![cfg(not(coverage))]` for that reason. Excluding a suite from coverage must not remove it from the pull request gate: `coverage:all` runs `tests/ui.rs` a second time without instrumentation so the trybuild snapshots still gate, and `crates/pina_cli/tests/generated_surfpool.rs` is covered by the `surfpool-examples` job.

## Common commands

- `devenv shell` — enter the dev environment
- `install:all` — install pinned cargo binaries and external tools
- `cargo build --all-features` — build the workspace
- `cargo test` — run the default test suite
- `devenv --profile kani shell -- test:kani` — run all bit-precise Kani proof harnesses
- `devenv --profile kani shell -- test:kani:quick` — run fast arithmetic, parser, compact-sizing, fixed-layout, and CPI proofs
- `devenv --profile kani shell -- test:kani:compact` — run bounded compact-layout state-machine proofs
- `build:pina:no-default` — verify `pina` across no-default feature subsets
- `lint:all` — run clippy, formatting, and docs verification
- `verify:docs` — validate reusable docs and mdBook output
- `fix:format` — format files and re-sync mdt-managed docs

## Task-specific guidance

- [Build and tooling](./docs/agents/build-and-tooling.md)
- [Coding style guide](./docs/agents/coding-style.md) — visual organization, whitespace patterns, and code aesthetics
- [Workspace architecture](./docs/agents/workspace-architecture.md)
- [Testing and SBF builds](./docs/agents/testing-and-sbf.md)
- [Release process and changesets](./docs/agents/release-and-changesets.md)
- [Git workflow](./docs/agents/git-workflow.md)
- [Security and code constraints](./docs/agents/security-and-code-constraints.md)
