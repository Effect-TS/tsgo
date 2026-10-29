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

// @filename: main.ts
import { BrowserRuntime } from "@effect/platform-browser"
import { BunRuntime } from "@effect/platform-bun"
import { NodeRuntime as Runtime } from "@effect/platform-node"
import { runMain as startMain } from "@effect/platform-node/NodeRuntime"
import { Effect, pipe } from "effect"

const serve = Effect.log("serving")

export const inSync = Effect.sync(() => {
  Runtime.runMain(serve)
})

export const inGen = Effect.gen(function*() {
  yield* Effect.log("starting")
  BunRuntime.runMain(serve)
})

export const inCallback = Effect.gen(function*() {
  yield* Effect.log("starting")
  return () => BrowserRuntime.runMain(serve)
})

export const inFn = Effect.fn("inFn")(function*() {
  yield* Effect.log("starting")
  startMain(serve)
})

export const inPipe = Effect.gen(function*() {
  yield* Effect.log("starting")
  serve.pipe(Runtime.runMain)
})

export const withOptions = Effect.gen(function*() {
  yield* Effect.log("starting")
  Runtime.runMain(serve, { disableErrorReporting: true })
})

export const inPipeFunction = Effect.gen(function*() {
  yield* Effect.log("starting")
  pipe(serve, BunRuntime.runMain)
})

const local = { runMain: (_: Effect.Effect<void>) => {} }

export const notPlatform = Effect.gen(function*() {
  yield* Effect.log("starting")
  local.runMain(serve)
})

export const handler = () => Runtime.runMain(serve)

Runtime.runMain(serve)
