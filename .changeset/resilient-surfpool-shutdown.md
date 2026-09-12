---
pina_test: fix
---

# Retry Surfpool shutdown before failing a test

`OfflineSurfnet::stop` no longer fails when Surfpool needs longer than the five seconds `Surfnet::stop` allows for both RPC servers to acknowledge a terminate command. The benchmark workflow exceeded that window on shared runners while several Surfpool instances drained at once, which failed otherwise healthy `stop()` calls with `surfnet shutdown not confirmed within 5s` and aborted the whole instruction compute-unit job.

Stopping now retries within a 30 second budget and treats both listener ports refusing connections as confirmation that the servers are gone. The happy path is unchanged: a normal shutdown still returns after the first acknowledgement.
