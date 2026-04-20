---
"@effect/tsgo": patch
---

Reduce typeparser property lookup overhead by using direct property type access for Effect-related type detection, and add a regression test covering the plugin-only TS2589 failure path in `effectInFailure`.
