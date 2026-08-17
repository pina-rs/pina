---
pina_cli: feat
pina: fix
---

Add `PodString<N, PFX>` and `PodVec<T, N, PFX>` support to the IDL generator: collection fields now map to fixed-size byte nodes that preserve their length prefixes and backing storage instead of falling back to public-key nodes. Encode account and instruction discriminators in generated clients, map signed Pod numeric elements at their real sizes, and preserve generic capacity parameters during IDL extraction. Reject noncanonical `PodString` length prefixes, initialize discriminators in typed account-creation helpers, and run the profile program's real SBF lifecycle in CI.
