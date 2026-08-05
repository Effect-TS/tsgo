import * as Console from "effect/Console"
import * as Effect from "effect/Effect"
import { readUpstream, type Upstream } from "./upstream.ts"

export const buildTypeScriptTestMatrix = (upstream: typeof Upstream.Type) => ({
  include: Object.keys(upstream.components.typescript)
    .sort((left, right) => Buffer.compare(Buffer.from(left), Buffer.from(right)))
    .map((version) => {
      const channels = (["latest", "next"] as const)
        .filter((channel) => upstream.typescript[channel] === version)
      return {
        name: channels.length === 0 ? version : channels.join("+"),
        version,
        repoctl: upstream.typescript.next === version
      }
    })
})

export const buildGeneratedMatrix = (upstream: typeof Upstream.Type) => ({
  include: [{
    name: "latest",
    component: "typescript",
    version: upstream.typescript.latest,
    branch: "generated/latest"
  }]
})

export const printTypeScriptTestMatrix = Effect.fnUntraced(function*(repositoryRoot: string) {
  const upstream = yield* readUpstream(repositoryRoot)
  yield* Console.log(JSON.stringify(buildTypeScriptTestMatrix(upstream)))
})

export const printGeneratedMatrix = Effect.fnUntraced(function*(repositoryRoot: string) {
  const upstream = yield* readUpstream(repositoryRoot)
  yield* Console.log(JSON.stringify(buildGeneratedMatrix(upstream)))
})
