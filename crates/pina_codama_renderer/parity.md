# Codama Renderer Parity

<br>

This document tracks which Codama IDL node types the `pina_codama_renderer` supports. The renderer produces fixed-size `#[repr(C)]` structs with `bytemuck::Pod` and `bytemuck::Zeroable` derives, so only node types that map to a known, constant-size layout are supported.

## Supported Type Nodes

<br>

| Codama Type                        | Rust Output                     | Notes                              |
| ---------------------------------- | ------------------------------- | ---------------------------------- |
| `NumberTypeNode` (u8)              | `u8`                            | Little-endian only                 |
| `NumberTypeNode` (i8)              | `i8`                            | Little-endian only                 |
| `NumberTypeNode` (u16)             | `pina_pod_primitives::PodU16`   | Little-endian only                 |
| `NumberTypeNode` (i16)             | `pina_pod_primitives::PodI16`   | Little-endian only                 |
| `NumberTypeNode` (u32)             | `pina_pod_primitives::PodU32`   | Little-endian only                 |
| `NumberTypeNode` (i32)             | `pina_pod_primitives::PodI32`   | Little-endian only                 |
| `NumberTypeNode` (u64)             | `pina_pod_primitives::PodU64`   | Little-endian only                 |
| `NumberTypeNode` (i64)             | `pina_pod_primitives::PodI64`   | Little-endian only                 |
| `NumberTypeNode` (u128)            | `pina_pod_primitives::PodU128`  | Little-endian only                 |
| `NumberTypeNode` (i128)            | `pina_pod_primitives::PodI128`  | Little-endian only                 |
| `BooleanTypeNode`                  | `pina_pod_primitives::PodBool`  | Must be little-endian u8-sized     |
| `PublicKeyTypeNode`                | `solana_pubkey::Pubkey`         | 32-byte fixed size                 |
| `FixedSizeTypeNode<BytesTypeNode>` | `[u8; N]`                       | Fixed-size byte arrays only        |
| `FixedSizeTypeNode<T>`             | `[T; N]`                        | Wraps any supported inner type     |
| `ArrayTypeNode` (fixed count)      | `[T; N]`                        | Only with `FixedCountNode`         |
| `StructTypeNode`                   | `struct { ... }`                | All fields must be supported types |
| `DefinedTypeLinkNode`              | `crate::generated::types::Name` | Links to other defined types       |

## Supported Discriminator Nodes

<br>

| Codama Type                                              | Rust Output                          | Notes                                                                |
| -------------------------------------------------------- | ------------------------------------ | -------------------------------------------------------------------- |
| `ConstantDiscriminatorNode` (number type + number value) | Typed constant (e.g. `u8`, `PodU16`) | Must be at offset 0; little-endian only; u8-u128 and i8-i128 formats |

## Supported Value Nodes (constant seeds and default values)

<br>

| Codama Type              | Context                                          | Notes                                        |
| ------------------------ | ------------------------------------------------ | -------------------------------------------- |
| `StringValueNode`        | PDA constant seeds                               | Rendered as `"...".as_bytes()`               |
| `NumberValueNode`        | PDA constant seeds, discriminator values         | Rendered as `N_type.to_le_bytes()` for seeds |
| `PublicKeyValueNode`     | PDA constant seeds, instruction account defaults | Rendered as `solana_pubkey::pubkey!("...")`  |
| `BytesValueNode` (UTF-8) | PDA constant seeds                               | Rendered as `"...".as_bytes()`               |
| `ConstantValueNode`      | PDA constant seeds                               | Delegates to inner string or number value    |

## Supported Instruction Input Value Nodes (account defaults)

<br>

| Codama Type          | Rust Output                     | Notes                                 |
| -------------------- | ------------------------------- | ------------------------------------- |
| `PublicKeyValueNode` | `solana_pubkey::pubkey!("...")` | Hardcoded public key default          |
| `ProgramIdValueNode` | `crate::PROGRAM_ID`             | Uses the primary program constant     |
| `ProgramLinkNode`    | `crate::LINKED_PROGRAM_ID`      | References another program in the IDL |

## Supported PDA Seed Nodes

<br>

| Codama Type                             | Rust Output                      | Notes                                                     |
| --------------------------------------- | -------------------------------- | --------------------------------------------------------- |
| `ConstantPdaSeedNode`                   | Constant byte expression         | Supports string, number, public key, bytes (UTF-8) values |
| `VariablePdaSeedNode` (PublicKey)       | `&solana_pubkey::Pubkey` param   | `key.as_ref()` for seeds                                  |
| `VariablePdaSeedNode` (Boolean)         | `bool` param                     | Must be little-endian u8; `&[u8::from(val)]`              |
| `VariablePdaSeedNode` (Number)          | Primitive int param (e.g. `u64`) | `val.to_le_bytes()` for seeds; little-endian only         |
| `VariablePdaSeedNode` (FixedSize bytes) | `&[u8; N]` param                 | `&val[..]` for seeds                                      |
| `VariablePdaSeedNode` (Fixed array)     | `&[T; N]` param                  | `&val[..]` for seeds                                      |

