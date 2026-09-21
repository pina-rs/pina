---
pina_test: feat
---

# Move the host stack to Surfpool 1.6 / Agave 4.2

`pina_test` anchored every host-side Solana dependency on the Agave 4.1 train because Surfpool 1.5 capped `litesvm` at `^0.14.0`, and that release does not compile against Agave 4.2. Surfpool 1.6 ships `litesvm` 0.16 on Agave 4.2, so the cap is gone and the workspace moves up with it:

- `surfpool-sdk` `~1.5` → `~1.6`
- `solana-client`, `solana-rpc-client`, and `solana-runtime` `~4.1` → `~4.2`
- `mollusk-svm` 0.14.0 → 0.15.1, which is the Mollusk release built on Agave 4.2

Every other tilde range in the crate already accommodated the newer train, so nothing else moved by hand. The resolver now proves out a single `solana-runtime` 4.2.2 island; the remaining multi-version crates (`solana-message`, `solana-transaction`, `solana-fee-calculator`) are the program-side crates that `pinocchio` and the examples depend on and were already split before this change. The devenv `ifiokjr-nixpkgs` pin moves to the revision that packages `surfpool` 1.6.0 so the CLI and the SDK agree; that revision keeps agave at 4.2.2.

`OfflineSurfnet::start` now selects `BlockProductionMode::Clock`. Surfpool 1.6.0's SDK defaults to `BlockProductionMode::Transaction`, and on an embedded offline instance that mode never confirms a submitted transaction: the client's confirmation loop polls `getSignatureStatuses` indefinitely and the test hangs with no error. The same suite passes in about five seconds in clock mode. This is a Surfpool regression rather than a Pina defect and is reported upstream as [surfpool#814](https://github.com/solana-foundation/surfpool/issues/814); the override comes out once that is fixed.

The upgrade also removes a documented limitation. Surfpool 1.5 could not derive CPI signers for PDAs with four or more seed arguments, so any program seeding a PDA that way could not run its Surfpool suite end to end. On 1.6.0 the vesting example's `[b"vesting", admin, beneficiary, mint]` plus bump PDA completes `Initialize` → `Claim` → `Cancel`, which the previous train could not reach. `crates/pina_test/readme.md` now records the fix instead of the caveat.

One unrelated transitive dependency is held back. `pinapod` 0.4.2 changes the code `#[derive(PodCompact)]` generates: it moves `writer.commit()` ahead of the inline writes and turns the length check that followed into a returned `InvalidLength`. Committing before those writes makes the captured length describe the buffer one step early, so the new check can fire on a legitimate write, and the `pina_root` macro-expansion snapshots no longer match. That release is reported as [pinapod#36](https://github.com/pina-rs/pinapod/issues/36); Pina stays on `pinapod` 0.4.1 until it is resolved.
