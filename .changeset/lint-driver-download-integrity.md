---
pina_cli: fix
---

# Verify the lint driver download before installing it

`pina lint` fetches a prebuilt driver binary, marks it executable, and runs it as its version probe. The download trusted the transport alone: no checksum was verified, the response body was read without a size bound, and the origin could be redirected through `PINA_LINT_DRIVER_*` environment variables.

The download path now fetches the `sha256` published beside the asset, compares it against the bytes received, and refuses to install on mismatch, a malformed checksum, or a checksum file that is not valid UTF-8. A new `VerifiedDriver` type wraps the bytes so only a checksum-verified download can reach the installer — a future edit cannot silently reintroduce an unverified path, because the compiler rejects it. The body read is capped at 256 MiB, and the release workflow now uploads the standalone driver's `.sha256` beside the binary, which it previously did not (only the archives carried published checksums).

A locally built driver (`pina lint --build-driver`) keeps its existing trust root — a cargo build against the pinned `pina_lints` release — and installs through a separate function, since no published checksum exists for an artifact built on the local machine.
