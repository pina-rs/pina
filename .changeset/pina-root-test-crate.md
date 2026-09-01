---
pina: none
pina_cli: none
pina_codama_renderer: none
pina_lints: none
pina_macros: none
pina_profile: none
pina_root: none
pina_sdk_ids: none
pina_test: none
pina_codama_nodes: none
pina_cli_npm: none
pina_skill: none
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

# Establish pina_root internal test crate

The workspace root becomes an internal, never-published `pina_root` crate hosting cross-crate tests through the public macro API. All 47 `pina_macros` tests migrate from internal `expand()` calls to behavioral tests through public macro invocations. The `pina_macros` dev-dependency on `pina` is removed, dissolving the publish-order cycle. The workspace range hack is deleted.
