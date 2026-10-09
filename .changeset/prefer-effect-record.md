---
"@effect/tsgo": minor
---

Add the opt-in `preferEffectRecord` diagnostic in `effectNative` for native Object calls and references. It suggests `Record.keys`, `Record.values`, `Record.toEntries`, `Record.fromEntries`, and `Record.has`, including native `hasOwnProperty` references.

Each suggestion explains input typing or runtime differences. For example, `Object.entries(data)` suggests `Record.toEntries(data)` with typed keys and a caveat about getter or proxy mutations. `data.hasOwnProperty(key)` suggests passing the receiver explicitly to `Record.has(data, key)`.

The rule defaults to off and the `effect-native` preset enables it as a warning. Both Effect v3 and v4 are supported. Only suppression quick fixes are available.
