---
"@effect/tsgo": minor
---

Add `obsoleteSchemaImport` diagnostic (`TS377128`) to warn when importing `@effect/schema` or `@effect/schema/*` in projects targeting Effect v4. In Effect v4, Schema is built directly into `effect` (`import { Schema } from "effect"` or `import * as Schema from "effect/Schema"`).
