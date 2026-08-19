---
core: major
---

# Bind Anchor Realloc Samples to Their Authority

`examples/anchor_realloc` is now a secure, intentionally non-ABI-compatible adaptation of Anchor's test fixture. It adds `Initialize` (discriminator `2`) and creates a per-authority sample PDA at `[b"sample", authority]`. `Realloc` still uses discriminator `0`, but its authority must now be writable and a signer, its sample must be initialized at the canonical PDA, and its `len` includes the 34-byte authenticated `Sample` header.

`Realloc2` (discriminator `1`) no longer resizes two arbitrary accounts. It validates both authenticated targets and returns `AccountDuplicateReallocs` before mutation, matching Anchor's duplicate-reallocation regression intent.

This removes the previous capability for an unrelated signer to resize an arbitrary writable account owned by the program. Regenerate Codama clients and initialize the sample before calling `Realloc`.
