---
pina_macros: feat
pina_lints: feat
pina: fix
pina_root: none
pina_cli_renderer: none
pina_abi: none
pina_codama_nodes: none
pina_codama_renderer: none
pina_codama_renderer_cli: none
pina_codama_renderer_cpi: none
pina_cpi_renderer: none
pina_profile: none
pina_sdk_ids: none
pina_skill: none
pina_test: none
---

# Close the shadow-PDA and discriminator-collision gaps

Fixed-account `#[pda]` schemas now generate `load_checked_pda` and `load_checked_pda_mut` alongside the stored-bump `load_pda` pair. The checked loaders search the seeds for the canonical bump and reject both an account at any other address and a stored bump that is not canonical — the fixed-account counterpart of the compact family's `with_checked_pda`, and previously the one PDA family with no canonical option at all. The audit proved the gap executable: a shadow account created at a noncanonical bump whose stored bump field matches passes `load_pda` byte for byte, so a program whose seeds do not bind a required signer accepts the shadow and the canonical account as the same logical entity (`crates/pina/tests/audit_adversarial.rs`). The stored-bump loaders keep their single-derivation cost and their documentation of when they are safe.

A new deny lint `deny_colliding_account_discriminators` closes the other proven framework finding. Discriminators are author-chosen integers and rustc only rejects duplicates within one enum, so two account enums that agree on a value and a serialized width pass every typed loader check — owner, discriminator, exact size — and either account deserializes as the other (the sealevel-attacks type-cosplay class; proven executable in the same test file). The lint collects every generated `impl HasDiscriminator` for types that also implement pina's account traits, evaluates the `VALUE` const and the repr width, and denies any value claimed by two different account types. Events share the trait shape but live in the log namespace, so they are excluded by construction. A UI fixture pair pins the collision and the clean case, and the lint ships in the catalog with the CLI's `lints.json`, `--explain` reference, and the docs page updated to match.
