---
pina_cli: fix
pina: none
---

# Retry the lint driver probe while the kernel reports it busy

`pina lint` starts the driver it just installed to check that it loads. The kernel refuses an `exec` while a write descriptor for the same file is still open — including one a concurrent fork inherited — and the install-then-probe sequence can overlap with that window, so the probe failed with a busy error on a driver that was perfectly runnable. The spawn now retries briefly for that one error and fails immediately for anything else, which also stops the driver tests from flaking under a parallel test run.

`pina` carries no behavior change in this release line; the entry records the added test coverage for the public `into_discriminator!` macro.