## Supported Program-Level Nodes

<br>

| Codama Type                | Rust Output                                                       | Notes                                                                                                     |
| -------------------------- | ----------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| `RootNode`                 | Entire generated crate                                            | Orchestrates all sub-renders                                                                              |
| `ProgramNode`              | `programs.rs` with `pub const PROGRAM_ID: Pubkey`                 | Supports multiple programs (primary + additional)                                                         |
| `AccountNode`              | `accounts/<name>.rs` with `#[repr(C)]` Pod struct                 | Includes `from_bytes`, `from_bytes_mut`, `TryFrom<AccountInfo>`, `LEN`, and `new` constructor             |
| `InstructionNode`          | `instructions/<name>.rs` with accounts struct + data struct       | Discriminator is required; generates `instruction()` and `instruction_with_remaining_accounts()` builders |
| `DefinedTypeNode` (struct) | `types/<name>.rs` with `#[repr(C)]` Pod struct                    | Full struct with constructor                                                                              |
| `DefinedTypeNode` (link)   | `types/<name>.rs` with `pub type Alias = ...`                     | Type alias to another defined type                                                                        |
| `DefinedTypeNode` (other)  | `types/<name>.rs` with `pub type Alias = ...`                     | Type alias to rendered Pod type                                                                           |
| `ErrorNode`                | `errors/<program>.rs` with `#[derive(Error, FromPrimitive)]` enum | Hex-formatted error codes; `From<Error> for ProgramError` impl                                            |
| `PdaNode`                  | `find_pda()` and `create_pda()` methods on accounts               | Only rendered when an `AccountNode` links to a PDA                                                        |
| `InstructionAccountNode`   | Field on instruction accounts struct                              | Supports `is_signer` (true/false/either), `is_writable`, `is_optional`                                    |

## Unsupported Type Nodes

<br>

| Codama Type                       | Reason                                                                             |
| --------------------------------- | ---------------------------------------------------------------------------------- |
| `StringTypeNode`                  | Variable-length; POD layout requires fixed size                                    |
| `BytesTypeNode` (unwrapped)       | Must be wrapped in `FixedSizeTypeNode` to have a known size                        |
| `NumberTypeNode` (f32)            | Floating-point types are not POD-compatible with bytemuck                          |
| `NumberTypeNode` (f64)            | Floating-point types are not POD-compatible with bytemuck                          |
| `NumberTypeNode` (ShortU16)       | Solana compact-u16 encoding is variable-length                                     |
| `NumberTypeNode` (big-endian)     | Only little-endian number types are supported                                      |
| `BooleanTypeNode` (non-u8 size)   | Booleans must be encoded as little-endian u8                                       |
| `ArrayTypeNode` (prefixed count)  | Prefixed-length arrays are variable-size; not POD                                  |
| `ArrayTypeNode` (remainder count) | Remainder arrays are variable-size; not POD                                        |
| `AmountTypeNode`                  | Not yet implemented in the renderer                                                |
| `DateTimeTypeNode`                | Not yet implemented in the renderer                                                |
| `EnumTypeNode`                    | Not yet implemented in the renderer                                                |
| `MapTypeNode`                     | Variable-size collection; not POD                                                  |
| `OptionTypeNode`                  | Variable-size optional; not POD (use `ZeroableOptionTypeNode` instead if possible) |
| `SetTypeNode`                     | Variable-size collection; not POD                                                  |
| `TupleTypeNode`                   | Not yet implemented in the renderer                                                |
| `HiddenPrefixTypeNode`            | Not yet implemented in the renderer                                                |
| `HiddenSuffixTypeNode`            | Not yet implemented in the renderer                                                |
| `PostOffsetTypeNode`              | Not yet implemented in the renderer                                                |
| `PreOffsetTypeNode`               | Not yet implemented in the renderer                                                |
| `RemainderOptionTypeNode`         | Not yet implemented in the renderer                                                |
| `SentinelTypeNode`                | Not yet implemented in the renderer                                                |
| `SizePrefixTypeNode`              | Variable-size; not POD                                                             |
| `SolAmountTypeNode`               | Not yet implemented in the renderer                                                |
| `ZeroableOptionTypeNode`          | Not yet implemented in the renderer                                                |

## Unsupported Discriminator Nodes

<br>

