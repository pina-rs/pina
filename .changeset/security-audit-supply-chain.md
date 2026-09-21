---
pina_cli: fix
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

# Document the lint-driver origin overrides as operator-only

`PINA_LINT_DRIVER_RELEASE`, `_REPO`, and `_BASE_URL` redirect the driver download and its checksum together, so they are a trusted-origin control rather than a convenience knob: whatever serves the override chooses which binary the CLI runs. The constants now say so, which is the documentation half of the 2026-09-21 audit's supply-chain finding; the version-pinned `npx` renderer fetches stay registry-trust-by-design.
