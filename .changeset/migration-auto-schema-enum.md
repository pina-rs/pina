---
pina_abi: fix
---

# Enumerate the auto policy's kind names in the ABI document schema

`MigrationAuto` has hand-written serde impls, so its JSON Schema is hand-written too, and it described its items as any string. A consumer validating a manifest against the published schema therefore admitted `["states"]` while the reader rejected it with `unknown migration kind` — the schema promised more than the code accepted, which defers the failure to runtime.

The items now carry the valid names, read from `ContractKind::config_name`, the same source `from_config_name` resolves through, so the schema and the reader cannot drift apart. The checked-in artifacts under `crates/pina_abi/schemas/`, the frozen fixtures, and the published copies under the book are regenerated to match, and a test asserts agreement in both directions: every name the schema advertises decodes, and every spelling the reader rejects stays unadvertised.
