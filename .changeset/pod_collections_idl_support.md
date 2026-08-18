---
pina_cli: feat
pina_codama_renderer: fix
pina: fix
---

Add semantic `PodString<N, PFX>` and `PodVec<T, N, PFX>` support to the IDL generator. Generated Rust clients now retain zeropod collection types and recursively validate their prefixes, UTF-8, and active elements instead of treating the fields as arbitrary bytes. Local contiguous `#[derive(PodEnum)]` companions are emitted as defined enum types; layouts that Codama cannot represent faithfully fail generation instead of falling back to public keys.

Encode account and instruction discriminators in generated clients, map signed Pod numeric elements at their real sizes, preserve generic capacity parameters during IDL extraction, reject noncanonical `PodString` length prefixes, initialize discriminators in typed account-creation helpers, and run the profile program's real SBF lifecycle in CI.
