# Compute-unit performance

Pina tracks four performance signals on every pull request:

- The ignored Surfpool suite records compute units for every exercised instruction against copied base and head ELFs.
- `pina profile` records static whole-program estimates, binary size, text size, and syscall counts for every top-level example program.
- Hyperfine compares representative base and head CLI commands.
- The existing host benchmark suite compares medians for performance-sensitive core operations.

The comparison uses `base - head`. A positive score is an improvement because the head consumes fewer compute units. A negative score is a regression.

The jobs run in parallel and update one sticky pull-request comment. CLI and host timings are advisory because hosted-runner timing is noisy. Instruction-CU regressions enforce the configured policy.

## PinaPod v0.2 migration results

The following exact results compare Pina v0.14.0 (`eeaeb1ec`) with the PinaPod v0.2 migration. Both sides use Solana's `cargo build-sbf`, the pinned `nightly-2025-11-20` toolchain, Mollusk 0.14.0, identical instruction bytes, and identical account fixtures.

| Instruction case                         | Base CU | Head CU | Performance change | Change |
| ---------------------------------------- | ------- | ------- | ------------------ | ------ |
| `account_realloc_program/grow_0_to_8`    | 8,378   | 6,749   | +1,629             | +19.4% |
| `account_realloc_program/initialize`     | 9,758   | 9,782   | -24                | -0.2%  |
| `account_realloc_program/rewrite_8_to_8` | 6,963   | 5,335   | +1,628             | +23.4% |
| `account_realloc_program/shrink_8_to_0`  | 7,110   | 5,508   | +1,602             | +22.5% |
| `counter_program/increment`              | 2,184   | 1,958   | +226               | +10.3% |
| `counter_program/initialize`             | 14,298  | 14,198  | +100               | +0.7%  |
| `profile_program/add_tag`                | 2,267   | 2,271   | -4                 | -0.2%  |
| `profile_program/initialize`             | 7,224   | 7,392   | -168               | -2.3%  |
| `profile_program/remove_tag`             | 2,296   | 2,276   | +20                | +0.9%  |
| `profile_program/update_profile`         | 2,576   | 2,554   | +22                | +0.9%  |

Seven of ten instruction cases improve. Compact realloc operations improve by 19.4% to 23.4% after removing duplicate account and PDA validation. The generated `load_pda_mut` helper makes the counter mutation 10.3% faster and prevents recursively validating a fixed representation several times at one boundary.

Three reviewed increases remained in that historical comparison. Compact initialization added 24 CU, fixed profile tag insertion added 4 CU, and fixed profile creation added 168 CU. Profile creation deliberately validated the completed `String`, `Vec`, and `Option` representation after its initializer ran. That final check prevented a closure from committing malformed state.

## Static SBF profile results

The broader static profile agrees with the instruction-level measurements. Unaffected examples remain byte-for-byte stable, while every example changed by the account migration has a lower whole-program CU estimate.

| Program                              | Base CU | Head CU | Performance change | Change |
| ------------------------------------ | ------- | ------- | ------------------ | ------ |
| `hello_solana_program`               | 837     | 837     | +0                 | +0.0%  |
| `duplicate_mutable_accounts_program` | 1,004   | 1,004   | +0                 | +0.0%  |
| `events`                             | 636     | 636     | +0                 | +0.0%  |
| `sysvar_checks_program`              | 1,662   | 1,662   | +0                 | +0.0%  |
| `system_accounts_program`            | 1,130   | 1,130   | +0                 | +0.0%  |
| `account_realloc_program`            | 5,135   | 4,787   | +348               | +6.8%  |
| `compact_accounts_program`           | 6,877   | 6,445   | +432               | +6.3%  |
| `counter_program`                    | 4,079   | 3,056   | +1,023             | +25.1% |
| `profile_program`                    | 4,930   | 3,588   | +1,342             | +27.2% |

The four changed binaries also shrink: `account_realloc_program` by 2,872 bytes, `compact_accounts_program` by 3,560 bytes, `counter_program` by 8,584 bytes, and `profile_program` by 11,464 bytes. These are historical migration results; the pull-request report now measures all examples automatically.

## Pull request policy

Instruction cases fail on every unapproved increase. A reviewed redesign can add an absolute ceiling to `runtimeApprovedTotals`. The allowance applies only when the base is below that ceiling and the head does not exceed it, so a later increase fails again.

Static profiles warn when both the absolute and percentage warning thresholds are reached, and fail when both failure thresholds are reached. Smaller static increases stay visible as regressions. Programs and instructions are discovered from the head checkout. Head-only items create baselines; missing head measurements fail.

The workflow uploads the reports, manifests, copied ELF files, and SHA-256 hashes as CI artifacts. This provenance prevents a successful report from silently describing a stale or different binary.

## Run the comparison locally

Run the complete base-versus-head comparison from the repository root:

```sh
devenv shell -- report:cu:compare:main
```

Run only the current all-example static profile set:

```sh
devenv shell -- profile:cu:tracked
```

Reports are written under `target/cu/`. The Markdown report is intended for review; the JSON report is the machine-readable source for later analysis.
