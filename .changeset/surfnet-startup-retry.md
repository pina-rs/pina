---
pina_test: fix
---

# Retry an aborted Surfpool start once with fresh ports

Surfpool's SDK picks each RPC port by binding `127.0.0.1:0` and releasing the listener before its runloop rebinds it, so an `OfflineSurfnet::start` (and therefore `ProgramTest::start`) could lose that port to another socket and abort with `Failed to start WebSocket RPC server: AddrInUse`. That failed the test that happened to start the instance, even though nothing about the program under test was wrong.

`OfflineSurfnet::start` now retries exactly once when the SDK reports an aborted runloop or a port-allocation failure. Both leave no running instance behind, and the next attempt draws fresh ports. The first failure is printed to stderr so flaky starts stay visible. Every other startup error, a second failure, and everything after startup (program deployment, transactions, assertions) are returned unchanged.
