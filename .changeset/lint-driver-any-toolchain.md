---
pina_cli: feat
pina_lints: chore
---

# Make the lint driver work on any toolchain

`pina lint` is no longer coupled to one exact nightly. A lint driver links the compiler's unstable `rustc_private` crates, so a driver only loads against the exact compiler revision it was built with: two dated nightlies that share a release line expose incompatible compiler libraries. Pina shipped one driver built for one pinned nightly, so a project on any other nightly could not lint at all, and a CLI installed with `cargo install pina_cli` got no driver because the prebuilt binary only travelled with the release archives.

`pina lint` now reads `rustc -vV` to identify the active compiler and resolves a driver for it, in order: `PINA_LINT_DRIVER_PATH`, a driver already cached for that commit hash, the driver bundled next to the CLI when it loads, and a download from the Pina release matching the CLI version. Asking a candidate driver to start _is_ the version check — a driver built for another compiler cannot load `librustc_driver` — so a driver that starts is by construction the right one, and one built for another nightly is skipped instead of failing deep inside cargo with a loader exit.

The cache is keyed by host triple and full compiler commit hash, so two nightlies coexist and a cached driver is never reused for a compiler it was not built with. Release drivers are published as standalone assets named `pina-lint-driver-<host>-<commit-hash>`, which makes the asset name the negotiation: a project on any other nightly asks for a name the release does not publish and gets a clear miss rather than a binary it cannot load. bitflip and kickjump can drop their toolchain and nixpkgs pins.

Two commands close the remaining gaps. `pina lint --build-driver` compiles the driver from the `pina_lints` release matching the CLI using the project's own toolchain, which covers a nightly Pina publishes no artifact for; it needs the `rustc-dev` and `rust-src` components and installs into the cache so later runs skip cargo entirely. `pina doctor` reports the active toolchain, the expected release, the resolved driver and how it was obtained, both search paths, and the one-line remedy when nothing matched — without downloading, because a diagnostic that populates a cache cannot be run to find out what is wrong. Every successful `pina lint` line names the driver that ran.

Blessing a deliberate exception now has a documented path. `pina lint --explain <LINT>` prints one lint's contract, why violating it is a vulnerability, and the sanctioned way to bless the pattern, and the same entries ship as a generated docs page whose sync with the CLI table is checked in `verify:docs`. Only lints with a documented exception suggest an `#[allow]`; the rest name the API or restructure that satisfies the contract.

The orphan `crates/pina_lints/src/lints/require_empty_before_init.rs` is deleted. The typed creation builders have enforced emptiness since the retirement of that lint, which removed it from the catalog and left only the unreferenced source file behind. The readme's lint catalog table, which had drifted a lint behind the registered set, is complete again and a test now keeps it that way.
