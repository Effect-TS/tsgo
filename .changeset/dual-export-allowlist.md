---
"@effect/tsgo": patch
---

Fix `unstableApiUsage` and `experimentalApiUsage` naming overloaded (dual) exports by module, which prevented `allowedUnstableApis` / `allowedExperimentalApis` from allowing them per export.

A dual export whose call signatures carry their own `@stability` tag — for example `effect/http/HttpClientRequest#bodyText` — was reported under the bare module name (`effect/http/HttpClientRequest`) because the selected call signature is not the module export. An allow-list entry for `effect/http/HttpClientRequest#bodyText` therefore never matched, and the only way to allow the use was a whole-module entry.

The reported name now resolves to the export that owns the selected overload signature, matching the README's `package/module#exportName` form:

```ts
import { HttpClientRequest } from "effect/http"

export const request = HttpClientRequest.post("http://x").pipe(
  HttpClientRequest.setHeader("a", "b"),
  HttpClientRequest.bodyText("{}", "application/json")
)
```

With:

```json
"allowedUnstableApis": [
  "effect/http/HttpClientRequest#post",
  "effect/http/HttpClientRequest#setHeader",
  "effect/http/HttpClientRequest#bodyText"
]
```

all three uses are allowed, and any unlisted dual export still warns under `package/module#exportName`.
