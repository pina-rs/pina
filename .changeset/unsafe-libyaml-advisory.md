---
pina_test: none
---

# Suppress the unmaintained unsafe-libyaml advisory

[RUSTSEC-2024-0449](https://rustsec.org/advisories/RUSTSEC-2024-0449) flags `unsafe-libyaml` as unmaintained, reached through `pina_test → surfpool-sdk → surfpool-core → serde_yaml`. The advisory database picked the entry up on 2026-10-12, which failed the `cargo-deny` CI gate for every open pull request. Host/test stack only; the suppression is tracked in #619 and is removable once surfpool drops the deprecated `serde_yaml`.
