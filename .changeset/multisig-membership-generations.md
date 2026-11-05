---
pina: none
---

# Stop rejoined multisig members reusing old grants

`multisig_program` now tracks membership generations. The multisig bumps `member_generation` on every `AddMember` and records the generation each member joined at in a new `member_generations` tail, and each spending limit records the generation it was granted at. `SpendingLimitUse` refuses a drawer who joined after the grant.

Previously, removing a member pruned only the spending limits passed to that execution, so an omitted limit kept the member on its roster, and re-adding the same address let the old grant authorize draws again. A returning member now needs a new grant, while members who never left keep theirs. The `Multisig` and `SpendingLimit` layouts gain the new fields in the unpublished v0 contract, and the IDL and generated clients are regenerated to match.
