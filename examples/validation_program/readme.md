# `validation_program`

<!-- {=pinaValidationExampleGuide} -->

## Complete Boundary-Validation Example

The `examples/validation_program` project uses the feature across every supported macro boundary:

| Boundary             | Example coverage                                                                             |
| -------------------- | -------------------------------------------------------------------------------------------- |
| Instruction data     | Numeric bounds, bounded strings, exact vector lengths, custom errors, and a cross-field hook |
| Instruction accounts | Signer, writable, owner, program, empty, non-empty, distinct-account rules, and struct hooks |
| Stored account state | Numeric bounds and a hook that keeps the minimum no greater than the maximum                 |
| Events               | Numeric, string, and vector constraints plus a hook that rejects duplicate approvals         |

The processor also keeps one policy rule explicit because it combines decoded instruction data with loaded account state. That distinction is intentional: annotations validate one received value or account list, while ordinary Rust remains the clearest place for rules spanning multiple boundaries.

Run its native and deployed-program tests from the repository root:

```bash
devenv shell -- cargo test -p validation_program
devenv shell -- pina test --project examples/validation_program
```

The existing `events_program` also enables `validation` and applies event rules without changing its transport-focused structure. It is the smaller reference for adding validation to an established program.

<!-- {/pinaValidationExampleGuide} -->
