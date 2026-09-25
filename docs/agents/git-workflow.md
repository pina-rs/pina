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

## Pushing over SSH

- `git push` opens the SSH connection and reads the remote's ref advertisement before it runs the pre-push hook, so the connection is idle for the whole `lint:push` gate. GitHub closes a connection that has been idle for six minutes; the gate runs longer, so the push dies with exit 141 (SIGPIPE) after the gate has already passed and pushes nothing. `fix:format` and `devenv shell` do not repair it.
- `devenv:git-hooks:install` sets `core.sshCommand` to `ssh -o ServerAliveInterval=15 -o ServerAliveCountMax=30` for the checkout, which holds the connection open. If a checkout sets its own `core.sshCommand`, the task prints a note instead of overwriting it — add the same options there.
- Treat a push that exits 141 as "the gate passed and the ref did not move", never as a failed gate. Check the ref with `git ls-remote origin refs/heads/<branch>` and re-run the push. Do not reach for `--no-verify`: it skips the gate rather than working around the transport.
