---
"@effect/tsgo": minor
---

Add `effect-tsgo vp [args...]`, which runs vite-plus with the Effect-patched Oxlint without patching `node_modules`, under Node.js and Bun:

```sh
effect-tsgo vp check
effect-tsgo vp lint src
effect-tsgo vp run check
```

`vp check`, `vp lint` and the scripts run through `vp run` report the `effecttsgo/*` rules configured in `vite.config.ts`, using the Effect-patched binding and `tsgolint` of the oxlint version vite-plus resolves.

`effect-tsgo oxlint` now loads the patched binding the same way, through a `process.dlopen` bridge preloaded with `NODE_OPTIONS` and `BUN_OPTIONS` in front of the options already set there, instead of `NAPI_RS_NATIVE_LIBRARY_PATH`. This fixes `effect-tsgo oxlint` with an `oxlint.config.ts` that imports vite-plus, which failed with `TypeError: Class extends value undefined` because that variable also redirected the other native addons of the process.
