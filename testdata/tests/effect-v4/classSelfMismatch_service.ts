import { ServiceMap, Effect } from "effect"

// @effect-expect-leaking FileSystem Cache
export class CorrectName extends ServiceMap.Service<CorrectName, {
  writeCache: () => Effect.Effect<void, never, FileSystem | Cache>
  readCache: Effect.Effect<void, never, FileSystem | Cache>
}>()("CorrectName") {}

// @effect-expect-leaking FileSystem Cache
export class WrongName extends ServiceMap.Service<CorrectName, {
  writeCache: () => Effect.Effect<void, never, FileSystem | Cache>
  readCache: Effect.Effect<void, never, FileSystem | Cache>
}>()("WrongName") {}
