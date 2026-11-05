---
pina_cli: fix
---

# Validate IDL redirects before connecting

Follow at most five same-origin redirects when importing an IDL URL. Reject redirects that change the scheme, host, or port or contain credentials before contacting the destination, and preserve the total 30-second fetch timeout.