| Codama Type                                    | Reason                                                    |
| ---------------------------------------------- | --------------------------------------------------------- |
| `FieldDiscriminatorNode`                       | Only `ConstantDiscriminatorNode` at offset 0 is supported |
| `SizeDiscriminatorNode`                        | Only `ConstantDiscriminatorNode` at offset 0 is supported |
| `ConstantDiscriminatorNode` (offset != 0)      | Discriminators must be at byte offset 0                   |
| `ConstantDiscriminatorNode` (non-number type)  | Discriminator type must be a `NumberTypeNode`             |
| `ConstantDiscriminatorNode` (non-number value) | Discriminator value must be a `NumberValueNode`           |
| `ConstantDiscriminatorNode` (f32/f64/ShortU16) | Float and ShortU16 discriminator formats are unsupported  |

## Unsupported Value Nodes (for constant seeds)

<br>

| Codama Type                  | Reason                                 |
| ---------------------------- | -------------------------------------- |
| `BytesValueNode` (non-UTF-8) | Only UTF-8 byte seeds are supported    |
| `ArrayValueNode`             | Not supported as a constant seed value |
| `BooleanValueNode`           | Not supported as a constant seed value |
| `EnumValueNode`              | Not supported as a constant seed value |
| `MapValueNode`               | Not supported as a constant seed value |
| `NoneValueNode`              | Not supported as a constant seed value |
| `SetValueNode`               | Not supported as a constant seed value |
| `SomeValueNode`              | Not supported as a constant seed value |
| `StructValueNode`            | Not supported as a constant seed value |
| `TupleValueNode`             | Not supported as a constant seed value |

## Unsupported Instruction Account Default Values

<br>

| Codama Type            | Reason                                                                               |
| ---------------------- | ------------------------------------------------------------------------------------ |
| `AccountValueNode`     | Only `PublicKeyValueNode`, `ProgramIdValueNode`, and `ProgramLinkNode` are supported |
| `AccountBumpValueNode` | Only `PublicKeyValueNode`, `ProgramIdValueNode`, and `ProgramLinkNode` are supported |
| `ArgumentValueNode`    | Only `PublicKeyValueNode`, `ProgramIdValueNode`, and `ProgramLinkNode` are supported |
| `ConditionalValueNode` | Only `PublicKeyValueNode`, `ProgramIdValueNode`, and `ProgramLinkNode` are supported |
| `IdentityValueNode`    | Only `PublicKeyValueNode`, `ProgramIdValueNode`, and `ProgramLinkNode` are supported |
| `PayerValueNode`       | Only `PublicKeyValueNode`, `ProgramIdValueNode`, and `ProgramLinkNode` are supported |
| `PdaValueNode`         | Only `PublicKeyValueNode`, `ProgramIdValueNode`, and `ProgramLinkNode` are supported |
| `ResolverValueNode`    | Only `PublicKeyValueNode`, `ProgramIdValueNode`, and `ProgramLinkNode` are supported |

## Limitations

<br>

- **Fixed-size only:** The renderer exclusively produces `#[repr(C)]` structs compatible with `bytemuck::Pod`. Any Codama type that implies a variable-length layout will be rejected with an error.
- **Little-endian only:** All numeric types and boolean sizes must use little-endian byte order, which is the native Solana byte order.
- **Discriminator required for instructions:** Every `InstructionNode` must have a `ConstantDiscriminatorNode` at offset 0. Account discriminators are optional (omitted if absent).
- **No enum rendering:** `EnumTypeNode` is not yet supported. Programs with enum-typed fields will fail to render.
- **No option rendering:** `OptionTypeNode` and `ZeroableOptionTypeNode` are not yet rendered. Optional fields in account data are unsupported.
- **No tuple rendering:** `TupleTypeNode` is not yet rendered. Tuple-typed fields will fail.
- **No float types:** `f32` and `f64` number formats are rejected everywhere (fields, discriminators, seeds).
- **No ShortU16:** The Solana compact-u16 encoding (`ShortU16`) is variable-length and rejected.
- **PDA seeds are limited:** Only string, number (little-endian), public key, fixed-size bytes (UTF-8 encoding only for `BytesValueNode`), and fixed arrays are supported as PDA seed types.
- **Instruction account defaults are limited:** Only hardcoded public keys (`PublicKeyValueNode`), the program's own ID (`ProgramIdValueNode`), and linked program IDs (`ProgramLinkNode`) can be used as default values for instruction accounts.
- **No CPI invoke helpers:** The renderer generates `solana_instruction::Instruction` builders but does not generate CPI invoke/invoke_signed wrappers.
- **No event support:** While the renderer handles accounts, instructions, defined types, and errors, there is no dedicated event rendering. Events would need to be modeled as defined types.
- **Single-file-per-entity output:** Each account, instruction, defined type, and error enum is rendered into its own file under `src/generated/`.
