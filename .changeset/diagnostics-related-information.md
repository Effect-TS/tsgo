---
"@effect/tsgo": patch
---

`effect-tsgo diagnostics` now reports a diagnostic's related locations. They used to be dropped in every format, so `missingEffectContext` never showed which layer could provide the missing service and `apiStabilityLeak` never showed where the unstable declaration lives.

With `--format json`, each diagnostic that has related locations gets a `relatedInformation` array. Each entry has the same location fields as the diagnostic, plus its own `code` and `message`. Diagnostics without related locations are unchanged.

```json
{
  "line": 23,
  "column": 3,
  "code": 377004,
  "name": "missingEffectContext",
  "message": "This Effect requires a service that is missing from the expected Effect context: `Db | Fs`.",
  "relatedInformation": [
    {
      "file": ".../src/index.ts",
      "start": 662,
      "length": 14,
      "line": 22,
      "column": 3,
      "endLine": 22,
      "endColumn": 17,
      "code": 377059,
      "message": "Adjusting this layer composition could provide the missing service."
    }
  ]
}
```

With `--format pretty`, the related locations are printed under the diagnostic, each with its own code snippet, as `tsc --pretty` does:

```
src/index.ts:23:3 - error effect(missingEffectContext): This Effect requires a service that is missing from the expected Effect context: `Db | Fs`.

23   Effect.runPromise
     ~~~~~~~~~~~~~~~~~

  src/index.ts:22:3 - Adjusting this layer composition could provide the missing service.
    22   Effect.provide(live),
         ~~~~~~~~~~~~~~
```

The `~` markers in pretty output are also aligned again on lines 10 and above. They used to sit one column to the left of the code.
