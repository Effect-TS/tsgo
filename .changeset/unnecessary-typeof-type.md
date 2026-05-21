---
"@effect/tsgo": minor
---

Add the `unnecessaryTypeofType` style diagnostic and quick fix.

This suggests replacing schema-style annotations such as `typeof UserId.Type`
with the matching named type when that type is available and equivalent.

Examples:

```ts
import { UserId } from "./schemas"

const a: typeof UserId.Type = {}
```

becomes:

```ts
import { UserId } from "./schemas"

const a: UserId = {}
```

It also supports qualified and namespace-imported names such as
`typeof UsersRepo.User.Type` and `typeof Schemas.UsersRepo.User.Type`.
