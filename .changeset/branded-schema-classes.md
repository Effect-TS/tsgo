---
"@effect/tsgo": minor
---

Add the `brandedSchemaClasses` plugin option, which lists the Schema class kinds that get a nominal brand (`{ readonly brand: unique symbol }`). The values are the Schema API names (`"Class"`, `"TaggedClass"`, `"Error"`, `"TaggedError"`, `"Opaque"`). The default is all five, so nothing changes unless the option is set. Like other per-file options, it can also be set in `overrides`.

- `schemaClassMissingBrand` only reports classes whose kind is in the list.
- The `Schema.` extends-clause completions only insert the brand for kinds in the list.
- An empty list means the rule never reports and the completions never insert a brand.

For example, to stop asking for a brand on tagged classes and tagged errors, whose literal `_tag` already keeps different classes apart:

```jsonc
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "brandedSchemaClasses": ["Class", "Error", "Opaque"]
      }
    ]
  }
}
```

```ts
export class User extends Schema.Class<User>("User")({ name: Schema.String }) {}
//                                     ^^^^ Add `{ readonly brand: unique symbol }` as the second type argument. ...
export class Created extends Schema.TaggedClass<Created>()("Created", { id: Schema.String }) {}
// no diagnostic: TaggedClass is not in brandedSchemaClasses
```
