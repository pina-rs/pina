---
pina_cli: feat
---

Add `PodString<N, PFX>` and `PodVec<T, N, PFX>` support to the IDL generator: collection fields now map to fixed-size Codama nodes (`FixedSizeTypeNode(BytesTypeNode, N + PFX)` and `ArrayTypeNode(T, N)`) instead of falling back to public-key nodes. `type_to_string` now preserves generic arguments so capacity parameters survive IDL extraction. Add a `profile_program` example demonstrating Pod collections in a full program lifecycle with mollusk-svm end-to-end tests.
