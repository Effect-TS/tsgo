---
"@effect/tsgo": patch
---

Fix `unnecessaryTypeofType` reporting a `typeof X.Type` query in the `extends` clause of the interface `X` itself, where the suggested replacement would make the interface extend itself.

```ts
export const Person = Schema.Struct({ name: Schema.String })
export type Identity<A> = A

// no longer reported: `extends Identity<Person>` would be a circular base
export interface Person extends Identity<typeof Person.Type> {}

// still reported: can be replaced with `UserId`
export interface Other extends Identity<typeof UserId.Type> {}
```
