import * as Console from "effect/Console"
import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import { appendFile } from "node:fs/promises"
import { buildTargets, oxlintBuildTargets, type BuildTarget } from "./build.ts"
import { getComponent, readUpstream, type ComponentName, type Upstream } from "./upstream.ts"

export class UpstreamResolveError extends Data.TaggedError("UpstreamResolveError")<{
  readonly reason: string
}> {
  get message(): string {
    return `Unable to resolve upstream component: ${this.reason}`
  }
}

export const resolveUpstreamInfo = Effect.fnUntraced(function*(
  upstream: typeof Upstream.Type,
  component: ComponentName,
  version?: string,
  target?: BuildTarget
) {
  const selected = yield* getComponent(upstream, component, version)
  if (target !== undefined) {
    const supported = component === "typescript" ? buildTargets : oxlintBuildTargets
    if (!(target in supported)) {
      return yield* new UpstreamResolveError({ reason: `${component} does not support target ${target}` })
    }
  }
  if (component === "oxlint" && target === undefined) {
    return yield* new UpstreamResolveError({ reason: "A target is required for oxlint" })
  }

  return {
    component: selected.name,
    version: selected.version,
    ...(target === undefined ? {} : { target }),
    ...(component === "oxlint"
      ? { rustTarget: oxlintBuildTargets[target as keyof typeof oxlintBuildTargets].rustTarget }
      : {
        // Go builds are cached under the TypeScript dependency identity so that
        // oxlint-tsgolint restores the compiler objects already cached by the
        // typescript validation jobs; only typescript itself persists the cache.
        goCache: {
          component: "typescript" as const,
          version: selected.typescript.version,
          save: component === "typescript"
        }
      })
  }
})

export const printUpstreamInfo = Effect.fnUntraced(function*(
  repositoryRoot: string,
  component: ComponentName,
  version?: string,
  target?: BuildTarget
) {
  const info = yield* resolveUpstreamInfo(yield* readUpstream(repositoryRoot), component, version, target)
  yield* Console.log(JSON.stringify(info))
  if (process.env.GITHUB_OUTPUT !== undefined) {
    const goCache = "goCache" in info ? info.goCache : undefined
    yield* Effect.tryPromise(() => appendFile(
      process.env.GITHUB_OUTPUT!,
      `component=${info.component}\nversion=${info.version}\nrust-target=${"rustTarget" in info ? info.rustTarget : ""}\n` +
        `go-cache-component=${goCache?.component ?? ""}\ngo-cache-version=${goCache?.version ?? ""}\ngo-cache-save=${goCache?.save === true}\n`
    ))
  }
  return info
})
