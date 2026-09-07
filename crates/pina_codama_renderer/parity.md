# Codama renderer parity

The repository renderer accepts Codama layouts that can be represented as native PinaPod schemas. Most schemas remain fixed-size. Accounts can also end in a suffix of supported compact fields. Unsupported or ambiguous layouts fail with a contextual error instead of silently falling back to a public key or byte array.

## Generated model

- Accounts, instruction wire schemas, events, and defined structs derive `pina::PinaPod`.
- Generated fixed account parsers return `AccountZc` after exact-length, discriminator, and recursive PinaPod validation.
- Generated compact account parsers return scoped `AccountRef` views after header, active-tail, and discriminator validation. Generated `AccountPatch` values describe atomic updates.
- Generated account initializers zero caller-owned storage, set the discriminator, and return the matching fixed view or apply a checked compact patch.
- Generated instruction data owns a private zeroed `Vec<u8>`. A closure configures its generated storage view; validation runs before the buffer can be moved into a Solana instruction.
- Native scalar fields remain native in schema declarations. PinaPod selects their little-endian alignment-one storage representations.
- Fixed-capacity strings, vectors, options, nested structs, and contiguous native enums retain semantic PinaPod types rather than becoming opaque bytes.

## Supported types

| Codama node                     | Native Rust schema                        | Constraints                                               |
| ------------------------------- | ----------------------------------------- | --------------------------------------------------------- |
| Little-endian integer           | `u8`…`u128`, `i8`…`i128`                  | Fixed-width only                                          |
| Boolean                         | `bool`                                    | One-byte PinaPod encoding                                 |
| Public key                      | `solana_pubkey::Pubkey`                   | 32 bytes                                                  |
| Fixed bytes/array               | `[T; N]`                                  | Supported fixed-size element                              |
| Fixed-size UTF-8 string         | `pina::String<N>` or explicit prefix form | Prefix and capacity must agree                            |
| Prefixed fixed-capacity array   | `pina::Vec<T, N>` or explicit prefix form | Fixed-size, recursively validated element                 |
| Trailing prefixed account array | compact `pina::Vec<T, N>`                 | `N` is declared capacity and must fit the selected prefix |
| Struct defined type             | Native `#[derive(pina::PinaPod)]` struct  | Every field supported                                     |
| Scalar enum defined type        | Native `#[derive(pina::PinaPod)]` enum    | Unit variants, unsigned repr, contiguous values from zero |
| Defined type link               | Generated native type                     | Target must resolve                                       |

The contiguous-enum restriction is deliberate: the stock JavaScript renderer does not preserve arbitrary enum discriminant values. Rejecting sparse values keeps Rust, JavaScript, and on-chain encodings identical.

## Discriminators

Constant numeric discriminators at offset zero are required for account and instruction nodes. They are emitted both as metadata/constants and as actual schema fields so every generated client includes the discriminator byte in the wire format.

Generated Rust instruction builders write the framework discriminator after the caller's configuration closure, so callers cannot emit a different instruction variant. JavaScript and Dart clients produced by `pina generate` validate discriminators, canonical values, UTF-8, prefixes, and declared compact capacities.

## PDA seeds and account defaults

The renderer supports fixed string/byte, little-endian number, boolean, and public-key PDA seeds. Instruction accounts support public-key, program-ID, and linked-program defaults. Optional accounts preserve Codama's explicit omitted or program-ID placeholder strategy.

## Rejected layouts

- variable-length strings, bytes, maps, or sets, and variable arrays outside the compact suffix of an account;
- remainder or sentinel encodings;
- big-endian and floating-point numbers;
- sparse enums or enums with payload variants;
- unresolved user-defined types;
- fixed-size wrappers whose semantic meaning cannot be recovered;
- non-zero-offset or size-derived discriminators.

Every accepted layout has one validation path. Fixed accounts and all instruction or event layouts have one exact byte size. Compact accounts have a fixed header, bounded tails, and a generated patch API. Pina intentionally supports less than the complete Codama schema language.
