---
pina: none
pina_macros: none
---

# Build every example program with fat link-time optimization

Every example program was built for SBF without link-time optimization, even those whose `crate-type = ["cdylib"]` allows it: the `cargo build-<program>` aliases and `scripts/build-runtime-compute-units.ts` never passed `--lto`, so the PR size benchmark recorded ELFs 12-37% larger than what `pina build` (which applies its `Production` profile) produces for the same program.

All 27 example programs now ship `crate-type = ["cdylib"]` only. Their `tests/surfpool` crates include the real source with `#[path = "../../../src/lib.rs"]` instead of depending on an rlib, mirroring each program's `pina` features in the harness manifest so the included source compiles identically; the twelve `tests/*.rs` integration tests that imported the program crate use the same include (re-exporting the program module at the test crate root where the program's submodules use `crate::` paths). Every build alias and every CU-measurement build passes `--lto`, and the example inventory derives the flag from each manifest (`ltoEligible`), so the benchmark always measures the artifact users actually deploy.

Measured on the same commit, same toolchain, three deterministic Surfpool runs each:

| Program         | Without LTO | With LTO | Δ size | Static CU Δ |
| --------------- | ----------- | -------- | ------ | ----------- |
| escrow          | 73,976      | 46,680   | −37%   | −34%        |
| staking-rewards | 87,264      | 59,120   | −32%   | −29%        |
| vesting         | 57,632      | 46,824   | −19%   | −14%        |
| multisig        | 195,752     | 166,792  | −15%   | −10%        |
| hello-solana    | 5,656       | 4,712    | −17%   | −18%        |

Runtime compute units do not regress: fat LTO's single codegen unit lets inlining collapse cross-crate glue that the unoptimized link kept as call sequences, so escrow's Make drops 29,535 → 29,105 CU and Take 32,230 → 31,658 CU. Every entrypoint frame stays within the 4 KB stack limit (deepest: vesting at 3,992; multisig sits exactly at 4,096 with no offset beyond it — its manifest documents the re-check rule for any future instruction growth, and its 17 Surfpool plus 38 e2e tests pass against the LTO ELF).

## Share one allocation spine across creation builders

The compact and fixed-layout PDA creation builders duplicated their entire allocation sequence once per generic instantiation: seed marshalling, signer assembly, rent computation, and the system-program CPI — ~5 KB per copy, three copies in the multisig example alone. `CompactCreationTarget::allocate_zeroed` and `PdaCreationTarget::allocate` now carry that runtime-only spine as shared functions, leaving the generic half (patch commit, discriminator init) thin. Multisig's three `CreateCompactProgramAccountWithBump` instantiations dropped from 15,736 to 7,280 outlined bytes plus one 3,000-byte shared copy — 5,252 bytes gone. Each spine keeps the shape its users measured best with. The compact-creation spine (`allocate_zeroed`) stays a real outlined call — with no `inline` attribute, LLVM keeps it outside the generic instantiations, so multisig's three copies collapse into the single shared 3,000-byte function. The PDA-creation spine (`PdaCreationTarget::allocate`) carries `#[inline(always)]` instead, because a single-instantiation program has no duplicate to collapse and pays only the call boundary: outlining it measured +80 CU and +768 bytes on the counter fixture's `initialize`. After the split, the fixture measures exactly its previous 12,720 bytes and 3,295 CU, and the escrow Surfpool suite still passes 10/10 against the rebuilt artifact.

## `#[path]` harnesses can expand migration-aware discriminators again

`verify_migration_contracts` resolved `migrations/manifest.json` from raw `CARGO_MANIFEST_DIR`, so a Surfpool harness that source-includes a cdylib-only program expanded the program's `#[discriminator]` with the harness's manifest directory and reported the program's declared migration ladder as missing — blocking the documented `#[path]` test pattern for every migration-aware program. It now uses `discover_program_dir()`, the same upward walk `#[account]` and the account ladder already use, so the manifest is found through the include just as it is through an rlib. The two unit tests and one trybuild fixture that asserted the old raw-path error now assert the unlocatable-manifest error the walk produces when no manifest exists anywhere above the expanding crate.

## Privacy pool prover module compiles in source-including harnesses

The host-only `prover` module is gated on the `prover` cargo feature, which a source-including harness cannot enable _on the program_ (features belong to a dependency, and a cdylib-only program is not a dependency). The Surfpool harness now declares `prover` as one of its **own** features — a `#[path]` include resolves `cfg(feature = ...)` against the including crate, so the harness's feature opens the module — while carrying the `ark-*` crates the module imports as plain dependencies. The program's gate itself is unchanged (`all(feature = "prover", not(target_os = "solana"))`): admitting every `cfg(test)` build would break the program's own lib-test target, which compiles under default features where the optional `ark-*` dependencies are absent. The SBF artifact is unaffected (`not(target_os = "solana")` still excludes the module from every on-chain build), and the `[[test]] required-features` on the program's own e2e suite is unchanged.

## Program self-tests moved beside the programs they pin

Converting the programs to cdylib-only left the workspace compute-unit harness (`tests/compute_units.rs`) linking against programs that no longer produce rlibs, so it now source-includes them with `#[path]` like the Surfpool harnesses. That exposed a rule the embedded `#[cfg(test)]` modules had been silently exempt from: their assertions pin enveloped codegen (envelope geometry, version bytes), and migration-aware codegen only expands when `migrations/manifest.json` is discoverable by walking up from the expanding crate — true for any crate inside the program's tree, false for a workspace-root harness. The programs' self-tests moved to `tests/` beside each program (migration-aware assertions to `tests/generated_views.rs` / `tests/self_checks.rs`, where the walk still finds the manifest); the account-realloc program's private-helper tests stayed in-crate because only the crate itself can reach those functions. The migrations program's exact runtime-CU snapshot moved to its own `tests/compute_units.rs` for the same reason, and the workspace harness now measures the four non-migration-aware programs. Every suite count is preserved: 18 migration-view tests, 13 counter, 22 profile, 11 realloc all run in their own packages.

The fuzz targets had the same dependency shape: `crates/pina_fuzz` depended on `counter_program`, `role_registry_program`, and `migrations_program` as rlibs, which cdylib-only programs no longer produce. Each program now carries a `fuzz/` re-export crate inside its own tree — the source include expands the migration-aware macros with the manifest found by the walk, and the rlib the wrapper produces carries the already-expanded codegen — and the fuzz targets depend on those (`migrations_program_fuzz` re-exports the program's `fuzzing` feature through its own, since an include resolves features against the including crate). The corpus and target set are unchanged; the smoke tier replays and fuzzes all three targets green.

## Measured the entrypoint account budget and recorded the lean-entrypoint strategy

The framework-comparison fixtures never passed `nostd_entrypoint!` its second argument, so they carried pinocchio's unrolled 255-account deserializer for programs whose widest instruction uses three accounts. New `pina_lean` fixtures measure the bounded budget with the exact comparison profile and verifier: hello 4,680 → 2,736 bytes (−41.5%) at +6 CU, counter 12,720 → 11,680 (−8.2%) at +1/+4 CU. ADR 0010 (`docs/src/adrs/0010-lean-entrypoint-strategy.md`) records the full exploration — where the remaining bytes live (derive dispatch, the `RangeInclusive` envelope triplication, error conversion), what Quasar and Anchor v2 do differently (lazy per-arm account walking, 32-byte entrypoint stubs), and the phased decision: adopt the budget now, build the opt-in `lean_entrypoint!` dispatcher behind equivalence measurements, and keep pinocchio as a library. The program-size guide documents the budget lever with the measured table.
