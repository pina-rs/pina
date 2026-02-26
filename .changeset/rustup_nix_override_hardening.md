---
default: note
---

Harden the rustup nix override to fix intermittent CI failures caused by rustup 1.28+ requiring a `version` field in `settings.toml` during shell completion generation in the install phase.
