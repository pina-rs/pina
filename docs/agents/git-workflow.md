# Git Workflow

- Create a dedicated branch for each change before committing.
- Use branch names with conventional prefixes, for example:
  - `feat/<description>`
  - `fix/<description>`
  - `docs/<description>`
  - `test/<description>`
  - `refactor/<description>`
  - `ci/<description>`
  - `build/<description>`
  - `chore/<description>`
- Do not use the `codex/` branch prefix.
- Commit messages must follow Conventional Commits, for example `fix(loaders): preserve borrow guard lifetime`.
- Pull request titles must also follow Conventional Commits. Prefer using the eventual squash-merge commit title as the PR title.
- GitHub issue titles must be written in sentence case. Do not use commit-style prefixes like `fix:` / `feat:` / `docs:` in issue titles.
- Open a pull request for review before merging.
- Link pull requests to the relevant issue(s).
