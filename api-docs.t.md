<!-- {@pinaPublicResultContract} -->

All APIs in this section are designed for on-chain determinism.

They return `ProgramError` values for caller-side propagation with `?`.

No panics needed.

<!-- {/pinaPublicResultContract} -->

<!-- {@pinaValidationChainSnippet} -->

Validation methods are intentionally chainable: `account.assert_signer()?.assert_writable()?.assert_owner(&program_id)?`.

<!-- {/pinaValidationChainSnippet} -->

<!-- {@pinaPdaSeedContract} -->

Seed-based APIs require deterministic seed ordering.

Program IDs must stay consistent across derivation and verification.

When a bump is required, prefer canonical bump derivation.

Use explicit bumps when needed.

<!-- {/pinaPdaSeedContract} -->

<!-- {@pinaTokenFeatureGateContract} -->

This API is gated behind the `token` feature. Keep token-specific code behind `#[cfg(feature = "token")]` so on-chain programs that do not use SPL token interfaces can avoid extra dependencies.

<!-- {/pinaTokenFeatureGateContract} -->

<!-- {@pinaMdtManagedDocNote} -->

This section is synchronized by `mdt` from `api-docs.t.md`.

<!-- {/pinaMdtManagedDocNote} -->
