# `pina_bpf`

<br>

BPF-targeted example ported from `pinocchio-bpf-starter` to `pina`.

## What this demonstrates

<br>

Compared to the original pinocchio starter style:

- Uses `pina` macros and types (`declare_id!`, `#[discriminator]`, `#[instruction]`, `parse_instruction`, `nostd_entrypoint!`).
- Separates the on-chain entrypoint behind a `bpf-entrypoint` feature.
- Includes host unit tests for instruction parsing and process logic.
- Includes ignored BPF integration tests that validate:
  - the BPF artifact exists after build
  - the generated artifact is a valid ELF binary

## Project workflow

<br>

Run the same commands available in a project created by `pina init`:

```bash
cd examples/pina_bpf
pina test --unit
pina test
pina generate
```
