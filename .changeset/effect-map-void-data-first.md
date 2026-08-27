---
"@effect/tsgo": minor
---

Detect the data-first form of `effectMapVoid` by driving detection off piping flows.

`effectMapVoid` previously matched only the data-last / pipeable form of the pattern, so a data-first `Effect.map(self, () => {})` was missed. Detection now runs over the piping-flow transformations, which normalize the subject away, so both forms are reported uniformly and the quick fix rewrites each one correctly.

For example, the data-first call:

```ts
Effect.map(Effect.succeed(1), () => {})
```

is now flagged and fixed to:

```ts
Effect.asVoid(Effect.succeed(1))
```

while the existing data-last form (`pipe(self, Effect.map(() => {}))`) continues to be fixed to a bare `Effect.asVoid`.
