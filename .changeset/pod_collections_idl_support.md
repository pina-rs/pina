---
pina_cli: feat
pina_codama_renderer: fix
pina: fix
---

Add semantic zeropod `String<N, PFX>` and `Vec<T, N, PFX>` support to the IDL generator. Generated Rust clients retain native schema types and recursively validate their generated storage views instead of treating collection fields as arbitrary bytes. Contiguous native enums deriving `ZeroPod` are emitted as defined enum types; layouts Codama cannot represent faithfully fail generation instead of falling back to public keys.

Encode account and instruction discriminators in generated clients, map signed Pod numeric elements at their real sizes, preserve generic capacity parameters during IDL extraction, reject noncanonical `PodString` length prefixes, initialize discriminators in typed account-creation helpers, and run the profile program's real SBF lifecycle in CI.

Generated JavaScript codecs now reject collection values that exceed their fixed capacity instead of truncating them. Decoders validate discriminators and canonical booleans, decode UTF-8 strictly, and preserve embedded NUL characters so their accepted wire format matches Pina's on-chain zeropod views.
