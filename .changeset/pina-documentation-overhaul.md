---
pina: docs
pina_cli: docs
pina_codama_renderer: docs
pina_lints: none
pina_macros: none
pina_profile: none
pina_sdk_ids: none
pina_skill: none
pina_test: none
---

# Overhaul the book, readmes, and rustdoc

The published book, root readme, crate readmes, security policy, agent guidance, and rustdoc all get a verified accuracy pass: the empty Security Lints page includes the real lint catalog again, badges stop rendering mangled `pina**cli` labels, stale claims (zeropod naming, flash-loan guards, closed audit findings, closed issue backlogs, vesting token behavior, node toolchain provenance, changeset front matter) are corrected against the code, the workspace and examples inventories gain the crates and examples that shipped since they were written, the tutorials no longer show snippets that cannot compile, and internal working notes move out of the user-facing book with their statuses refreshed. The pina_codama_renderer entry API, the pina token aliases, and the pina_cli Codama generation options gain rustdoc, and generated clients no longer emit broken intra-doc links.
