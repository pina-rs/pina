---
pina_cli: fix
---

# Respect manifest overflow checks in the size profile

The default `SizeProfile::Production` sets `overflow-checks = false` as Cargo environment overrides, and environment overrides beat `[profile.release]` in a manifest. A program that deliberately set `overflow-checks = true` therefore had the setting silently replaced with wrapping arithmetic on its next `pina build`, turning a loud failure into a wrong result.

`pina build` now reads `[profile.release].overflow-checks` from the workspace manifest that owns the build. An explicit `true` outranks the size profile: the checks stay enabled and the command warns that it gave up part of the profile. A silent manifest, or one that agrees with disabling the checks, behaves exactly as before. `--overflow-checks` still forces them on.

`pina build --verify` no longer applies profile overrides at all. A verified artifact must stay reproducible from its recorded Git revision, and an override that exists only on the command line cannot be reproduced from that revision. Declare the profile under `[profile.release]` — the layout `pina init` writes — so the ordinary and verified backends compile the same artifact. Pina warns when a requested profile would make the two artifacts differ, naming the missing setting.
