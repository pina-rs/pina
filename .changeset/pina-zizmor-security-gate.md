---
pina: none
pina_lints: none
pina_macros: none
pina_codama_nodes: none
pina_codama_renderer: none
pina_profile: none
pina_sdk_ids: none
pina_skill: none
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

# Gate zizmor workflow auditing into the security check

Zizmor now audits every workflow and composite action as part of `verify:security`, with a zero-findings policy recorded in `.github/zizmor.yml`. Checkouts persist credentials only where authentication is required, the compute-unit report status is passed through `env:` instead of template expansion, and Dependabot updates gain a seven-day cooldown. The `taiki-e/install-action` pin is refreshed to its latest release hash.
