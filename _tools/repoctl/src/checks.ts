import * as Console from "effect/Console"
import * as Effect from "effect/Effect"
import * as Path from "effect/Path"
import { ensureEffectFixtures } from "./fixtures.ts"
import { runCommand } from "./process.ts"

export const runChecks = Effect.fnUntraced(function*(repositoryRoot: string) {
  const path = yield* Path.Path
  yield* ensureEffectFixtures(repositoryRoot)
  yield* Console.log("Checking Go packages")
  yield* runCommand("go", repositoryRoot, ["build", "./..."], false, { CGO_ENABLED: "0" })
  yield* Console.log("Checking CLI package")
  yield* runCommand("pnpm", path.join(repositoryRoot, "_packages", "tsgo"), [
    "exec",
    "tsc",
    "-b",
    "tsconfig.json"
  ])
})
