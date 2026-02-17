---
pina: minor
---

Add three custom dylint lint rules to catch common Solana security mistakes at compile time:

- `require_owner_before_token_cast`: Warns when `as_token_mint()`, `as_token_account()`, `as_token_2022_mint()`, or `as_token_2022_account()` is called without a preceding `assert_owner()` or `assert_owners()` on the same receiver.
- `require_empty_before_init`: Warns when `create_program_account()` or `create_program_account_with_bump()` is called without a preceding `assert_empty()` on the target account.
- `require_program_check_before_cpi`: Warns when `.invoke()` or `.invoke_signed()` is called without a preceding program address verification via `assert_address()`, `assert_addresses()`, or `assert_program()`.
