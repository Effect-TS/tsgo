// @filename: /node_modules/@effect/platform-node/package.json
{ "name": "@effect/platform-node", "version": "0.108.0", "types": "index.d.ts" }

// @filename: /node_modules/@effect/platform-node/index.d.ts
export * as NodeRuntime from "./NodeRuntime.js"

// @filename: /node_modules/@effect/platform-node/NodeRuntime.d.ts
import type { Effect } from "effect/Effect"
export interface RunMain {
  (options?: { readonly disableErrorReporting?: boolean | undefined }): <E, A>(effect: Effect<A, E>) => void
  <E, A>(effect: Effect<A, E>, options?: { readonly disableErrorReporting?: boolean | undefined }): void
}
export declare const runMain: RunMain

// @filename: /node_modules/@effect/platform-bun/package.json
{ "name": "@effect/platform-bun", "version": "0.108.0", "types": "index.d.ts" }

// @filename: /node_modules/@effect/platform-bun/index.d.ts
export * as BunRuntime from "./BunRuntime.js"

// @filename: /node_modules/@effect/platform-bun/BunRuntime.d.ts
import type { Effect } from "effect/Effect"
export interface RunMain {
  (options?: { readonly disableErrorReporting?: boolean | undefined }): <E, A>(effect: Effect<A, E>) => void
  <E, A>(effect: Effect<A, E>, options?: { readonly disableErrorReporting?: boolean | undefined }): void
}
export declare const runMain: RunMain

// @filename: /node_modules/@effect/platform-browser/package.json
{ "name": "@effect/platform-browser", "version": "0.108.0", "types": "index.d.ts" }

// @filename: /node_modules/@effect/platform-browser/index.d.ts
export * as BrowserRuntime from "./BrowserRuntime.js"

// @filename: /node_modules/@effect/platform-browser/BrowserRuntime.d.ts
import type { Effect } from "effect/Effect"
export interface RunMain {
  (options?: { readonly disableErrorReporting?: boolean | undefined }): <E, A>(effect: Effect<A, E>) => void
  <E, A>(effect: Effect<A, E>, options?: { readonly disableErrorReporting?: boolean | undefined }): void
}
export declare const runMain: RunMain

// @filename: handlers.ts
import { BrowserRuntime } from "@effect/platform-browser"
import { BunRuntime } from "@effect/platform-bun"
import { NodeRuntime } from "@effect/platform-node"
import { NodeRuntime as Runtime } from "@effect/platform-node"
import * as DeepNodeRuntime from "@effect/platform-node/NodeRuntime"
import { runMain as startMain } from "@effect/platform-node/NodeRuntime"
import { Effect } from "effect"

const serve = Effect.log("serving")

export const inSync = Effect.sync(() => {
  NodeRuntime.runMain(serve)
})

export const inGen = Effect.gen(function*() {
  yield* Effect.log("starting")
  NodeRuntime.runMain(serve)
})

export const inFn = Effect.fn("inFn")(function*() {
  yield* Effect.log("starting")
  BunRuntime.runMain(serve)
})

export const inNestedCallback = Effect.gen(function*() {
  yield* Effect.log("starting")
  setTimeout(() => {
    BrowserRuntime.runMain(serve)
  }, 0)
})

export const aliased = Effect.gen(function*() {
  yield* Effect.log("starting")
  Runtime.runMain(serve)
  DeepNodeRuntime.runMain(serve)
  startMain(serve)
})

export const curried = Effect.gen(function*() {
  yield* Effect.log("starting")
  NodeRuntime.runMain({ disableErrorReporting: true })(serve)
})

// not a platform runtime: same names, but declared locally
const LocalRuntime = { runMain: (_: Effect.Effect<void>) => {} }
export const localLookalike = Effect.gen(function*() {
  yield* Effect.log("starting")
  LocalRuntime.runMain(serve)
})

// not inside an Effect context
export function startOutsideEffect() {
  NodeRuntime.runMain(serve)
}

// the handler returns its Effect
export const handler = serve

// @filename: main.ts
import { NodeRuntime } from "@effect/platform-node"
import { handler } from "./handlers"

// the only runMain, at the entrypoint
NodeRuntime.runMain(handler)
handler.pipe(NodeRuntime.runMain)
