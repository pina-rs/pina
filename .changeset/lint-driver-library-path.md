---
pina_cli: breaking
pina_lints: breaking
---

# ship the lint driver prebuilt with pina

Stop building and installing the security-lint driver per user. The lints are statically linked into `pina_lint_driver`, which now ships prebuilt next to the `pina` CLI in every release archive and npm platform package; `pina lint` resolves the driver beside its own executable and runs it directly. The release pipeline builds the driver with the pinned nightly from `rust-toolchain.toml` on runners whose architecture matches the release target, because `rustc-dev` ships compiler libraries only for a toolchain's own host, so targets without a matching runner ship CLI-only archives.

Remove the `cargo install pina_lints` machinery, the Cargo-home driver cache keyed by the `rustc -vV` fingerprint, and the Dylint-compatible cdylib crate type and protocol symbols: nothing loads lint libraries dynamically anymore. The CLI still prepends the active toolchain's sysroot library directory to the dynamic-library search path and starts the bundled driver once before the lint run, so a project whose toolchain differs from the pinned nightly fails fast with an error naming the required release instead of a loader exit deep inside cargo. Treat empty `CARGO` environment values like unset variables. `PINA_LINT_DRIVER_PATH` continues to point local lint development at a workspace-built driver; the `security:pina-lint` task exports the same library path for its direct `cargo check` invocation.
