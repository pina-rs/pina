---
pina_cpi_renderer: fix
---

# Interpolate the field name in boolean CPI argument writes

The `TypeNode::Boolean` branch of `render_argument` built its write statement with a plain string, so the `{field}` placeholder reached the generated CPI source literally as `data[offset] = u8::from(self.{field});` and `pina cpi` aborted for every program with a boolean instruction argument because the emitted source failed to parse. The branch now formats the statement like every other scalar argument, escaping the `{offset}` placeholders so `render_argument_write` still substitutes the concrete offsets.
