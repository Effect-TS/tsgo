---
"@effect/tsgo": minor
---

Add the `promiseRaceInEffect` diagnostic (group `effectNative`, off by default, Effect v3 and v4).

It reports a global `Promise.race` or `Promise.any` call over a data-driven list (an array, or a tuple with a rest element) inside Effect-run code: an Effect constructor thunk such as `Effect.promise` / `Effect.tryPromise`, or an `Effect.gen` / generator `Effect.fn` body. Once a winner settles, the losing promises keep running, and fiber interruption does not reach them. Fixed-length tuples are not reported, and neither is a combinator inside a function nested in the thunk.

```ts
import { Effect } from "effect"

declare const mirrors: ReadonlyArray<string>
declare const download: (url: string) => Promise<Uint8Array>

// reported
export const firstSettled = Effect.tryPromise(() => Promise.race(mirrors.map((url) => download(url))))

// preferred: the losers are interrupted as soon as there is a winner
export const firstSettledEffect = Effect.raceAllFirst(mirrors.map((url) => Effect.tryPromise(() => download(url))))
```

The message for `Promise.any` names `Effect.raceAll`. The message for `Promise.race` names `Effect.raceAllFirst` on Effect v4; Effect v3 has no list version, so there it reports the problem without naming a replacement.

A quick fix is offered only when an `Effect.promise` / `Effect.tryPromise` thunk's whole expression body is `Promise.race(xs.map(<inline callback>))` or `Promise.any(xs.map(<inline callback>))`, `xs` is a constant, and the callback always returns a `Promise`. Each item gets its own `Effect.promise` / `Effect.tryPromise` thunk, which declares the original `signal` parameter again and takes over the callback's return type annotation. No fix is offered when a callback parameter has a default value or a destructuring pattern (they would now run when the Effect is built), when the callback's parameters use the thunk's parameters, when a named callback refers to itself, or when `map` has explicit type arguments:

- On Effect v4, `Promise.race` becomes `Effect.raceAllFirst(...)`. `Promise.any` gets no fix there, because on an empty list `Effect.raceAll([])` never settles while `Promise.any([])` rejects.
- On Effect v3, `Promise.any` becomes `Effect.raceAll(...)`. `Promise.race` gets no fix there.

Accepted differences after the fix: `xs.map` now runs when the Effect is built (each callback body stays lazy). When every item fails, the error is not an `AggregateError`. On v3, an empty list makes the Effect die with an `IllegalArgumentException`.
