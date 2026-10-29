// @filename: /node_modules/@effect/platform-node/package.json
{ "name": "@effect/platform-node", "version": "0.94.0", "types": "index.d.ts" }

// @filename: /node_modules/@effect/platform-node/index.d.ts
export * as NodeRuntime from "./NodeRuntime.js"

// @filename: /node_modules/@effect/platform-node/NodeRuntime.d.ts
import type { Effect } from "effect/Effect"
export declare const runMain: {
  (options?: { readonly disableErrorReporting?: boolean | undefined }): <E, A>(effect: Effect<A, E>) => void
  <E, A>(effect: Effect<A, E>, options?: { readonly disableErrorReporting?: boolean | undefined }): void
}

// @filename: /node_modules/@effect/platform-bun/package.json
{ "name": "@effect/platform-bun", "version": "0.94.0", "types": "index.d.ts" }

// @filename: /node_modules/@effect/platform-bun/index.d.ts
export * as BunRuntime from "./BunRuntime.js"

// @filename: /node_modules/@effect/platform-bun/BunRuntime.d.ts
import type { Effect } from "effect/Effect"
export declare const runMain: {
  (options?: { readonly disableErrorReporting?: boolean | undefined }): <E, A>(effect: Effect<A, E>) => void
  <E, A>(effect: Effect<A, E>, options?: { readonly disableErrorReporting?: boolean | undefined }): void
}

// @filename: /node_modules/@effect/platform-browser/package.json
{ "name": "@effect/platform-browser", "version": "0.94.0", "types": "index.d.ts" }

// @filename: /node_modules/@effect/platform-browser/index.d.ts
export * as BrowserRuntime from "./BrowserRuntime.js"

// @filename: /node_modules/@effect/platform-browser/BrowserRuntime.d.ts
import type { Effect } from "effect/Effect"
export declare const runMain: {
  (options?: { readonly disableErrorReporting?: boolean | undefined }): <E, A>(effect: Effect<A, E>) => void
  <E, A>(effect: Effect<A, E>, options?: { readonly disableErrorReporting?: boolean | undefined }): void
}

// @filename: services.ts
import * as Effect from "effect/Effect"
import * as Layer from "effect/Layer"

export class MyService1 extends Effect.Service<MyService1>()("MyService1", {
  succeed: { value: 1 }
}) {}

export class MyService2 extends Effect.Service<MyService2>()("MyService2", {
  succeed: { value: 2 }
}) {}

export const AppLayer = Layer.mergeAll(MyService1.Default, MyService2.Default)

export const program = Effect.gen(function*() {
  const service1 = yield* MyService1
  const service2 = yield* MyService2
  return service1.value + service2.value
})

// @filename: main.ts
// @effect-diagnostics strictEffectProvide:warning
import { BrowserRuntime } from "@effect/platform-browser"
import { BunRuntime } from "@effect/platform-bun"
import { NodeRuntime } from "@effect/platform-node"
import { runMain as startMain } from "@effect/platform-node/NodeRuntime"
import * as Effect from "effect/Effect"
import { pipe } from "effect/Function"
import { AppLayer, program } from "./services"

// Should NOT report: the runner is the next step of the pipe
program.pipe(Effect.provide(AppLayer), Effect.runPromise)
pipe(program, Effect.asVoid, Effect.provide(AppLayer), Effect.runFork)
program.pipe(Effect.provide(AppLayer), NodeRuntime.runMain)
program.pipe(Effect.provide(AppLayer), BunRuntime.runMain({ disableErrorReporting: true }))

// Should NOT report: the provided Effect is the runner's argument
Effect.runPromise(program.pipe(Effect.provide(AppLayer)))
Effect.runSync(Effect.provide(program, AppLayer))
Effect.runSyncExit((Effect.provide(program, AppLayer)))
Effect.runPromiseExit(Effect.provide(AppLayer)(program))
Effect.runCallback(Effect.provide(program, AppLayer))
Effect.runPromise(program.pipe(Effect.provide(AppLayer)), { signal: new AbortController().signal })
NodeRuntime.runMain(Effect.provide(program, AppLayer))
BunRuntime.runMain(program.pipe(Effect.provide(AppLayer)), { disableErrorReporting: true })
BrowserRuntime.runMain(program.pipe(Effect.provide(AppLayer)))
startMain(Effect.provide(program, AppLayer))

// Should report: the provided Effect is bound to a variable first
export const handler = program.pipe(Effect.provide(AppLayer))
const provided = Effect.provide(program, AppLayer)
Effect.runPromise(provided)

// Should report: another step follows the provide before the runner
Effect.runPromise(program.pipe(Effect.provide(AppLayer), Effect.asVoid))
program.pipe(Effect.provide(AppLayer), Effect.asVoid, NodeRuntime.runMain)

// Should report: the provided Effect is not run
export const inEffect = Effect.sync(() => Effect.provide(program, AppLayer))
Effect.runPromise(Effect.void).then(() => Effect.provide(program, AppLayer))

// Should report: a similarly named local function is not a runner
const FakeEffect = { runPromise: <A>(value: A) => value }
export const fakeStep = program.pipe(Effect.provide(AppLayer), FakeEffect.runPromise)
export const fakeCall = FakeEffect.runPromise(Effect.provide(program, AppLayer))
