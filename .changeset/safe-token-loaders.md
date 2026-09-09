---
pina: breaking
pina_cli: breaking
pina_lints: breaking
---

# Consolidate checked token loaders

Make canonical SPL Token or Token-2022 ownership an explicit contract of every public token loader. The implementation delegates ownership and layout validation to the pinned upstream checked account-view parsers without repeating their owner comparison. The ATA loader additionally validates the derived address and the current authority and mint stored in account data.

Remove the redundant `*_checked` loader aliases. Also retire `require_owner_before_token_cast` and `require_associated_token_address_before_ata_cast`, because runtime loaders now enforce both conditions without lexical lint checks.
