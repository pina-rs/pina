---
pina_cli: feat
pina_skill: docs
---

# Gate upgrades on a rehearsal with `pina deploy --rehearse`

`pina deploy --rehearse` replays the target cluster's recent transactions against the upgrade before the confirmation prompt, using the same engine as `pina rehearse`, and stops the deployment before anything is sent when the upgrade changes behaviour. The rehearsal runs after the plan is printed. It reads the planned artifact once, checks it against the SHA-256 the plan pinned, and rehearses those exact bytes; with `--build` that is the artifact just built. The deployment later checks its private snapshot against the same digest, so an artifact replaced during or after the rehearsal is never deployed.

The rehearsal targets the cluster the deployment writes to: the named cluster's endpoint (`localnet` is `http://127.0.0.1:8899`), or the custom URL after `pina rehearse --rpc-url`'s URL checks, named in the report by its origin only. A behaviour change stops the deployment with exit code `2` unless `--allow-rehearsal-changes` accepts it. A rehearsal that compares nothing stops it with exit code `3`, and so does a first deployment, because there is no deployed program to rehearse against; deploy a first version without `--rehearse`. A rehearsal that cannot run exits `1`. `--rehearse-limit <N>` replays the N most recent transactions (default 25, up to 1000). With `--dry-run` the rehearsal reports what a deployment would do and exits with the same codes, and with `--dry-run --json` the plan gains a `rehearsal` key holding the complete `pina rehearse --json` report. Progress goes to stderr.

The agent skill's CLI reference describes the gated deployment workflow.
