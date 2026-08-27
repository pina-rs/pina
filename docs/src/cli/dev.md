# Run a Development Surfnet

`pina dev` builds the current SBF program once, then delegates the persistent development network, artifact watching, and redeployment to Surfpool.

```bash
# First run: explicitly allow Surfpool to create txtx.yml.
pina dev --yes

# Review and commit txtx.yml, then use the safe offline default.
pina dev

# Opt into remote state explicitly.
pina dev --network devnet
pina dev --rpc-url https://api.mainnet-beta.solana.com
```

Pina invokes the foreground equivalent of:

```bash
surfpool start --watch --artifacts-path target/deploy \
  --manifest-file-path txtx.yml --offline
```

Surfpool remains in control of its terminal UI, logs, file watcher, redeployment, and shutdown. Pina intentionally leaves standard input, output, and errors attached to the foreground Surfpool process so prompts and Ctrl-C work normally. The short non-interactive version probe receives closed input and bounded output instead. Rebuild the program in another terminal to update the canonical `.so` and trigger redeployment.

## Deployment runbook

Surfpool uses `txtx.yml` to describe deployment. Creating or refreshing that runbook can write project files, so Pina will not silently accept a missing manifest. Run `pina dev --yes` for the first invocation, then inspect and commit the generated `txtx.yml`. The flag is forwarded to Surfpool and authorizes its non-interactive runbook changes; omit it during ordinary development when you want Surfpool to ask before changing the runbook. Pina rejects directories, symlinks, and Windows reparse points at this path.

## Network safety

Surfpool itself defaults to a mainnet datasource. Pina does not inherit that network-sensitive default: it always supplies `--offline` unless `--network` or `--rpc-url` is explicitly selected. The two upstream options conflict.

An explicit `--rpc-url` must use HTTP or HTTPS and include a host. Pina rejects user information, query parameters, fragments, and control characters. Surfpool receives every accepted URL as a child-process argument, where local process inspection can reveal it. Never put a credential or other secret anywhere in the URL, including an otherwise valid host or path. Prefer a named `--network` or a credential-free endpoint.

Pina requires Surfpool 1.5.0 or newer because its delegated watch and artifact flags are tested against that contract. See the official [Surfpool CLI reference](https://docs.surfpool.run/toolchain/cli).

## Options

| Option                | Meaning                                     |
| --------------------- | ------------------------------------------- |
| `--project <DIR>`     | Project directory or a directory below it   |
| `--network <CLUSTER>` | Fork `mainnet`, `devnet`, or `testnet`      |
| `--rpc-url <URL>`     | Fork a credential-free HTTP(S) RPC endpoint |
| `--yes`               | Allow non-interactive runbook file changes  |

Run `pina dev --help` for the authoritative command contract.
