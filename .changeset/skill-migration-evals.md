---
pina_cli: feat
pina_skill: fix
---

# Serve the agent skill from the CLI, fix migration docs

## `pina skill`

The agent skill now ships inside the `pina` binary, so an agent holding only the toolkit can reach the same guidance a skill installation would give it:

```sh
pina skill                            # list the bundled topics
pina skill read pina                  # the entrypoint, raw Markdown on stdout
pina skill read migrations            # any reference, byte for byte
pina skill install --dir ~/.claude/skills/pina
```

`read` prints the document itself with no terminal rendering so agents can parse it; `install` writes the whole tree and refuses to replace an existing skill unless `--force` is passed. `pina --help` and `pina docs` advertise the command. The embedded documents are a committed copy of `packages/pina__skill` kept in step by `scripts/docs/sync-skill.mjs`; `verify:docs` fails when the copy drifts.

## Skill corrections

The manual-transition ABI is now written down instead of left to be inferred: a transition's `data` includes the discriminator and version envelope while the offsets printed in a generated stub's comment block are payload relative, so a `u64` shown at `32..40` is `data[34..42]`. Compact contracts get their own `target_size`/`working_size` shape, `--envelope-ack` is covered, growth budgets are sized against a whole stale ladder, and `tests/abi_layout.rs` is documented as a generated drift gate that `pina migrations check` enforces.

`references/project-setup.md` no longer recommends `#![cfg_attr(not(test), no_std)]` — every program in this repository except the fuzzing-gated `migrations_program` writes plain `#![no_std]` and gates only a host `cdylib`'s `extern crate std` — and the retired integer manifest formats are replaced with the `abiVersion` string both documents carry.

An evaluation harness for the skill lives in `evals/pina-skill`; seventeen scenarios grade real changes to throwaway programs against the real CLI, and its `FINDINGS.md` records the measured result and its limits.
