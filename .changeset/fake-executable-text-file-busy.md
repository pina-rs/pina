---
pina_cli: test
---

# Write test executables without a Text file busy race

`codama::tests::client_runner_reports_spawn_and_renderer_failures` failed intermittently on Linux CI: spawning its fake renderer returned `ETXTBSY` ("Text file busy"), so the error no longer contained the renderer's output. Linux refuses to `exec` a file that any process holds open for writing. The test wrote the script itself, and a concurrent `fork` on another test thread inherited the write descriptor until that child called `exec`, so the script could still have a writer when it ran. Every test that wrote a fake `node`, `npx`, `cargo`, `solana`, or verifier and then ran it shared the race, including the lint fixture that wrote to a staging file and renamed it, because the kernel checks the file rather than its name.

`crates/pina_cli/tests/support/mod.rs` now provides `write_executable`, which writes the script through a short-lived `/bin/sh`, so the test binary never holds a descriptor for the file. The unit tests include the same module, every fake executable in `pina_cli`'s tests goes through it, and the symlinked `tests/fixtures/shell-driver.sh` workaround it replaces is removed. The lint driver tests are unchanged: driver selection already retries a busy executable.
