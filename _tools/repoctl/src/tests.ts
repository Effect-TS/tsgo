import * as Console from "effect/Console"
import * as Effect from "effect/Effect"
import * as Path from "effect/Path"
import { ensureEffectFixtures } from "./fixtures.ts"
import { runCommand } from "./process.ts"

export const runTests = Effect.fnUntraced(function*(repositoryRoot: string) {
  const path = yield* Path.Path
  yield* ensureEffectFixtures(repositoryRoot)
  yield* Console.log("Running Go tests")
  yield* runCommand("go", repositoryRoot, ["test", "./..."], false, { CGO_ENABLED: "0" })
  yield* Console.log("Running shim generator tests")
  yield* runCommand("go", path.join(repositoryRoot, "_tools", "gen_shims"), ["test", "./..."], false, {
    CGO_ENABLED: "0",
    GOWORK: "off"
  })
  yield* Console.log("Running CLI tests")
  yield* runCommand("pnpm", path.join(repositoryRoot, "_packages", "tsgo"), ["exec", "vitest", "run"])
})
