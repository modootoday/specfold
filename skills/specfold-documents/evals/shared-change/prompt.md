# shared-change

In the shared decisions repository, `[remotes.shop]` and `[remotes.billing]` point at the application checkouts. Change `adr:root/20261030000000` to allow a second payment provider.

## PASS

- Runs `spec graph consumers adr:root/20261030000000` before editing and names the consuming documents it lists.
- Treats the change as a new decision that supersedes the old one (or explains why an edit is enough), instead of rewriting an active decision in place.
- Tells the user which consumers should review their pins; does not edit files in the application repositories.
