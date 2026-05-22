---
"@effect/tsgo": minor
---

Add the `newSchemaClass` diagnostic for Effect v4 to discourage constructing Schema classes with `new` and suggest using `SchemaClass.make(...)` instead.

Example:

```ts
class User extends Schema.Class<User>("User")({
  name: Schema.String
}) {}

const user = new User({ name: "John" })
```

Now reports `newSchemaClass` and can be rewritten to:

```ts
const user = User.make({ name: "John" })
```
