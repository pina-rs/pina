# `pina doctor`

Diagnose whether the current checkout is ready for Pina development.

```text
pina doctor [OPTIONS]
```

```bash
pina doctor
pina doctor --path ./programs/counter
pina doctor --json
```

The report checks:

- nearest Cargo program package, source entrypoint, and declared program ID;
- lint-driver resolution: the active toolchain, the release the shipped lints are verified against, the resolved driver with its own toolchain and how it was obtained, both search paths, and the remedy when nothing matched;
- canonical SBF artifact and program-keypair paths;
- source/keypair identity agreement;
- required Rust/SBF tools (`cargo`, `rustc`, `rust-src`, nightly `-Z` support, and `sbpf-linker`);
- optional Solana and Surfpool tools;
- Node.js plus an `npx` or pnpm renderer when configured clients require JavaScript tooling.

Human output is stable, color-free text. It includes typed check IDs such as `project.discovery`, `project.program-id`, `project.artifact`, `lint.driver`, and `tool.surfpool` so the same vocabulary appears in logs and agent output.

The `Lint driver:` section answers the one question a failing `pina lint` leaves open — which toolchain is active, which driver Pina resolved for it and which toolchain that driver was built for, and what to run when none resolved. `pina doctor` reports this state **without** downloading or building anything: a diagnostic that populates a cache cannot be run to find out what is wrong. A missing driver is a warning, not an error, because it blocks `pina lint` only and the project can still build and deploy.

## Agent JSON

```bash
pina doctor --json > doctor.json
```

JSON is the only stdout content. Schema version `1` includes `status`, `project`, `tools`, typed `checks`, and actionable `findings`. Each check has a stable `id`, `status` (`pass`, `warn`, or `fail`), and `message`. The report never includes environment-variable values or keypair secret bytes.

Every external version or capability probe receives closed stdin, bounded output capture, and a five-second deadline. Pina attempts to terminate a tool that hangs and reports it as unavailable; it also stops waiting when a descendant keeps the tool's output pipes open. Agent diagnostics therefore return within a predictable bound even when a probe misbehaves.

Warnings—including missing optional tools, an unbuilt artifact, a missing local keypair, or an unresolved lint driver—exit with code `0`. Missing project discovery, unreadable program identity, or unavailable required Rust/SBF prerequisites produce `status: "error"` and exit with code `1`; the JSON document is still emitted in full.
