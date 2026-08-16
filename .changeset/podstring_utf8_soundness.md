---
pina_pod_primitives: fix
---

Remove the unsound `Deref<Target = str>` and `AsRef<str>` impls from `PodString`. Because `PodString` is `bytemuck::Pod`, bytes loaded from untrusted account data may not be valid UTF-8, and the removed impls produced a `&str` via `from_utf8_unchecked` — undefined behavior. Use `try_as_str()` for validated access, or `as_str_unchecked()` (unsafe) for unchecked access.
