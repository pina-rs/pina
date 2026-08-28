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
- canonical SBF artifact and program-keypair paths;
- source/keypair identity agreement;
- required Rust/SBF tools (`cargo`, `rustc`, `rust-src`, nightly `-Z` support, and `sbpf-linker`);
- optional Solana and Surfpool tools;
- Node.js plus an `npx` or pnpm renderer when configured clients require JavaScript tooling.

Human output is stable, color-free text. It includes typed check IDs such as `project.discovery`, `project.program-id`, `project.artifact`, and `tool.surfpool` so the same vocabulary appears in logs and agent output.

## Agent JSON

```bash
pina doctor --json > doctor.json
```

JSON is the only stdout content. Schema version `1` includes `status`, `project`, `tools`, typed `checks`, and actionable `findings`. Each check has a stable `id`, `status` (`pass`, `warn`, or `fail`), and `message`. The report never includes environment-variable values or keypair secret bytes.

Every external version or capability probe receives closed stdin, bounded output capture, and a five-second deadline. Pina attempts to terminate a tool that hangs and reports it as unavailable; it also stops waiting when a descendant keeps the tool's output pipes open. Agent diagnostics therefore return within a predictable bound even when a probe misbehaves.

Warnings—including missing optional tools, an unbuilt artifact, or a missing local keypair—exit with code `0`. Missing project discovery, unreadable program identity, or unavailable required Rust/SBF prerequisites produce `status: "error"` and exit with code `1`; the JSON document is still emitted in full.
