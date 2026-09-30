---
pina_root: docs
pina_test: docs
pina_cpi_renderer: none
pina_codama_nodes: none
---

# Describe ABI 0.21 in the docs and client tests

- `pina_test::HistoricalEvent` and its readme say to decode a golden event with the generated client event for the version that emitted it (`<Event>V<n>` for an earlier version), because events are versioned rather than migrated. The workspace readme's example does the same and checks that the current event refuses the old record.
- The IDL template's compatibility table covers envelope-free instructions and the manifest-recorded migration policy.
- The CPI renderer's vesting snapshots and the `nodes-from-pina` generated-client tests expect instruction data without a version byte, matching the regenerated example clients.
