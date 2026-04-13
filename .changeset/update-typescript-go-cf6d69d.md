---
"@effect/tsgo": patch
---

Update the bundled `typescript-go` submodule to `cf6d69d83` and refresh the local compatibility layer for the upstream AST and language-service API changes.

This includes a refreshed code-actions patch plus shim regeneration so repository setup, checks, tests, and lint all pass on the new upstream revision.
