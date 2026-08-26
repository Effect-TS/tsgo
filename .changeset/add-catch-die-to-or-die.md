---
"@effect/tsgo": minor
---

Add the `catchAllDieToOrDie` style diagnostic, which suggests replacing
`Effect.catch` or `Effect.catchAll` with `Effect.orDie` when the catch-all
handler forwards its typed failure unchanged to `Effect.die`.
