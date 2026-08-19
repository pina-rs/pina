---
pina_cli: feat
pina_codama_renderer: fix
pina: fix
---

Add semantic zeropod `String<N, PFX>`, `Vec<T, N, PFX>`, and fixed `Option<T>` parsing to the IDL toolchain for legacy IDLs and advanced direct zeropod integrations. Pina's audited account, instruction, and event macros accept only scalar `Option<T>` and reject string/vector, explicit `PodOption`, enum, custom, and nested layouts. Generated clients for supported Pina schemas therefore use scalar options and fully initialized fixed byte arrays; layouts Codama cannot represent faithfully fail generation instead of falling back to public keys.

Encode account and instruction discriminators in generated clients, map signed Pod numeric elements at their real sizes, preserve generic capacity parameters during IDL extraction, reject noncanonical `PodString` length prefixes, initialize discriminators in typed account-creation helpers, and run the profile program's real SBF lifecycle in CI.

Generated JavaScript codecs validate discriminators, canonical booleans, and scalar-option tags. Advanced collection codecs reject values that exceed fixed capacity, decode UTF-8 strictly, and preserve embedded NUL characters, but those collection layouts are outside Pina's macro-generated schema contract.
