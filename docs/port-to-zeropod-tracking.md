# Native zeropod integration — implementation tracking

_Origin: pina-rs/pina#193 and pina-rs/pina#195. Follow-up: pina-rs/pina#192._

## Design decisions

1. **Native schemas, generated storage views.** `#[account]`, `#[instruction]`, and `#[event]` derive `zeropod::ZeroPod`. A schema such as `ProfileState` has native Rust fields; runtime data is accessed through `ProfileStateZc`.
2. **Bytes only enter the model.** Account and instruction byte slices may be validated and viewed without copying. Pina never exposes the object representation of a schema or zero-copy view as bytes.
3. **Inactive capacity is unobservable.** `String` and `Vec` storage may leave inactive capacity uninitialized. Safe access is limited to their active elements, and Pina does not add a whole-object byte-slice escape hatch.
4. **Caller-owned initialization.** `Type::initialize(&mut bytes)` zeroes an exact-size buffer, writes the discriminator, validates it, and returns `&mut TypeZc`. This is the ergonomic construction path for tests and account creation.
5. **Zeropod owns zero-copy casts.** Pina delegates slice-to-view conversion to `ZeroPodFixed`; it does not duplicate generic pointer-casting helpers or implement zeropod's unsafe traits itself.
6. **Generated clients own wire buffers.** Rust client instruction builders allocate a zeroed `Vec<u8>`, configure its private generated zeropod view, validate it, and move the buffer into the instruction. They do not expose a reusable `to_bytes()` method.
7. **Discriminator-first layout.** The discriminator remains the first schema field and is included in `SIZE`.

## Migration checklist

- [x] Make `PinaAccount` a `ZeroPodFixed` native schema and return `Self::Zc` from account loaders.
- [x] Make account, instruction, and event macros derive `ZeroPod` and generate checked `try_from_bytes` / `initialize` helpers.
- [x] Remove Pina's `to_bytes`, `PinaSerialize`, `InstructionBuilder`, custom `PodEnum`, and generic `pod_from_bytes` APIs.
- [x] Migrate examples, tests, and security fixtures to native schema fields and `.get()` / `.set()` storage-view access.
- [x] Generate native zeropod schemas and validated storage views in Rust Codama clients without Pina-owned raw casts.
- [x] Regenerate all IDLs and clients after source documentation is current.
- [x] Run formatting, workspace tests, no-default builds, docs, IDL drift, client contract tests, Miri, and real SBF examples.
- [ ] Rebase, push, and complete hosted CI/review monitoring on PR #192.

## Safety boundary

The native schema value is never treated as its wire representation. Only initialized runtime buffers are passed to zeropod for validation and viewing. Mutable views borrow those buffers, preventing simultaneous raw access. Pina's public APIs expose semantic fields and active collection elements; they do not make inactive capacity observable.
