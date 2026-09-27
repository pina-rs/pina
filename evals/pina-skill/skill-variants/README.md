# Skill variants

Each directory here is a complete `pina` skill that a run can install, so a skill change can be measured rather than asserted.

- `baseline` — the skill as it was before the evaluation work (the version at the commit this branch started from). It is deliberately frozen: editing it would invalidate every comparison already recorded in `results/`.
- `improved` — the candidate skill, carrying the changes this evaluation justifies. It is kept byte-identical to `packages/pina__skill`, which is the published artifact, so the thing that was measured is the thing that ships.

Add a new directory for any other candidate. Keep at least one variant frozen as the reference point; a comparison against a moving baseline proves nothing.
