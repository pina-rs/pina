---
pina_cli: fix
---

Fix sha2 0.11 digest formatting in test code.

The #255 dependency update migrated the two lib call sites but missed nine `format!("{:x}", Sha256::digest(..))` uses across `pina_cli` test modules, where sha2 0.11 digest arrays no longer implement `LowerHex`. The Windows CLI portability job and any compilation of the test targets therefore failed. Hash formatting now maps bytes explicitly, producing identical output.
