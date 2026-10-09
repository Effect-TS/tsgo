---
"@effect/tsgo": patch
---

Update the shim generator's Go tools dependency to support Go 1.27.2 export data and restore compiler setup.

Preserve pointers to unexported structs in generated shim mirrors so fields that follow them retain their offsets and accessors return the correct values.
