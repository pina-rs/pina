---
pina: breaking
pina_cli: breaking
---

# Align CPI invocation methods and checks

Split CpiContext execution into invoke(data) and invoke_signed(data, signers), update generated CPI templates, and extend the arbitrary-CPI lint to cover invoke_with_program and invoke_signed_with_program without accepting unrelated program validation.

This is a breaking change for code that uses `CpiContext` directly:

```rust
// Before: every invocation accepted a signer slice.
context.invoke(data, signers)?;

// After: transaction-level signatures need no signer argument.
context.invoke(data)?;

// PDA-authorized calls use the explicitly signed method.
context.invoke_signed(data, signers)?;
```

Generated CPI modules created by `pina init` now use the signed method internally. The security lint also recognizes the dual-program token helpers, requiring the exact program argument passed to `invoke_with_program` or `invoke_signed_with_program` to have a preceding address validation rather than accepting a check on an unrelated program account. Validation is tracked by HIR identity across every control-flow path and invalidated by reassignment, so shadowed bindings and non-dominating checks cannot authorize a CPI.
