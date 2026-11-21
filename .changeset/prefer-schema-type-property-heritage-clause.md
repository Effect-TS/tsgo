---
"@effect/tsgo": patch
---

Fix `preferSchemaTypeProperty` reporting a `Schema.Schema.Type<typeof X>`-style helper used as an interface `extends` or class `implements` entry on TypeScript 7.1, where the suggested `typeof X.Type` is not valid syntax.

```ts
export const Person = Schema.Struct({ name: Schema.String })

// no longer reported: `extends typeof Person.Type` does not parse
export interface Person extends Schema.Schema.Type<typeof Person> {}
export class PersonImpl implements Schema.Schema.Type<typeof Person> {
  name = ""
}

// still reported: can be replaced with `Readonly<typeof Person.Type>`
export interface ReadonlyPerson extends Readonly<Schema.Schema.Type<typeof Person>> {}
```
