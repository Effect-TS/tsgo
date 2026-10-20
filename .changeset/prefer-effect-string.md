---
"@effect/tsgo": minor
---

Add the opt-in `preferEffectString` diagnostic in `effectNative` for native String method invocations. It suggests 32 `effect/String` counterparts, including `trimLeft` to `String.trimStart` and `trimRight` to `String.trimEnd`.

Each suggestion explains typing or runtime differences. For example, `email.indexOf("@")` suggests `String.indexOf`, which returns Option with None replacing -1.

The rule defaults to off and the `effect-native` preset enables it as a warning. Both Effect v3 and v4 are supported. Only suppression quick fixes are available.
