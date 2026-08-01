import * as Console from "effect/Console"
import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"
import * as Path from "effect/Path"
import { buildTargets } from "./build.ts"
import { readUpstream } from "./upstream.ts"

export const bundleUpstream = Effect.fnUntraced(function*(repositoryRoot: string) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  yield* readUpstream(repositoryRoot)
  const source = path.join(repositoryRoot, "_packages", "tsgo", "upstream.json")

  for (const target of Object.keys(buildTargets)) {
    const destination = path.join(repositoryRoot, "_packages", `tsgo-${target}`, "lib", "upstream.json")
    yield* fs.makeDirectory(path.dirname(destination), { recursive: true })
    yield* fs.copyFile(source, destination)
  }
  yield* Console.log("Bundled upstream.json in platform packages")
})
