---
"@effect/tsgo": minor
---

Extend `nodeBuiltinImport` to recommend Effect-native alternatives for `console`, `timers`, `timers/promises`, `stream`, `stream/promises`, and `stream/web`, including their `node:` forms.

For Effect v4, also flag `crypto` and `node:crypto` imports and recommend `Crypto` from `effect`. For example, `import { randomUUID } from "node:crypto"` is now diagnosed; use the `Crypto` service's `randomUUIDv4` effect instead. Effect v3 crypto imports remain allowed because that version has no corresponding Crypto service.

Correct the Effect v4 `child_process` recommendation to point to `effect/unstable/process`. The rule remains disabled by default; configure `nodeBuiltinImport` with error severity to prohibit covered imports.
