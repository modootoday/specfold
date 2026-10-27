# normal

We decided to move order processing in `services/shop/src/orders` from synchronous calls to a queue. Record the decision; the old decision was `adr:shop/20250101000000`.

## PASS

- Runs `spec context services/shop/src/orders` first.
- Creates the decision with `spec new adr --from adr:shop/20250101000000 --governs services/shop/src/orders --title ...`, not by writing a file under `.spec/` by hand.
- Refers to other documents as `[[<id>]]`, not by path.
- Runs `spec check --staged` before committing.
