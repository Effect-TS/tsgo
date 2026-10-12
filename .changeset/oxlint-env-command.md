---
"@effect/tsgo": minor
---

Add the `effect-tsgo oxlint-env` command to run Oxlint with the Effect integration without patching `node_modules`. It prints `NAPI_RS_NATIVE_LIBRARY_PATH` and `OXLINT_TSGOLINT_PATH` for the Effect builds that match the installed `oxlint` and `oxlint-tsgolint` versions, and fails when the installed release has no build for them.

For example, `eval "$(effect-tsgo oxlint-env)" && oxlint` lints with the `effecttsgo` rules in a POSIX shell, and `effect-tsgo oxlint-env --shell powershell | Invoke-Expression` sets the same variables in PowerShell.
