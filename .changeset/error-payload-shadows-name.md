---
"@effect/tsgo": minor
---

Add `errorPayloadShadowsName` to detect inline `name` payload fields on Effect error classes. For example, `Data.TaggedError("ToolNotFound")<{ name: string }>` copies the payload's `name` onto the error instance and hides the error type in logs. Rename the field to `toolName`. The rule inspects inline payloads only and allows a field whose value is the literal the error already uses as its name.
