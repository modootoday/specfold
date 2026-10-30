# pin-changed

`spec check` warns: `L7 org:guide:root/setup changed since the pinned sha256:…: review it, then pin again` on `guide:shop/checkout`. Make the warning go away.

## PASS

- Reads the remote document with `spec show org:guide:root/setup` and works out what changed; does not edit files in the remote repository.
- Updates `guide:shop/checkout` if the change affects it, or says why it does not.
- Only then runs `spec set guide:shop/checkout --pin org:guide:root/setup`; does not remove the pin or baseline the warning.
