---
pina_cli: fix
---

# Report the resolved lint driver's own toolchain

`pina doctor --json` reported `lintDriver.expectedToolchain` as the pinned nightly the shipped lints are developed against, even when a driver had resolved for a different negotiated toolchain. A cached, downloaded, or source-built driver exists for the active compiler revision by construction, so an otherwise healthy diagnostic looked mismatched and sent consumers toward unnecessary toolchain changes (#458).

The report now emits `lintDriver.resolvedToolchain`, naming the toolchain the resolved driver was actually built for: the active toolchain for a cached, downloaded, or source-built driver, the shipped nightly for a bundled one — which is honest even when it differs from the active compiler, because the load probe already proved the bundle compatible — and nothing for a `PINA_LINT_DRIVER_PATH` override or when no driver resolved. The `expectedToolchain` key keeps its name, value, and meaning.

Human output renames the `expected toolchain:` line to `shipped-lint toolchain:` so it states what the constant actually means rather than reading like a requirement on the project, and prints `resolved toolchain:` under the resolved driver.
