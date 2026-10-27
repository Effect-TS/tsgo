---
"@effect/tsgo": patch
---

`preferSchemaOverJson`'s Effect v4 hint names `Schema.fromJsonString(Schema.Unknown)` for unknown shapes instead of `Schema.UnknownFromJsonString`, which effect marks `@internal` and leaves out of its published types:

```
Use `Schema.fromJsonString(Schema.Unknown)` for unknown shapes, `Schema.fromJsonString(schema)` for known ones, or `Schema.toCodecJson(schema)` when working with JSON values instead of strings.
```
