---
pina_cli: fix
---

# Read transition offsets from the stored schema

A generated automatic transition derived its byte offsets from a source schema with dropped and renamed fields already resolved. The bytes it edits are the stored layout, where a dropped field still occupies its space, so any field after a removal was read from the wrong offset and a field after a rename could be read from the wrong one too. The destination validation accepted the result whenever the shifted bytes still formed a valid layout, which made the corruption silent: `{ a, b, c } -> { a, c }` copied `a` to `a` and left `c` reading the old `b`. `SOURCE_SIZE` was wrong in the same way, understating the stored payload.

The planner now proves a transition from the stored schema against the developer's resolved intent and emits its proof: surviving fields copy from their stored offsets, dropped fields keep occupying their bytes, and every stored field must be either copied or explicitly discarded. A change that no single copy order can express — one field moving right while another moves left, a same-name type change, two destination fields reading one stored field — falls back to a manual transition instead of emitting bytes its own proof does not justify.

Fixed-layout accounts gain a manual escape hatch. `--manual <field>` names an added field whose conversion the developer writes. The recorded transition mode and renames carry that intent, so repeated `make` runs keep the authored body instead of regenerating an automatic transition over it. Pairing it with `--rename` allows a rename whose type changed, which a verbatim byte copy cannot express; the previously impossible "combine `first_name` and `last_name` into `name`" migration is now a generated manual draft with the correct stored sizes. Contradictory answers fail closed in both directions, including across the persisted `[migrations.answers]` table.

A generated manual draft now carries an offset table in its header: one row per field with its payload-relative range in the stored layout and in the destination, `removed` for acknowledged discards, and prefix notes for compact tails. The developer implementing the body can see at a glance which bytes can move and which stay put.

A recorded manual transition now scopes itself to the hop it was written for. Previously the recorded mode seeded the _next_ adjacent change too, so any hop after a hand-written transition was branded manual forever, even one the byte-level proof fully justifies — removing a field two versions after a narrowing conversion wrongly demanded another manual body. Repeated `make` runs over the manual hop's own draft still replay its recorded mode, so authored bodies remain stable.
