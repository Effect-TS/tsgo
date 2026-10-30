// @filename: /node_modules/nested/package.json
{ "name": "nested", "version": "1.0.0", "types": "index.d.ts" }
// @filename: /node_modules/nested/index.d.ts
export { marker } from "effect"
// @filename: /node_modules/nested/node_modules/effect/package.json
{ "name": "effect", "version": "3.19.19", "types": "index.d.ts" }
// @filename: /node_modules/nested/node_modules/effect/index.d.ts
export declare const marker: unique symbol
// @filename: mixedVersions.ts
// @effect-diagnostics *:off
// @effect-diagnostics effectSleepZeroToYieldNow:suggestion
import { Effect } from "effect"
import { marker } from "nested"

// MUST NOT TRIGGER: the program also loads Effect v3, so the Effect version is ambiguous
export const sleep = Effect.sleep(0)
export const mixedPackage = marker
