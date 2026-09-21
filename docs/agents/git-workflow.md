# Git Workflow

## Commit signing

- Sign with the OpenPGP key `60CD779496945399`. It is registered on the GitHub account as a signing key, so your commits show as Verified.
- Never change the signing configuration to work around a signing error — that is how this repository's history acquired permanently Unverified commits. The global rule and the meaning of each signing error live in the `git-workflow` skill; read it before touching `gpg.format`, `user.signingkey`, or `commit.gpgsign`.
- Run `git verify-commit <sha>` before pushing. GitHub records a commit's verification state when it is pushed and does not retroactively re-verify, so a bad signature stays Unverified on `main` permanently.

## Branches and messages

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
- GitHub issue titles must be written in title case. Do not use commit-style prefixes like `fix:` / `feat:` / `docs:` in issue titles.
- Open a pull request for review before merging.
- Link pull requests to the relevant issue(s).
