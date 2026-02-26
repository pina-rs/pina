---
pina: patch
pina_cli: patch
---

Fix broken doc comments produced by `mdt` template expansion. The line-prefix mode was emitting `-->//` instead of `-->` followed by `///`, and blank lines inside reusable doc blocks were missing the `///` prefix. This caused rustdoc warnings and broken documentation rendering.

Also simplifies a raw string literal in `pina_cli` init templates and shortens a fully-qualified `std::result::Result::ok` path to `Result::ok`.
