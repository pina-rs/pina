---
pina_cli:
  bump: minor
  type: feat
pina_codama_nodes:
  bump: minor
  type: feat
pina_cli_npm:
  bump: minor
  type: feat
pina_cli_darwin_arm64:
  bump: minor
  type: feat
pina_cli_darwin_x64:
  bump: minor
  type: feat
pina_cli_freebsd_x64:
  bump: minor
  type: feat
pina_cli_linux_arm64_gnu:
  bump: minor
  type: feat
pina_cli_linux_arm64_musl:
  bump: minor
  type: feat
pina_cli_linux_x64_gnu:
  bump: minor
  type: feat
pina_cli_linux_x64_musl:
  bump: minor
  type: feat
pina_cli_win32_arm64_msvc:
  bump: minor
  type: feat
pina_cli_win32_x64_msvc:
  bump: minor
  type: feat
pina_skill:
  bump: minor
  type: feat
pina_macros:
  bump: patch
  type: fix
---

# Make Pina Tooling Self-Describing

Add comprehensive CLI help, stable machine-readable IDL output, bundled documentation discovery, and a complete mdBook CLI reference. Publish prebuilt CLI packages for every release target, rename the Codama helpers to `@pina-rs/codama-nodes`, and add the installable `@pina-rs/skill` agent guide. Split macro expansion code into focused modules without changing generated output.
