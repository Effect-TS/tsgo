---
"@effect/tsgo": minor
---

Add the `schemaClassObjectLiteral` diagnostic. It warns when an object literal is checked as a Schema class instance. A plain object has no class prototype, so `instanceof`, class methods, and Schema encoding fail at runtime. A Schema brand does not help, because a spread copies the brand property.

```ts
class User extends Schema.Class<User, { readonly brand: unique symbol }>("User")({ name: Schema.String }) {}

const copy: User = { ...user } // warning: build it with User.make({ ...user })
const renamed = User.make({ ...user, name: "b" }) // ok
```

In Effect v4, the rule skips objects nested inside a Schema constructor call, such as `Parent.make({ child: { ...child } })`, because v4 builds the nested class instance from the plain object.
