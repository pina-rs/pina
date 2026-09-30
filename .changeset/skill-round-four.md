---
pina_skill: fix
---

# Update the skill with the round-four evaluation findings

The setup sequence now runs `pina keys new` and `pina migrations create` before building, and its example dispatch uses the `try_from((program_id, accounts))` form the accounts derive implements. The migrations reference documents stale manual bodies, `--manual` conversion, the refusal of unpinned receipts and the `--pin-legacy` repair for ABI 0.20 ledgers, program-ID rebinding, the envelope-free build a missing manifest allows, and the `--auto` and `--version-type` flags that record the migration policy in the manifest now that `pina.toml` holds none, and it corrects two claims: `create` never writes `[migrations.answers]`, and reordering or widening fields is manual. The CLI reference gains a "Using generated clients safely" section on owner checks, event attribution, trailing bytes, and writable flags, plus deploy caveats for loopback tunnels and snapshot paths. The testing reference drops the unconfigured `bpfel-unknown-none` build and the Mollusk claim.
