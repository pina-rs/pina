---
pina: patch
---

Upgrade the example dev-dependency `mollusk-svm` from 0.11.0 to 0.15.0 and `solana-account` from ^3 to ^4, eliminating RUSTSEC-2026-0097 (unsound `rand` 0.7.3) and RUSTSEC-2026-0173 (unmaintained `proc-macro-error2`) from the example test dependency graph. Also updates `jiff`, `defmt`, `env_logger`, `solana-logger`, and `crossbeam-epoch` to patched versions.
