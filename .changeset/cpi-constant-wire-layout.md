---
pina_cpi_renderer: fix
---

# Reject CPI layouts that would drop or move constant fields

Reject omitted arguments unless their bytes are already covered by the discriminator prefix in wire order. Preserve constant and field discriminator representations, and validate byte discriminator widths so generated CPI data cannot shift caller arguments into constant fields.
