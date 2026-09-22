---
pina_cli: fix
pina_cpi_renderer: fix
---

# Fix the CPI scaffold's pina dependency for workspace crates

`pina generate` scaffolds a CPI crate for every example, but the scaffold hardcoded `pina = { version = "0.17", default-features = false }` — a registry pin that stopped matching this workspace's own `pina` releases. Every previously committed CPI crate had already been pinned to `pina = { workspace = true }` by hand, so the defect only surfaced on the next newly generated example: it added a second `pina` to the dependency graph, which makes `cargo kani -p pina` fail with "multiple `pina` packages in your project" and breaks both Kani proof tiers.

The scaffold now takes the dependency source from the caller. `pina generate`, whose output is always a workspace member, inherits the workspace dependency; `pina cpi` and `pina import`, whose output lives in the caller's own project with no workspace entry to inherit, pin the renderer's own release line instead of a stale literal. A regression test covers both modes.
