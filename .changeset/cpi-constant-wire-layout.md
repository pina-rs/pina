---
pina_cpi_renderer: fix
---

# Reject CPI layouts that would drop or move constant fields

Reject omitted arguments unless they exactly match the leading discriminator fields in wire order. Validate byte discriminator widths so generated CPI data preserves secondary selectors and cannot shift caller arguments into constant fields.
