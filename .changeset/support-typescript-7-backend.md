---
"@effect/tsgo": minor
---

Add support for the `typescript` package (>= 7, e.g. the 7.0 RC) as a native backend, alongside the existing `@typescript/native-preview` backend.

`effect-tsgo patch`/`unpatch` now resolve the native TypeScript binary from whichever backend is installed: `@typescript/native-preview` is tried first (back-compat), then `typescript` >= 7, whose Go binary ships as `lib/tsc` under the `@typescript/typescript-<plat>-<arch>` platform sub-package. A version gate ensures `typescript` < 7 (the JavaScript compiler) is never treated as a native backend.

`effect-tsgo setup` recognises an existing `typescript` >= 7 install as the native backend so it no longer redundantly re-adds `@typescript/native-preview`, writes the correct backend package to `package.json`, and points the VS Code `typescript.native-preview.tsdk` setting at `node_modules/typescript` for the `typescript` backend (vs `node_modules/@typescript/native-preview` otherwise).

Before, a project using `typescript@^7.0.1-rc` without `@typescript/native-preview` failed with `NativePreviewNotInstalledError`:

```
$ effect-tsgo patch
ERROR: NativePreviewNotInstalledError: @typescript/native-preview is not installed.
```

After, the same project patches successfully against the `typescript` >= 7 binary.
