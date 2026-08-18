# Codama renderer parity

The repository renderer accepts only Codama layouts that can be represented as fixed-size native zeropod schemas. Unsupported or ambiguous layouts fail with a contextual error instead of silently falling back to a public key or byte array.

## Generated model

- Accounts, instruction wire schemas, events, and defined structs derive `pina::ZeroPod`.
- Generated account parsers return `AccountZc` after exact-length, discriminator, and recursive zeropod validation.
- Generated account initializers zero caller-owned storage, set the discriminator, and return `&mut AccountZc`.
- Generated instruction data owns a private zeroed `Vec<u8>`. A closure configures its generated storage view; validation runs before the buffer can be moved into a Solana instruction.
- Native scalar fields remain native in schema declarations. Zeropod selects their little-endian alignment-one storage representations.
- Fixed-capacity strings, vectors, options, nested structs, and contiguous native enums retain semantic zeropod types rather than becoming opaque bytes.

## Supported types

| Codama node                   | Native Rust schema                        | Constraints                                               |
| ----------------------------- | ----------------------------------------- | --------------------------------------------------------- |
| Little-endian integer         | `u8`…`u128`, `i8`…`i128`                  | Fixed-width only                                          |
| Boolean                       | `bool`                                    | One-byte zeropod encoding                                 |
| Public key                    | `solana_pubkey::Pubkey`                   | 32 bytes                                                  |
| Fixed bytes/array             | `[T; N]`                                  | Supported fixed-size element                              |
| Fixed-size UTF-8 string       | `pina::String<N>` or explicit prefix form | Prefix and capacity must agree                            |
| Prefixed fixed-capacity array | `pina::Vec<T, N>` or explicit prefix form | Fixed-size, recursively validated element                 |
| Struct defined type           | Native `#[derive(pina::ZeroPod)]` struct  | Every field supported                                     |
| Scalar enum defined type      | Native `#[derive(pina::ZeroPod)]` enum    | Unit variants, unsigned repr, contiguous values from zero |
| Defined type link             | Generated native type                     | Target must resolve                                       |

The contiguous-enum restriction is deliberate: the stock JavaScript renderer does not preserve arbitrary enum discriminant values. Rejecting sparse values keeps Rust, JavaScript, and on-chain encodings identical.

## Discriminators

Constant numeric discriminators at offset zero are required for account and instruction nodes. They are emitted both as metadata/constants and as actual schema fields so every generated client includes the discriminator byte in the wire format.

## PDA seeds and account defaults

The renderer supports fixed string/byte, little-endian number, boolean, and public-key PDA seeds. Instruction accounts support public-key, program-ID, and linked-program defaults. Optional accounts preserve Codama's explicit omitted or program-ID placeholder strategy.

## Rejected layouts

- variable-length strings, bytes, arrays, maps, or sets;
- remainder or sentinel encodings;
- big-endian and floating-point numbers;
- sparse enums or enums with payload variants;
- unresolved user-defined types;
- fixed-size wrappers whose semantic meaning cannot be recovered;
- non-zero-offset or size-derived discriminators.

Every accepted account and instruction layout must have one exact byte size and one validation path. This is intentionally narrower than the complete Codama schema language.
