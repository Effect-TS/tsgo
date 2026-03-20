// @effect-v3
import type { Effect } from "effect"
import { Context } from "effect"

export class FileSystem extends Context.Tag("FileSystem")<FileSystem, {
  writeFile: (content: string) => Effect.Effect<void>
}>() {}

export class Cache extends Context.Tag("Cache")<Cache, {
  writeFile: (content: string) => Effect.Effect<void>
}>() {}

// LeakingDeps is leaking FileSystem and Cache, but only Cache should be considered to be leaked

// @effect-expect-leaking FileSystem
export class LeakingDeps extends Context.Tag("LeakingDeps")<LeakingDeps, {
  writeCache: () => Effect.Effect<void, never, FileSystem | Cache>
  readCache: Effect.Effect<void, never, FileSystem | Cache>
}>() {}

// LeakingDeps2 is leaking FileSystem and Cache, but both are expected to be leaked

// @effect-expect-leaking FileSystem Cache
export class LeakingDeps2 extends Context.Tag("LeakingDeps2")<LeakingDeps2, {
  writeCache: () => Effect.Effect<void, never, FileSystem | Cache>
  readCache: Effect.Effect<void, never, FileSystem | Cache>
}>() {}

// LeakingDeps3 is leaking FileSystem and Cache, but both are expected to be leaked

/**
 * Example inside of a class with multiple JSDoc
 * @effect-leakable-service
 * @effect-expect-leaking FileSystem Cache
 */
export class LeakingDeps3 extends Context.Tag("LeakingDeps3")<LeakingDeps3, {
  writeCache: () => Effect.Effect<void, never, FileSystem | Cache>
  readCache: Effect.Effect<void, never, FileSystem | Cache>
}>() {}

// LeakingDeps4 is leaking FileSystem and Cache, but both are expected to be leaked

// @effect-expect-leaking FileSystem Cache
export const LeakingDeps4 = Context.GenericTag<{
  writeCache: () => Effect.Effect<void, never, FileSystem | Cache>
  readCache: Effect.Effect<void, never, FileSystem | Cache>
}>("LeakingDeps4")

/**
 * This comment does not suppress GenericTag diagnostics when it is attached to
 * the shape interface instead of the exported const statement.
 * @effect-expect-leaking FileSystem Cache
 */
interface LeakingDeps5Shape {
  writeCache: () => Effect.Effect<void, never, FileSystem | Cache>
  readCache: Effect.Effect<void, never, FileSystem | Cache>
}

export const LeakingDeps5 = Context.GenericTag<LeakingDeps5Shape>("LeakingDeps5")
