# Agent Skill

`@pina-rs/skill` packages task-focused guidance for agents that create, audit, or maintain Pina programs. It covers project setup, account and instruction authoring, PDA and authority validation, IDL and client generation, SBF profiling, tests, and compatibility review.

## Install

```sh
npm install --global @pina-rs/skill
pina-skill --install
```

The installer copies the runtime skill to `$CODEX_HOME/skills/pina` when `CODEX_HOME` is set, otherwise `~/.codex/skills/pina`. It refuses to replace an existing directory, so local skill changes cannot be lost silently.

Inspect the packaged source or manual installation details with:

```sh
pina-skill --print-path
pina-skill --print-install
```

## Scope

The skill is intentionally specific to Pina. Its entrypoint establishes the program invariants that matter across tasks:

- preserve `no_std` compatibility and the project's feature boundary;
- validate accounts before casts, mutation, resize, close, or CPI;
- construct account-management and generated CPI operations as documented structs, then call `invoke` or `invoke_signed`;
- keep discriminators and PDA seed namespaces explicit and stable;
- treat generated IDLs and clients as reviewed public contracts;
- verify changes at the smallest meaningful layer before running the full project suite.

Detailed guidance is split into focused references for setup, program authoring, CLI and code generation, and testing. Agents load only the reference needed for the current task.

## Package layout

| Path                              | Purpose                                           |
| --------------------------------- | ------------------------------------------------- |
| `SKILL.md`                        | Runtime entrypoint and task routing               |
| `agents/openai.yaml`              | Display metadata and invocation policy            |
| `references/project-setup.md`     | Scaffolding, boundaries, features, and entrypoint |
| `references/program-authoring.md` | Macros, validation, PDAs, CPI, resize, and close  |
| `references/cli-and-codegen.md`   | CLI discovery, IDLs, clients, and profiling       |
| `references/testing.md`           | Unit, VM, SBF, generated, and release checks      |
| `bin/pina-skill.cjs`              | Non-destructive installer and path discovery      |
