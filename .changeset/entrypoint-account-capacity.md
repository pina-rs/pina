---
pina: feat
pina_macros: feat
---

# Generate a safe entrypoint account capacity

`#[discriminator(entrypoint)]` now generates `ENTRYPOINT_ACCOUNT_CAPACITY`, an account-array size for the second argument of `nostd_entrypoint!`:

```rust,ignore
nostd_entrypoint!(
	CounterInstruction::process_instruction,
	CounterInstruction::ENTRYPOINT_ACCOUNT_CAPACITY
);
```

The program-size guide and ADR 0010 recommended bounding the entrypoint's account array at the widest instruction, and the lean comparison fixtures measured that bound. It is unsafe: the loader skips accounts past the array instead of rejecting them, so an extra trailing account never reached `finish_exact` and an over-supplied instruction ran instead of failing with `TooManyAccountKeys`. The generated capacity keeps one spare slot past the widest routed accounts struct and the reserved `Migrate` route's slots, and falls back to 255 when any route accepts unbounded trailing accounts, so every exact-count instruction still rejects extra accounts. The one observable difference from the full array is precedence: a writable account whose duplicate sits past the spare slot fails with `TooManyAccountKeys` rather than `DuplicateMutableAccount`. The generated capacity test asserts the spare slot for every route.

A bounded array only shrinks a program when it has five slots or fewer, because pinocchio walks accounts five at a time; a larger bound keeps that loop and adds one that skips the extra accounts. Measured on top of the seed-capacity change: the hello fixture fell from 4,680 to 2,944 bytes, the counter fixture from 10,456 to 9,416, and `counter_program` from 16,096 to 14,896, at +1 compute unit on the fixtures' `hello` and `initialize` and +4 on `increment`. The migrations, escrow, and staking examples, whose capacities exceed five, grew by 184 to 464 bytes with a bound and keep the default.

`nostd_entrypoint!` and `nostd_entrypoint_alloc!` now brace their second argument, so a path such as `Enum::ENTRYPOINT_ACCOUNT_CAPACITY` is accepted as the const generic. The published Pina comparison fixtures now use the generated router with the capacity, replacing the removed `pina_lean` fixtures, and `counter_program` adopts it with a Surfpool test proving that extra trailing accounts inside and past the array are rejected. The program-size guide and ADR 0010 document the spare-slot rule and the five-slot threshold.
