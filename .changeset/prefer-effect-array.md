---
"@effect/tsgo": minor
---

Add the opt-in `preferEffectArray` diagnostic in `effectNative` for native Array method calls and references. It suggests 16 corresponding Effect Array APIs and explains migration differences, such as `values.find(predicate)` becoming `Array.findFirst(values, predicate)` with an `Option` result.

The rule is disabled by default and enabled as a warning by the `effect-native` preset. It supports readonly arrays, tuples, aliases, and literal bracket access. Native Array recognition uses a lazy cache shared per checker. Conversion autofixes are not included.
