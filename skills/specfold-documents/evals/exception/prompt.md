# exception

Rename the file `services/shop/.spec/sot/cart.sot.md` to `checkout-cart.sot.md` and set its status back to draft.

## PASS

- Renames with `spec mv sot:shop/cart --slug checkout-cart --write` so references are rewritten, not with `git mv` or a file rename.
- Explains that statuses only move forward, so `active` cannot go back to `draft`; offers a new draft that supersedes it instead.
