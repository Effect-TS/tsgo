// @effect-v4
// @effect-diagnostics *:off
// @effect-diagnostics acquireReleaseDisposable:warning
/// <reference lib="esnext.disposable" />
import { Effect } from "effect"

declare const acquire: Effect.Effect<AsyncDisposable>

export const resource = Effect.acquireRelease(
  acquire,
  (resource) => Effect.promise(() => resource[Symbol.asyncDispose]())
)
