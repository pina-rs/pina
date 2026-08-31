---
pina: none
pina_lints: none
pina_macros: none
pina_codama_nodes: none
pina_codama_renderer: none
pina_profile: none
pina_sdk_ids: none
pina_skill: docs
pina_cli_npm: none
pina_cli_darwin_arm64: none
pina_cli_darwin_x64: none
pina_cli_freebsd_x64: none
pina_cli_linux_arm64_gnu: none
pina_cli_linux_arm64_musl: none
pina_cli_linux_x64_gnu: none
pina_cli_linux_x64_musl: none
pina_cli_win32_arm64_msvc: none
pina_cli_win32_x64_msvc: none
---

# Configure codecov from the repository root

The new root-level `codecov.yml` scopes coverage accounting to the unit-testable crates. It is a repository configuration file, so every package is affected without requiring a version bump. The agent-skill testing and project-setup references gain the Surfpool runtime limitations and the delegated SBF build toolchain.
