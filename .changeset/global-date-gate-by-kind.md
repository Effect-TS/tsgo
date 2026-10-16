---
"@effect/tsgo": patch
---

Make `globalDate` and `globalDateInEffect` do less work per file. Their shared walker used to look up the Effect context flags of every node it visited, although only call expressions (`Date.now()`) and `new` expressions (`new Date()`) can produce a diagnostic. It now looks up the flags only on those two node kinds, like `globalFetch`, `globalConsole`, `globalTimers`, `globalRandom` and `cryptoRandomUUID` already do.

Reported diagnostics are unchanged.
