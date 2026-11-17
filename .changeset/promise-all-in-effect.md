---
"@effect/tsgo": minor
---

Add the `promiseAllInEffect` diagnostic (effectNative, off by default; enable with `"promiseAllInEffect": "warning"`) and its quick fix.

Inside Effect constructor thunks (`Effect.promise`, `tryPromise`, `sync`, `try`, `suspend`, `callback`) and `Effect.gen` / generator `Effect.fn` bodies, a call to the global `Promise.all` or `Promise.allSettled` (in any spelling, such as `Promise["all"]` or `Promise?.all`) over an array or a tuple with a rest element, a union of them, one intersected with metadata such as a brand, or a type parameter constrained to one, is reported: it starts every promise at once with no concurrency bound, and they keep running when the fiber is interrupted. Fixed-length tuples such as `Promise.all([a(), b()])` are not reported, nor are calls inside a function nested in the thunk.

The quick fix (Promise.all only) applies when an `Effect.promise` / `Effect.tryPromise` thunk's whole concise body is `Promise.all(xs.map(<inline callback>))` over an array, with a callback of at most two parameters:

```ts
// before
Effect.tryPromise(() => Promise.all(ids.map((id) => fetchUser(id))))
// after
Effect.forEach(ids, (id) => Effect.tryPromise(() => fetchUser(id)), { concurrency: "unbounded" })
```

The thunk's `signal` parameter is declared again on each per-item thunk, and an `async` callback keeps `async`. There is no fix for `allSettled` (its success type would change), identifier callbacks, callbacks that do not return a `Promise` for every item, callbacks with a `this` parameter, async or block-bodied thunks, non-array receivers, or when moving the callback would change what a name refers to.
