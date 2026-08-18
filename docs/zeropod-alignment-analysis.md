# Zeropod integration and safety boundary

_Updated after the native-schema redesign in the PR #192 follow-up to PR #195._

## Architecture

Pina uses zeropod's normal derive model rather than implementing a parallel zero-copy system:

```rust
#[account(discriminator = ProfileAccountType)]
pub struct ProfileState {
	pub bump: u8,
	pub name: pina::String<32>,
	pub tags: pina::Vec<u64, 8>,
	pub active: bool,
}
```

The source struct is a native schema. `zeropod::ZeroPod` generates `ProfileStateZc`, whose fields have alignment-one storage representations. Pina's account loaders validate the runtime byte slice and return a borrow of that generated view. Native integers and booleans remain native in the schema; callers use `.get()` / `.set()` on their generated storage fields.

## Why there is no `to_bytes()`

Zeropod's fixed-capacity string, vector, and option storage may leave inactive capacity uninitialized. Reading the complete object representation as a byte slice would therefore be undefined behavior even though all active values are valid. Alignment and absence of padding do not prove that every byte has been initialized.

Pina consequently supports one direction at the library boundary:

```text
initialized runtime bytes -> zeropod validation -> borrowed TypeZc view
```

It does not support:

```text
schema or TypeZc object representation -> borrowed byte slice
```

The inactive portion of a collection is not part of its semantic value and must remain unobservable.

## Initialization and testing

Every account, instruction, and event schema has an `initialize` helper. The caller supplies an exact-size mutable byte slice; Pina fills the entire slice with zero, writes the discriminator, asks zeropod for the checked mutable view, and returns that borrow:

```rust,ignore
let mut storage = vec![0u8; ProfileState::SIZE];
{
	let profile = ProfileState::initialize(&mut storage)?;
	profile.bump = bump;
	profile.name.try_set("alice")?;
	profile.active.set(true);
}

// The view borrow has ended. `storage` can now be installed as account data or
// passed to the test runtime.
```

This is ergonomic without constructing a native schema value, reading an object representation, or adding a serializer to Pina.

Generated off-chain clients necessarily create the final `Vec<u8>` required by Solana's `Instruction`. They allocate that buffer as zeroed storage, configure a private generated zeropod view, validate it, and move the buffer directly into the instruction. They do not expose a general-purpose `to_bytes()` API.

## Ownership of unsafe code

Zeropod owns the unsafe traits and byte-to-view pointer conversions. Pina's schema macros derive `zeropod::ZeroPod`; Pina does not manually implement `ZcElem`, `ZcValidate`, or `ZeroPodFixed`, and it does not provide a second generic casting helper.

Pina is responsible for the surrounding checks:

- exact account or instruction length;
- expected discriminator;
- runtime borrow-guard lifetime;
- owner, signer, writable, and PDA validation;
- calling zeropod validation before returning a typed view.

## Verification requirements

The integration is covered at several boundaries:

- compile-time tests pin generated schema/view APIs;
- negative tests reject bad booleans, enum values, UTF-8, length prefixes, and nested active elements;
- account loader tests keep immutable and mutable runtime borrow guards alive;
- Miri exercises borrow/aliasing behavior;
- generated-client contract tests assert exact instruction bytes without an object-representation cast;
- IDL drift tests ensure generated Rust and JavaScript clients describe the same discriminator-first wire layout;
- real SBF tests prove the initialized buffers are accepted on chain.

The invariant is simple: Pina may validate and borrow initialized external bytes, but it never turns an in-memory zeropod value into a raw byte slice.
