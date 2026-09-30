---
pina_cli: test
pina: none
---

# Isolate CLI test fixture commits from global git config

The verified-build tests in `crates/pina_cli/tests/project_commands.rs` commit a throwaway fixture repository. Those commits read the developer's global git config, so with `commit.gpgsign = true` every fixture commit was signed through the developer's gpg-agent. Under the parallel load of the pre-push gate the agent failed with `gpg: signing failed: Cannot allocate memory`, and the gate blocked the push while the same test passed on its own.

The fixture commands now run with `GIT_CONFIG_GLOBAL=/dev/null` and `GIT_CONFIG_NOSYSTEM=1`, so no global or system setting reaches them and a `--global` write fails instead of editing the developer's `~/.gitconfig`. The fixture keeps its own repository-local identity. `git_environment_isolation_preserves_external_config` now also injects a global and system config that signs through a failing program, and asserts the verified-build cases pass and leave that config unchanged. Removing either override makes it fail.
