---
pina: feat
pina_abi: feat
pina_cli: feat
pina_codama_renderer: none
pina_macros: feat
---

# Accept typed fixed arrays `[T; N]` in zero-copy schemas

Schema fields can now declare typed fixed arrays instead of byte blobs with hand-rolled endian packing:

```rust
#[account(discriminator = LootboxKind)]
pub struct Rewards {
	pub outcome_weights: [u64; 8],
	pub mints: [Address; 4],
	pub flags: [bool; 2],
	pub nested: [[u8; 4]; 2],
	pub maybe: Option<[u64; 2]>,
}
```

Storage is `[PodT; N]` little-endian with no length prefix — `[u64; 8]` stores `[PodU64; 8]` exactly as the compiler lays it out, so existing byte-for-byte wire expectations hold and clients read plain little-endian integers. Validation recurses per element, so restricted-domain elements such as `PodBool` are checked individually, and nested arrays compose. `[u8; N]` keeps its identity mapping and its bytes node in generated IDLs.

The grammar change is recursive: the element is classified by the same closed rules as any other fixed field, which means typed arrays accept audited scalars, `Address`, `PodU*`/`PodI*`/`PodBool`, fixed-point types, and nested arrays, while still rejecting `char`, `NonZero*`, custom `ZcField` mappings, and non-literal lengths with a pointed diagnostic (`[T; N]` array elements must be fixed Pina schema types). Compact accounts accept typed arrays as inline header fields, and generated patch builders take native or pod spellings (`.weights([5, 6])` and `.weights([PodU64::from(5), PodU64::from(6)])` both compile) through a new `IntoPodArray` conversion that mirrors the existing `IntoPodOption`.

`pina_cli` maps typed arrays to fixed-count Codama `ArrayTypeNode`s and computes their sizes element-wise, and `pina_abi`'s migration manifest physical layout now sizes `[T; N]` by composing the element size with the literal length (previously only `[u8; N]` was sized, and nested arrays were mis-split at the first `;`). This release requires `pinapod` 0.4.0, which is where the generalized `ZcElem`, `ZcValidate`, and `ZcField` array impls live.
