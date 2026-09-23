---
pina_cli: none
---

# Stop the deploy stdin test racing a startup deadline

The test that proves `solana` inherits a closed stdin polled the child against a three-second deadline, which Pina's `cargo metadata` project discovery exceeded on a loaded runner. It now waits for the marker the fake `solana` writes after reading EOF and keeps a thirty-second deadline purely as a hang bound. Test-only; no release note.
