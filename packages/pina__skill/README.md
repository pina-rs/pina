# @pina-rs/skill

Agent guidance for creating, auditing, and maintaining Pina Solana programs.

The skill covers project setup, discriminator-first data layouts, account validation, PDA design, IDL and client generation, SBF profiling, and proportionate verification. Its instructions preserve `no_std` compatibility and treat the checked-in project configuration as authoritative.

## Install

```sh
npm install --global @pina-rs/skill
pina-skill --install
```

The default destination is `$CODEX_HOME/skills/pina` when `CODEX_HOME` is set, otherwise `~/.codex/skills/pina`. Installation refuses to replace an existing skill directory.

Inspect the source path or print manual installation instructions with:

```sh
pina-skill --print-path
pina-skill --print-install
```

At runtime, agents begin with [SKILL.md](./SKILL.md) and load a focused file under [references](./references) only when the task needs it.
