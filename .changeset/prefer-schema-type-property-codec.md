---
"@effect/tsgo": patch
---

`preferSchemaTypeProperty` now also reports the `Schema.Codec` helpers, the Effect v3 `Schema.Schema.Encoded` and `Schema.Schema.Context` helpers, and helpers referenced through a named `Schema` or `Codec` import. The quick fix rewrites each one to the matching property.

```ts
import { Schema } from "effect"

const User = Schema.Struct({ id: Schema.NumberFromString })

type UserEncoded = Schema.Codec.Encoded<typeof User> // now reported, fixed to: typeof User.Encoded
type UserDecodingServices = Schema.Codec.DecodingServices<typeof User> // fixed to: typeof User.DecodingServices
type UserEncodingServices = Schema.Codec.EncodingServices<typeof User> // fixed to: typeof User.EncodingServices
```

The message now names the helper as written and the property to use, for example "Do not use `Schema.Codec.Encoded<typeof X>`. Use `typeof X.Encoded` instead."
