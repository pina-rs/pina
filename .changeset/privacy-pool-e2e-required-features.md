---
pina: none
---

# Build privacy pool host tests without the prover feature

`cargo test -p privacy_pool_program` failed to compile: the SBF end-to-end suite in `tests/e2e.rs` imports the host-only `prover` module and names `ark_groth16` types, and both exist only when the `prover` feature is enabled. The workspace test sweep never saw it, because the privacy pool's Surfpool crate depends on the program with `prover` and feature unification switched it on for every workspace build.

The `e2e` test target now declares `required-features = ["prover"]`, so a plain host run skips the suite and the documented `--features prover --test e2e -- --include-ignored` invocation still builds and runs it. The program's dependencies, SBF artifact, and compute units are unchanged.
