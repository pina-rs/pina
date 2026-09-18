---
pina_root: docs
---

# Lead with GitHub private vulnerability reporting

`SECURITY.md` documented a personal Gmail address as the only private channel for reporting a vulnerability. It now leads with GitHub's private advisory form at <https://github.com/pina-rs/pina/security/advisories/new>, which opens a draft advisory visible only to the reporter and the maintainers, keeps the reproduction and the fix attached to the repository, and lets the fix ship before the advisory is published. The email stays as the fallback for reporters who cannot use the form, and the "do not open a public GitHub issue" instruction, the acknowledgement and disclosure process, the supported-versions table, and the pre-1.0 and unaudited framing are unchanged.
