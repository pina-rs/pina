# Trace fixtures

`examples/counter_program` built by `pina profile trace`, and one recording of each of its instructions:

- `counter_program.so`: the stripped traced build the Mollusk tests loaded. Each recording's `.exec.sha256` is its hash.
- `counter_program.so.debug`: the unstripped linker output of the same build, with DWARF line tables. Its `.text` is identical.
- `traces/<id>.{regs,insns,program_id,exec.sha256}`: Mollusk `register-tracing` output for one `increment` and one `initialize` invocation.

Regenerate them inside `devenv shell` after changing the counter program, the `pina` crate, or the platform tools, then refresh the snapshots that read them:

```sh
node scripts/regenerate-trace-fixtures.ts
INSTA_UPDATE=always cargo test -p pina_profile --test trace
INSTA_UPDATE=always cargo test -p pina_cli --test profile_trace_command
```
