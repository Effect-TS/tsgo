---
"@effect/tsgo": minor
---

Report a diagnostic when `diagnosticSeverity` names a rule this build does not provide. A name that does not resolve was previously accepted in silence, so a project pinning a rule at `error` believed it was enforced while nothing ran. The check covers the top-level `diagnosticSeverity` map and the map inside each `overrides` entry, is anchored on the offending key in the config file that declares it, and carries a spelling suggestion when one is close enough.

```jsonc
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "diagnosticSeverity": {
          "floatingEfect": "error"
          // warning TS377135: Unknown Effect diagnostic rule `floatingEfect` in
          // `diagnosticSeverity`. Did you mean `floatingEffect`?
          // effect(unknownRuleName)
        }
      }
    ]
  }
}
```

The diagnostic is a `warning` by default and is configured through `diagnosticSeverity.unknownRuleName` like any other, so `"unknownRuleName": "off"` silences it. Note that with the default `ignoreEffectWarningsInTscExitCode: false` a warning already fails `tsc`.
