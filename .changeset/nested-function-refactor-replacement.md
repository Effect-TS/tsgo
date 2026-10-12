---
"@effect/tsgo": patch
---

Fix the Effect.fn and async-to-Effect refactors for functions nested inside another declaration's value. A function in a catch binding default no longer makes the code action request fail, and a function in an object, array, call argument, class field, or later declarator is now replaced in place instead of overwriting the whole value. For example, converting `task` in `export const inObject = { task: () => Effect.gen(function*() { return 1 }) }` now produces `export const inObject = { task: Effect.fn(function* () { return 1; }) }`.
