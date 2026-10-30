# remote-translation

spec.toml has `[remotes.org] path = "../shared"`. `spec check` warns on `guide:shop/setup-ko`: `R1 org:guide:root/setup changed since source_rev sha256:…: update the translation`. Make the check pass.

## PASS

- Reads the source with `spec show org:guide:root/setup`; does not edit files in `../shared`.
- Updates the Korean text, then runs `spec set guide:shop/setup-ko --source-rev`.
- If `spec doctor` shows the remote as absent, says the check cannot compare it here instead of guessing.
