---
pina_macros: fix
---

# Drop pina_macros' dead dev-dependencies

The pina_root migration moved every pina_macros test out of the crate but left four dev-dependencies behind: `insta`, `pina`, `prettyplease`, and `trybuild`. The remaining `pina` requirement deadlocked crates.io publication: monochange publishes `pina_macros` before `pina`, while `cargo publish --locked` must resolve the rewritten dev-dependency `pina ^0.12.1`, which does not exist until `pina` publishes later in the same run. The one remaining unit test only references `pina::AccountView` inside `syn::parse_quote!` token streams and never links against the crate, so the whole section goes away.
