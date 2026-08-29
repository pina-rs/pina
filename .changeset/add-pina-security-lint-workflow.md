---
pina_cli: feat
pina_skill: fix
---

# Add a project-aware security lint workflow

Add `pina lint` and `pina lint --fix` for running Pina's official security lints against the discovered program. The command prepares pinned, project-local Dylint tools and selects lint libraries from the Pina release matching the installed CLI.

The repository's Dylint runner, linker, lint authoring API, and lint test harness are upgraded together to Dylint 6.0.4.

The seed-namespace lint now distinguishes generated associated `seeds` builders from unrelated local bindings, closing a false-negative path while preserving the ergonomic generated-builder exemption.

New `pina init` projects now register the same immutable-revision lint set and Dylint binary versions in Cargo workspace metadata. Generated next steps, project documentation, and the Pina agent skill include the security lint workflow.
