---
pina_cli: fix
---

# Document the lint-driver origin overrides as operator-only

`PINA_LINT_DRIVER_RELEASE`, `_REPO`, and `_BASE_URL` redirect the driver download and its checksum together, so they are a trusted-origin control rather than a convenience knob: whatever serves the override chooses which binary the CLI runs. The constants now say so, which is the documentation half of the 2026-09-21 audit's supply-chain finding; the version-pinned `npx` renderer fetches stay registry-trust-by-design.
