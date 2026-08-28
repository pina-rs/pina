---
name: pina
description: Create, audit, and maintain no_std Solana programs built with Pina and pinocchio. Use for Pina project setup, program identity, account and instruction authoring, PDA validation, diagnostics, IDL or client generation, SBF profiling, tests, and project upgrades.
---

# Pina

Build Pina programs that are small, explicit, and safe at the account boundary. Preserve the project's chosen structure and commands unless the user asks for a redesign.

## Establish the local contract

Before changing code:

1. Read the nearest `AGENTS.md`, `Cargo.toml`, `.cargo/config.toml`, and project documentation.
2. Inspect the installed interface with `pina --help` and `pina <command> --help`; do not rely on remembered flags.
3. Identify the program crate, its `pina` version and features, its entrypoint feature, and its existing test harness.
4. Treat generated IDLs and clients as derived files. Find the repository's generation command before editing them.

When no project exists, read [references/project-setup.md](references/project-setup.md). For an existing program, select only the reference that matches the task.

## Non-negotiable program invariants

- Preserve `no_std` compatibility for on-chain code. Keep host-only tooling and test dependencies outside the program runtime path.
- Do not introduce `unsafe` code or unstable features.
- Validate account identity, signer status, writability, ownership, and data shape before casts, mutation, lamport transfers, resize operations, or CPI.
- Use explicit discriminator values and type-specific PDA seed namespaces. Prefer canonical bump validation.
- Construct account-management operations and generated CPIs as documented instruction structs, then call `.invoke()` or `.invoke_signed(signers)`. Do not recreate the removed free-function helper API.
- Keep instruction dispatch deterministic: parse once, match explicitly, then construct and validate the accounts type for that instruction.
- Maintain discriminator-first, fixed-layout storage types expected by Pina and zeropod. Do not place ordinary Rust `bool`, `String`, or variable-length collections in zero-copy account layouts.
- Preserve error values and wire formats unless the user explicitly accepts a compatibility change.

Read [references/program-authoring.md](references/program-authoring.md) before changing macros, account layouts, validation chains, PDAs, CPIs, or close/reallocation logic.

## Workflow

1. Inspect the smallest relevant surface and state the compatibility boundary: wire format, account layout, program ID, generated IDL, or CLI output.
2. Make the narrowest idiomatic change. Reuse Pina validation and loader APIs instead of duplicating parsing or ownership checks.
3. Add or update a regression test at the layer where the behavior is observable.
4. Run focused tests first, then the repository's documented format, lint, build, and test commands. For a standalone project without task aliases, use Cargo commands from [references/testing.md](references/testing.md).
5. Regenerate the IDL and clients when the public program surface changes. Review the diff before accepting generated output.

## Task routing

- Project creation, dependency features, entrypoint wiring, or workspace layout: read [references/project-setup.md](references/project-setup.md).
- Accounts, instructions, discriminators, PDAs, validation, CPI, resize, or close behavior: read [references/program-authoring.md](references/program-authoring.md).
- CLI discovery, project diagnostics, program keys, IDL extraction, Codama client generation, terminal docs, completions, or profiling: read [references/cli-and-codegen.md](references/cli-and-codegen.md).
- Unit, Mollusk, SBF, generated-artifact, or release checks: read [references/testing.md](references/testing.md).

Do not load every reference for routine edits.
