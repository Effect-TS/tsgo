---
"@effect/tsgo": minor
---

Add the `allOfMapToForEach` style diagnostic, which suggests replacing
`Effect.all(values.map(callback), options)` with the equivalent
`Effect.forEach(values, callback, options)` form.
