---
pina: breaking
pina_cli: breaking
pina_lints: breaking
---

# Make token loaders safe by default

Require canonical SPL Token or Token-2022 ownership in every public token loader. The ATA loader now validates the derived address and the wallet and mint stored in account data.

Remove the redundant `*_checked` loader aliases. Also retire `require_owner_before_token_cast` and `require_associated_token_address_before_ata_cast`, because runtime loaders now enforce both conditions without lexical lint checks.
