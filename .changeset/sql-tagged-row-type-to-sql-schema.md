---
"@effect/tsgo": minor
---

Add the `sqlTaggedRowTypeToSqlSchema` diagnostic (off by default, Effect v4). It flags Effect's `sql` tagged template when it is given an explicit row type, which only asserts the row shape, and suggests `SqlSchema` with a `Result` Schema so rows are decoded at runtime.

```ts
const [existing] = yield* sql<CommandRow>`SELECT command_id, fingerprint FROM commands WHERE key_hash = ${keyHash}`
// Suggested instead:
const findByKey = SqlSchema.findOne({
  Request: Schema.String,
  Result: CommandRow,
  execute: (key) => sql`SELECT command_id, fingerprint FROM commands WHERE key_hash = ${key}`
})
```
