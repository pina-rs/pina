---
pina: feat
pina_test: breaking
---

# Move the host test stack to Surfpool 1.7 and Agave 4.3

`pina_test` anchors every host-side Solana dependency, so the Surfpool train decides the Agave train the whole workspace resolves against. Surfpool 1.7 ships `litesvm` 0.18 on Agave 4.3, and the workspace moves with it:

- `surfpool-sdk` `~1.6` → `~1.7`
- `solana-client`, `solana-rpc-client`, and `solana-runtime` `~4.2` → `~4.3`
- `mollusk-svm` 0.15.1 → 0.16.4, the Mollusk release built on Agave 4.3
- every other tilde anchor moves to the minor the train resolves: `solana-account` `~4.7`, `solana-address` `~2.7`, `solana-clock` `~3.2`, `solana-epoch-rewards` `~3.2`, `solana-epoch-schedule` `~3.3`, `solana-fee-calculator` `~3.3`, `solana-hash` `~4.6`, `solana-instruction` `~3.5`, `solana-last-restart-slot` `~3.2`, `solana-message` `~4.6`, `solana-pubkey` `~4.3`, `solana-rent` `~4.4`, `solana-reward-info` `~7.0`, `solana-short-vec` `~3.3`, `solana-signature` `~3.5`, `solana-slot-history` `~3.2`, `solana-stake-history` `~1.1`, `solana-transaction` `~4.3`, and `solana-transaction-error` `~3.4`

The resolver proves out a single Agave 4.3.2 island. The multi-version crates that remain are the program-side ones `pinocchio` and the examples depend on, which were already split.

Surfpool 1.7 cannot resolve beside `pinapod` 0.4.7. `surfpool-core` 1.7 pulls `phoenix-rise-math`, which pins `fixed =1.29.0`, while `pinapod` pinned `=1.30.0`, and Agave 4.3 requires `solana-address` 2.7 or newer while `pinapod` capped it at `<2.7`. Neither bound reflected a real incompatibility. `pinapod` 0.4.8 widens them to `^1.29.0` and `>=2.2, <3.0`, and `pinapod` 0.4.9 implements `FromZc` for `PodBool`, which the 0.4.8 companion surface requires of a schema that declares the pod directly. This change moves the workspace pin to 0.4.9, so downstream projects resolve through published releases rather than a repository patch.

`pinapod` 0.4.9's derive also adds the companion value surface that shipped with it: every generated `*Zc` view now implements `PartialEq`, `Eq`, `Hash`, and `Debug` over active content, and `to_native()` / `FromZc` materialize a fully owned value from it. `pina_root`'s expansion snapshots record the added impls, and the two shadowing UI fixtures now pin the new `FromZc` rejection alongside the existing type mismatch. The wire format, validation order, and instruction paths are unchanged.

The devenv `ifiokjr-nixpkgs` pin moves to the revision that packages the Surfpool 1.7.0 CLI, so the installed `surfpool` and the SDK agree; the same revision keeps `agave` at 4.2, which the SBF build toolchain uses independently. `@solana/surfpool` moves to `^1.7.0` for the Codama end-to-end suite.

The upgrade removes thirteen advisory exceptions from `deny.toml`. Surfpool 1.7 resolves `reqwest` 0.12 and `rustls` 0.23, so the `reqwest` 0.11 → `rustls` 0.21 → `rustls-webpki` 0.101 chain, its `webpki-roots` 0.25 roots, `rustls-pemfile`, the `hyper` 0.14 `h2` advisory, `atty`, `instant`, `memmap2` 0.5, `proc-macro-error2`, and the mismatched `rand` advisory all disappear from the graph (#621). `unused-ignored-advisory = "deny"` now fails the gate when an exception stops matching, so the list cannot drift again. The remaining exceptions are the ones the train still carries: `unsafe-libyaml` through Surfpool's deprecated `serde_yaml` (#619), the `crypto-bigint` timing advisory (#654), and the guarded `zstd` advisories (#669).

`OfflineSurfnet::start` drops its `BlockProductionMode::Clock` override. Surfpool 1.6.0 never confirmed transactions on the SDK's default transaction-triggered block production; 1.7.0 restores it ([surfpool#814](https://github.com/solana-foundation/surfpool/issues/814)), so embedded tests run on the default again and see only the slots their own transactions produce instead of a block every clock tick.

Surfpool 1.7 also applies mainnet rent rates, which charge 5,080 lamports per grown byte against the 6,960 that `Rent::default()` computes. A `HistoricalAccount` funded at the host default therefore held a surplus the runtime never charged, which silently hid the migration budget check: both accounts migrated for free and the shared-budget test passed a transaction it expected to reject. `HistoricalAccount::new` still records the default-rate estimate, but installation now resolves the rent-exempt minimum through the running instance unless `with_lamports` supplied an explicit balance. The migrations example's cap moves from 24,000 to 18,000 lamports: the mainnet-rate `ManualState` ladder needs 15,240, and one step across two `State` slots costs 20,320, so the example keeps both the funded ladder and the shared-budget rejection that the surfpool suite asserts. A runtime on Agave's older rent schedule needs a larger cap, and the example documents that.
