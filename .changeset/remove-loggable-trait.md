---
pina: major
---

**BREAKING**: Removed the `Loggable` trait. This trait had no implementations anywhere in the codebase and was dead code. If you were depending on this trait, define your own logging trait or use `log!` / `sol_set_return_data` directly.
