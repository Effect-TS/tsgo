---
"@effect/tsgo": patch
---

Update the Effect dependencies to 4.1 (`effect`, `@effect/platform-node`, `@effect/platform-node-shared`, `@effect/vitest` 4.1.2).

Effect 4.1 replaced the `effect/Crypto` module with the `effect/crypto` modules, so the diagnostics that pointed to it now name the new APIs:

- `cryptoRandomUUID` and `cryptoRandomUUIDInEffect` suggest `SecureRandom` from `effect/crypto`, whose `uuidV4` replaces `crypto.randomUUID()`.

  ```ts
  const id = crypto.randomUUID()
  //         ^^^^^^^^^^^^^^^^^^^ This code uses `crypto.randomUUID()`, prefer `SecureRandom` from `effect/crypto` instead.
  ```

- `nodeBuiltinImport` reports `crypto` and `node:crypto` imports with `crypto` from `effect` as the Effect counterpart.

  ```ts
  import { createHash } from "node:crypto"
  //                         ^^^^^^^^^^^^^ This module reference uses the `crypto` module, the corresponding Effect API is `crypto` from `effect`.
  ```

The `effect-tsgo` CLI now hashes files with the `Digest` service from `effect/crypto`.
