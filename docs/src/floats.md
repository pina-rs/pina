# Float and Fixed-Point Fields

Pina schemas can store fractional numbers two ways, each behind its own optional feature:

- `floats` enables native `f32` and `f64` fields, converted to and from their bit pattern under the hood through the `PodF32`/`PodF64` pods that `pinapod` provides, and
- `fixed` enables fixed-point `FixedI*<Frac>` and `FixedU*<Frac>` types from the pinned [`fixed`](https://crates.io/crates/fixed) crate, re-exported as `pina::fixed`.

Both forms are stored as the complete bit pattern of a backing little-endian integer pod. Every bit pattern is a valid stored value, so zero-copy reads stay total and validation stays O(1) per field.

The features are independent: enabling `floats` alone does not pull in the `fixed` crate, and enabling `fixed` alone does not provide `f32`/`f64` fields. Enable what the program actually stores:

```toml
[dependencies]
pina = { version = "...", features = ["floats"] } # f32/f64 fields
pina = { version = "...", features = ["fixed"] } # FixedI*/FixedU* fields
pina = { version = "...", features = ["floats", "fixed"] } # both
```

Either feature also enables `derive`, so no separate dependency entries are needed.

## Native float fields

With `floats` enabled, declare `f32` and `f64` exactly like any other scalar. The generated zero-copy view converts through `pinapod`'s `PodF32` and `PodF64` storage pods, so accessors take and return native floats — the same ergonomics as `u32` fields converting through `PodU32`:

```rust
use pina::*;

#[account(discriminator = ReadingKind::Live)]
pub struct LiveReading {
	pub temperature: f32,
	pub depth: f64,
	pub bias: Option<f32>,
	pub samples: Vec<f32, 4>,
}

#[instruction(discriminator = ReadingKind::Calibrate)]
pub struct Calibrate {
	pub offset: f64,
}
```

Storage accessors convert automatically:

- `state.temperature.set(-12.5)` stores `(-12.5f32).to_bits()` little-endian.
- `state.temperature.get()` returns the decoded `f32`.
- `Option<f32>` and `Vec<f32, N>` compose with the standard bounded-collection grammar.
- Compact accounts accept `f32`/`f64` as inline header fields, and compact patches take native floats. Both spellings work for optional fields: `patch.maybe_bias(Some(0.5))` and `patch.maybe_bias(None)` need no pod import, because `pinapod` implements the field mapping for the `f32` primitive itself.

## Fixed-point fields

Fixed-point types represent fractional values as scaled integers: a `FixedU64<U16>` stores value × 2¹⁶ in a `u64`. Declare them with a `typenum` fractional-bits parameter, which Pina re-exports through `pina::fixed`:

```rust
use pina::fixed::FixedU64;
use pina::fixed::types::extra::U16;
use pina::*;

#[account(discriminator = PriceKind::Live)]
pub struct LivePrice {
	pub price: FixedU64<U16>,
	pub authority: Address,
}
```

Fixed-point fields expose their backing bits through the standard pod accessors. Convert at the boundary with `from_bits`/`to_bits`, or construct exact values with `from_num`:

```rust
let price = FixedU64::<U16>::from_num(3);          // exactly 3.0
state.price.set(price.to_bits());
let decoded = FixedU64::<U16>::from_bits(state.price.get());
assert_eq!(decoded.to_bits(), 3 << 16);
```

Valid `Frac` widths depend on the backing width; a `FixedU64<Frac>` supports 0–64 fractional bits. The `fixed` crate rejects invalid parameters at compile time and provides `checked_*`, `saturating_*`, and `wrapping_*` arithmetic alongside the default operators.

## Wire format and generated clients

Both families are stored as the complete bit pattern of a backing little-endian integer:

| Schema type       | Storage   | Wire bytes                   |
| ----------------- | --------- | ---------------------------- |
| `f32`             | `PodF32`  | `f32::to_bits()` as `u32` LE |
| `f64`             | `PodF64`  | `f64::to_bits()` as `u64` LE |
| `FixedI8<Frac>`   | `i8`      | 1 byte                       |
| `FixedI16<Frac>`  | `PodI16`  | 2 bytes                      |
| `FixedI32<Frac>`  | `PodI32`  | 4 bytes                      |
| `FixedI64<Frac>`  | `PodI64`  | 8 bytes                      |
| `FixedI128<Frac>` | `PodI128` | 16 bytes                     |
| `FixedU8<Frac>`   | `u8`      | 1 byte                       |
| `FixedU16<Frac>`  | `PodU16`  | 2 bytes                      |
| `FixedU32<Frac>`  | `PodU32`  | 4 bytes                      |
| `FixedU64<Frac>`  | `PodU64`  | 8 bytes                      |
| `FixedU128<Frac>` | `PodU128` | 16 bytes                     |

Generated Codama clients describe these fields as their backing little-endian integers (`u32`, `u64`, `i64`, and so on). The IDL describes the wire, and the wire is an integer bit pattern; a client that wants a float divides by 2^Frac (fixed-point) or converts the bits itself (IEEE-754). This keeps every existing Rust, TypeScript, and Dart client working without float codec support.

## Choosing between fixed-point and floating point

Prefer fixed-point for anything that touches value, accounting, or consensus-sensitive math:

| Concern              | Fixed-point (`Fixed*`)                                                        | Native float (`f32`/`f64`)                                                     |
| -------------------- | ----------------------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| Determinism          | Exact integer arithmetic; every validator agrees                              | IEEE-754 semantics with rounding at every operation                            |
| Representable values | Exactly `value × 2^Frac` within range                                         | Dense near zero, gappy at magnitude; no exact decimals                         |
| Overflow behavior    | Same as Rust integers: panic in debug, wrap in release; `checked_*` available | Drifts toward infinity; silently loses precision                               |
| Solana compute units | Plain integer instructions                                                    | SBF has no hardware float unit: every operation lowers to soft-float sequences |
| Client ergonomics    | Divide by 2^Frac (an exact shift)                                             | Reconstruct IEEE bits by hand in every client                                  |

Use native floats when the values are inherently approximate sensor-style readings, when the math must match an off-chain float model bit-for-bit, or when interoperating with a program that already stores raw float bits (for example, Anchor's `floats` pattern, which Pina's `float_accounts_program` example ports). In that case store the bits and treat them as opaque: comparisons and aggregation belong off-chain, where the float model is defined.

Solana execution is deterministic across validators, but IEEE-754 does not make cross-platform float math trivially reproducible (FMA contraction, libm differences in transcendentals, and NaN payload propagation all vary). Fixed-point avoids the entire class: integer addition is integer addition everywhere.

## Build impact

The two features have very different costs, which is part of why they are separate:

- **`floats` adds no crates.** `PodF32`/`PodF64` are four- and eight-byte wrappers in `pinapod`, which Pina already depends on. A program that enables the feature without using it links nothing extra, and the `pina` rlib stays byte-identical (609,304 bytes when measured).
- **`fixed` adds two `no_std` crates** to the dependency graph — `fixed =1.30.0` and its only dependency `typenum`. Compiling them costs about 9.5 seconds once, and about 0.4 seconds on a cached rebuild. A program that only stores `f32`/`f64` fields can leave this out entirely.
- The `fixed` crate is heavily generic: its own rlib is large (about 28 MB with debug metadata), but only the fixed-point types a program actually instantiates are monomorphized into the final SBF binary. A program that uses one `FixedU64<U16>` field pays for exactly that instantiation.
- The exact `=1.30.0` pin is deliberate: Pina's generated code and pinapod's `ZcField` implementations expand against `fixed`'s type-level contracts, so mixed versions cannot be allowed to coexist. Pina re-exports the pinned instance as `pina::fixed`; derive your schema fields from that path and the version question disappears.

## Security notes

- **Every bit pattern is a valid value.** Unlike `char` or `NonZero*`, no float or fixed-point bit pattern is an invalid state, so zero-copy casts remain sound and validation cannot reject a legitimate value.
- **Zeroed storage is value zero, not an invalid account.** The typed discriminator still guards account identity: a fully zeroed account fails validation until `write_discriminator` runs. After initialization, an unset `Option<f32>` reads as `None` and a zeroed fixed-point reads as `0.0`.
- **NaN is a value, not an error.** Float storage preserves bit patterns exactly, including NaN payloads, and `PodF32`/`PodF64` compare bitwise so `Eq` stays sound. On-chain float arithmetic is still the author's responsibility: `NaN == NaN` is false in native float math, and validators cannot detect a program that treats NaN inconsistently.
- **Use checked arithmetic for value-bearing state.** Fixed-point operators panic on overflow in debug builds and wrap in release, matching Rust integer semantics. Prefer `checked_add`/`checked_sub` (or the pod's `checked_*` helpers on the backing integer pods) in any path that moves value.
- **Float precision is a protocol decision.** Two floats written by the same instruction read back identically; only arithmetic introduces rounding. Keep multi-step float computation off-chain or in fixed-point, and use on-chain floats as stored configuration or recorded observations.
- **Clients see integers.** A TypeScript or Dart client that naively renders a fixed-point field shows the raw scaled integer. Scale on the client with an exact divisor derived from the documented `Frac` parameter.
