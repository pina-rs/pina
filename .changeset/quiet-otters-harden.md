---
pina_cli_renderer: fix
pina_cli: fix
pina_codama_renderer_cli: fix
---

Harden the generated CLIs and the renderer:

- the https-only endpoint check now parses the URL authority and compares the exact loopback host, so `http://localhost.evil.com` or `http://127.0.0.1@evil.com` are rejected instead of trusted
- generated TypeScript fetch commands verify account ownership before decoding, and `--program-id` is validated instead of cast
- generated TypeScript apps declare `commander` and a kit version matching the emitted code, so `npm install` yields a runnable app
- regeneration in update mode refuses destinations missing the `.pina-generated` marker instead of clobbering hand-written CLIs
- IDL names that would emit invalid Rust identifiers are rejected with a clear renderer error
- global flags work after the subcommand in generated Rust CLIs
