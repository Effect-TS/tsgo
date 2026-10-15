---
"@effect/tsgo": patch
---

Rules now read the type the checker computed for an expression while it checked the file, instead of asking the checker again afterwards. Asking again repeated work, such as flow analysis, and in a few cases gave a different type than the check used. For example, a return literal checked while its enclosing generic call was still resolving:

```ts
export const takeWhileDefined = dual(2, <A>(self: Iterable<A>, f: (input: A) => A | undefined) => ({
  [Symbol.iterator]() {
    const iterator = self[Symbol.iterator]()
    return {
      next() {
        const result = iterator.next()
        if (result.done) {
          // the check typed this as { done: boolean; value: undefined }
          // asking again gave { done: true; value: undefined }
          return { done: true, value: undefined }
        }
        return { done: false, value: f(result.value) }
      }
    }
  }
}))
```

Rules now see `{ done: boolean; value: undefined }`, the type the rest of the check is based on.
