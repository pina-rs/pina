---
pina: patch
---

Release and publishing pipeline hardening updates:

- Added a `docs-pages` GitHub Actions workflow that builds mdBook docs and deploys them to GitHub Pages on each published release.
- Tightened CI defaults by reducing workflow permissions to read-only where write access is not required.
- Updated CI test coverage to run `cargo test --all-features --locked` for closer release parity.
- Updated the pinned `knope` tool version to `0.22.3` so `knope` commands validate and run reliably in this toolchain.
