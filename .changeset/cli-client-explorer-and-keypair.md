---
pina_codama_renderer_cli: fix
---

# Fix CLI Explorer links and keypair validation

The Dart CLI renderer printed an Explorer URL without a cluster selector, so a signature from a devnet or testnet run linked to the mainnet Explorer page. It now derives the selector from the resolved endpoint and appends `?cluster=devnet` or `?cluster=testnet`. Mainnet Beta and arbitrary custom endpoints stay bare, because the Explorer only understands the named clusters and a guessed selector for an unknown host would point at the wrong chain. The match is on the parsed hostname rather than a substring, so a custom endpoint that merely contains a cluster's host as a path or subdomain is not mislabeled.

Both renderers also accepted a keypair array of 64 values without checking the values themselves. `Uint8Array` and `Uint8List` wrap out-of-range bytes rather than rejecting them, so a malformed keypair file produced a different signer instead of an error — one that would simply fail every transaction with no explanation. Each byte must now be an integer from 0 to 255, and anything else fails with the existing keypair-loading error.
