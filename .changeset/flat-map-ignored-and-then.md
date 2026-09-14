---
"@effect/tsgo": minor
---

Add the `flatMapIgnoredParamToAndThen` diagnostic and quick fix for replacing zero-parameter `Effect.flatMap` callbacks that return an existing constant Effect value with `Effect.andThen`. Existing diagnostics now also recognize Effect APIs referenced through constant aliases, and their quick fixes rebuild the replacement API from the source file's imported module name without modifying the shared alias.
