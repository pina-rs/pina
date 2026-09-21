---
pina: none
---

# Enable overflow checks in release builds

The workspace root now sets `overflow-checks = true` on `[profile.release]`. Rust release builds wrap silently on integer overflow by default, and the examples build for SBF through this profile; every value-bearing path already uses checked arithmetic and the `require_checked_asset_arithmetic` lint keeps it that way, but the profile is the workspace-wide backstop for the paths that regress — the Cetus class. Derived programs should copy the setting into their own root manifest, which the profile's comment says in place.
