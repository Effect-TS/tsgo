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
        component: "typescript",
        version,
        repoctl: upstream.typescript.next === version
      }
    })
})

export const buildOxlintTestMatrix = (upstream: typeof Upstream.Type) => {
  const runtimes = new Map<string, {
    readonly names: Array<string>
    readonly oxlintVersion: string
    readonly tsgolintVersion: string
  }>()
  for (const profile of upstream.profiles) {
    const oxlintVersion = profile.dependencies.oxlint
    const tsgolintVersion = profile.dependencies["oxlint-tsgolint"]
    if (oxlintVersion === undefined || tsgolintVersion === undefined) continue
    const key = `${oxlintVersion}\0${tsgolintVersion}`
    const runtime = runtimes.get(key)
    if (runtime === undefined) {
      runtimes.set(key, { names: [profile.name], oxlintVersion, tsgolintVersion })
    } else {
      runtime.names.push(profile.name)
    }
  }
  return {
    include: [...runtimes]
      .sort(([left], [right]) => Buffer.compare(Buffer.from(left), Buffer.from(right)))
      .map(([, runtime]) => ({
        name: runtime.names.sort((left, right) => Buffer.compare(Buffer.from(left), Buffer.from(right))).join("+"),
        oxlint: { component: "oxlint", version: runtime.oxlintVersion },
        tsgolint: { component: "oxlint-tsgolint", version: runtime.tsgolintVersion }
      }))
  }
}

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

export const printOxlintTestMatrix = Effect.fnUntraced(function*(repositoryRoot: string) {
  const upstream = yield* readUpstream(repositoryRoot)
  yield* Console.log(JSON.stringify(buildOxlintTestMatrix(upstream)))
})

export const printGeneratedMatrix = Effect.fnUntraced(function*(repositoryRoot: string) {
  const upstream = yield* readUpstream(repositoryRoot)
  yield* Console.log(JSON.stringify(buildGeneratedMatrix(upstream)))
})
