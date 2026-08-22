# AGENTS.md

Pina is a Rust workspace for building performant, `no_std` Solana programs on top of `pinocchio`.

## Repo defaults

- **Always run commands inside `devenv shell`** so the nix-managed toolchain (cargo, mdt, dprint, clippy, etc.) is used instead of stale cargo-installed or system binaries.
- Use `devenv` for the development shell and repo task runner.
- Use `cargo` for workspace tasks; use `pnpm` only for JS/Codama subprojects.
- Format with `fix:format` or `dprint fmt`; do not run `rustfmt` directly.
- Workspace code must preserve `no_std` compatibility where applicable.
- `unsafe_code` and `unstable_features` are denied workspace-wide.

## Contribution conventions

- GitHub issue titles must be written in title case (e.g. `Add Miri Coverage for Account Loader Aliasing Rules`). Do not use commit-style prefixes like `fix:` / `feat:` / `docs:` in issue titles.
- Pull request titles must follow Conventional Commits (e.g. `feat(loaders): preserve borrow guard lifetime`).
- Never merge a `chore(release): prepare release` pull request. Release pull requests must remain open until Ifiok Jr. (`@ifiokjr`) explicitly decides to merge them himself.

## Common commands

- `devenv shell` — enter the dev environment
- `install:all` — install pinned cargo binaries and external tools
- `cargo build --all-features` — build the workspace
- `cargo test` — run the default test suite
- `build:pina:no-default` — verify `pina` across no-default feature subsets
- `lint:all` — run clippy, formatting, and docs verification
- `verify:docs` — validate reusable docs and mdBook output
- `fix:format` — format files and re-sync mdt-managed docs

## Task-specific guidance

- [Build and tooling](./docs/agents/build-and-tooling.md)
- [Coding style guide](./docs/agents/coding-style.md) — visual organization, whitespace patterns, and code aesthetics
- [Workspace architecture](./docs/agents/workspace-architecture.md)
- [Testing and SBF builds](./docs/agents/testing-and-sbf.md)
- [Release process and changesets](./docs/agents/release-and-changesets.md)
- [Git workflow](./docs/agents/git-workflow.md)
- [Security and code constraints](./docs/agents/security-and-code-constraints.md)
