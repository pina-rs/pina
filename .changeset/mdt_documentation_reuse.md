---
default: patch
pina: patch
pina_pod_primitives: patch
---

Expand mdt documentation reuse across the workspace.

Added 10 new mdt provider blocks in `template.t.md`:

- `pinaProjectDescription` — single-source project tagline
- `pinaInstallation` — cargo add instructions
- `podTypesTable` — Pod types reference table
- `podArithmeticDescription` — Pod arithmetic semantics
- `pinaWorkspacePackages` — workspace crate table
- `pinaFeatureHighlights` — feature bullet list
- `sbfBuildInstructions` — SBF build commands
- `pinaTestingInstructions` — testing commands
- `pinaBadgeLinks` — shared badge link references
- `pinaSecurityBestPractices` — security checklist

Wired 15 new consumers across:

- `readme.md` (root) — 10 consumer blocks
- `crates/pina/readme.md` — feature flags table + badge links
- `crates/pina_pod_primitives/readme.md` — pod types table + arithmetic description
- `docs/src/security-model.md` — security best practices

Provider/consumer counts: 13/31 → 23/46.
