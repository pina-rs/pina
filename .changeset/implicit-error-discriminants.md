---
pina_cli: breaking
---

# Record implicit `#[error]` codes in IDLs

`pina_cli::parse::error_enum::extract_error_enums` now returns `Result<Vec<ErrorIr>, syn::Error>` instead of `Vec<ErrorIr>`, like the other source extractors, and it rejects a discriminant it cannot evaluate. Library callers must handle the `Result`, for example with `?` or by mapping the `syn::Error` into their own error type. Programs whose `#[error]` enums use an expression such as `BASE + 1` as a discriminant must switch to an integer literal before `pina idl` or `pina generate` will extract them.

IDL extraction recorded every `#[error]` variant without an integer-literal discriminant as code `0`. For `enum E { A = 6000, B }` the IDL listed `B` as `0`, although the program returns `6001` for it, so generated clients decoded the error as the wrong variant or not at all.

Extraction now follows Rust's rule: a variant without a discriminant takes the previous variant's code plus one, starting at zero. A discriminant pina cannot evaluate now fails extraction with an error that names the variant instead of being recorded as a guess.
