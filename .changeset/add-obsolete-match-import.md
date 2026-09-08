---
"@effect/tsgo": minor
---

Add `obsoleteMatchImport` diagnostic (`TS377127`) to warn when importing `@effect/match` in projects targeting Effect v4. In Effect v4, pattern matching is built directly into `effect` (`import { Match } from "effect"` or `import * as Match from "effect/Match"`).
