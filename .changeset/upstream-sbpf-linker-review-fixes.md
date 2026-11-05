---
pina_cli: fix
---

# Address the sbpf-linker PR review findings

Corrects the example documentation (`Clock::from_account_view`, not the unimplemented `sol_get_sysvar` syscall), uses `std::env::join_paths` for the linker PATH prefix so the separator matches the platform, prints the captured compiler diagnostics before returning `BuildError::CargoFailed` so upstream-build failures are diagnosable, and removes the process-wide env-var-mutating routing test in favor of the existing child-process integration test that covers the same path safely.
