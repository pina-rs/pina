---
pina_cli: test
---

# Close mutation-testing gaps in verified-build validation

Nightly mutation testing showed that several verified-build record checks could be weakened without any test failing. Some existing cases were rejected by an earlier check (an out-of-order feature list, a record filename that no longer matched its manifest) before reaching the check they were written for. The record cases now name each file for its own manifest and assert the exact rejection reason, and new cases cover the inclusive feature-byte and record-size limits, the file-type checks on record paths, IPv6 port suffixes, public `2001::/16` addresses, the 253-byte host and 63-byte label limits, and a zero run that spans a read boundary in the executable hash.
