---
pina_cli: minor
---

Add automated release workflow for pina CLI binary distribution.

Register `pina_cli` as a knope-managed package with versioning, changelogs, and GitHub releases. Add a GitHub Actions workflow that builds and uploads cross-platform binaries when a `pina_cli` release is created. Supports 9 target platforms: Linux (GNU/musl, x86_64/aarch64), macOS (x86_64/aarch64), Windows (x86_64/aarch64), and FreeBSD (x86_64). Each binary includes SHA512 checksums for verification.
