---
"@effect/tsgo": patch
---

Update the bundled `typescript-go` submodule to `8c45757f8` and refresh the local compatibility layer for the upstream hover and printer API changes.

This updates the hover patch, regenerates shims, and adjusts local callers to the new `TypeToStringEx` and `SignatureToStringEx` signatures so setup, check, test, and lint continue to pass.
