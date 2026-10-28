import * as Console from "effect/Console"
import * as Effect from "effect/Effect"
import { runCommandString } from "./process.ts"
import { buildReleasePlan } from "./releasePlan.ts"
import { decodeUpstream, readUpstream, type Upstream } from "./upstream.ts"

export const buildTypeScriptTestMatrix = (upstream: typeof Upstream.Type) => ({
  include: Object.keys(upstream.components.typescript)
    .sort((left, right) => Buffer.compare(Buffer.from(left), Buffer.from(right)))
    .map((version) => {
      const channels = (["latest", "beta", "next"] as const)
        .filter((channel) => upstream.tags.typescript[channel] === version)
      return {
        name: channels.length === 0 ? version : channels.join("+"),
        component: "typescript",
        version,
        repoctl: upstream.tags.typescript.next === version
      }
    })
})

// A TypeScript recorded only as a tsrs component's dependency (no channel tag, oxlint-tsgolint release or profile
// uses it) is the commit tsrs ports, which has no npm release: perf has no stock compiler to compare it against.
const perfSkippedTypeScriptVersions = (upstream: typeof Upstream.Type): ReadonlySet<string> => {
  const used = new Set<string | undefined>([
    ...Object.values(upstream.tags.typescript),
    ...Object.values(upstream.components["oxlint-tsgolint"]).map((component) => component.dependencies.typescript),
    ...upstream.profiles.map((profile) => profile.dependencies.typescript)
  ])
  return new Set(
    Object.values(upstream.components.tsrs ?? {})
      .map((component) => component.typescript.version)
      .filter((version) => !used.has(version))
  )
}

type PerfSide = "base" | "head"

interface PerfBuildEntry {
  readonly name: string
  readonly side: PerfSide
  readonly ref: string
  readonly version: string
}

interface PerfMeasureEntry {
  readonly name: string
  readonly base?: { readonly ref: string; readonly version: string }
  readonly head?: { readonly ref: string; readonly version: string }
}

// build: one patched compiler per channel and side, each built with its own commit's tooling.
// measure: one entry per channel, timing stock, base, and head on a single runner; a channel present on one side
// only has just that side.
export const buildPerfMatrix = (
  base: typeof Upstream.Type,
  head: typeof Upstream.Type,
  baseRef: string,
  headRef: string
) => {
  const build: Array<PerfBuildEntry> = ([["base", base, baseRef], ["head", head, headRef]] as const).flatMap((
    [side, upstream, ref]
  ) => {
    const skipped = perfSkippedTypeScriptVersions(upstream)
    return buildTypeScriptTestMatrix(upstream).include
      .filter(({ version }) => !skipped.has(version))
      .map(({ name, version }) => ({ name, side, ref, version }))
  })
  const measure = new Map<string, PerfMeasureEntry>()
  for (const { name, ref, side, version } of build) {
    measure.set(name, { ...measure.get(name), name, [side]: { ref, version } })
  }
  return { build: { include: build }, measure: { include: [...measure.values()] } }
}

export const buildOxlintTestMatrix = (upstream: typeof Upstream.Type) => {
  const runtimes = new Map<string, {
    readonly oxlintVersion: string
    readonly tsgolintVersion: string
  }>()
  const defaultRuntime = {
    oxlintVersion: upstream.tags.oxlint.latest,
    tsgolintVersion: upstream.tags["oxlint-tsgolint"].latest
  }
  runtimes.set(`${defaultRuntime.oxlintVersion}\0${defaultRuntime.tsgolintVersion}`, defaultRuntime)
  for (const profile of upstream.profiles) {
    const oxlintVersion = profile.dependencies.oxlint
    const tsgolintVersion = profile.dependencies["oxlint-tsgolint"]
    if (oxlintVersion === undefined || tsgolintVersion === undefined) continue
    const key = `${oxlintVersion}\0${tsgolintVersion}`
    if (!runtimes.has(key)) {
      runtimes.set(key, { oxlintVersion, tsgolintVersion })
    }
  }
  return {
    include: [...runtimes]
      .sort(([left], [right]) => Buffer.compare(Buffer.from(left), Buffer.from(right)))
      .map(([, runtime]) => ({
        name: `oxlint (${runtime.oxlintVersion}) + oxlint-tsgolint (${runtime.tsgolintVersion})`,
        oxlint: { component: "oxlint", version: runtime.oxlintVersion },
        tsgolint: { component: "oxlint-tsgolint", version: runtime.tsgolintVersion }
      }))
  }
}

export const buildGeneratedMatrix = (upstream: typeof Upstream.Type) => ({
  include: [{
    name: "latest",
    component: "typescript",
    version: upstream.tags.typescript.latest,
    branch: "generated/latest"
  }]
})

export const buildReleaseMatrix = (upstream: typeof Upstream.Type) => ({
  include: buildReleasePlan(upstream)
})

export const printTypeScriptTestMatrix = Effect.fnUntraced(function*(repositoryRoot: string) {
  const upstream = yield* readUpstream(repositoryRoot)
  yield* Console.log(JSON.stringify(buildTypeScriptTestMatrix(upstream)))
})

export const printPerfMatrix = Effect.fnUntraced(function*(
  repositoryRoot: string,
  baseRef: string,
  headRef: string
) {
  const head = yield* readUpstream(repositoryRoot)
  const base = yield* decodeUpstream(
    yield* runCommandString("git", repositoryRoot, ["show", `${baseRef}:_packages/tsgo/upstream.json`])
  )
  yield* Console.log(JSON.stringify(buildPerfMatrix(base, head, baseRef, headRef)))
})

export const printOxlintTestMatrix = Effect.fnUntraced(function*(repositoryRoot: string) {
  const upstream = yield* readUpstream(repositoryRoot)
  yield* Console.log(JSON.stringify(buildOxlintTestMatrix(upstream)))
})

export const printGeneratedMatrix = Effect.fnUntraced(function*(repositoryRoot: string) {
  const upstream = yield* readUpstream(repositoryRoot)
  yield* Console.log(JSON.stringify(buildGeneratedMatrix(upstream)))
})

export const printReleaseMatrix = Effect.fnUntraced(function*(repositoryRoot: string) {
  const upstream = yield* readUpstream(repositoryRoot)
  yield* Console.log(JSON.stringify(buildReleaseMatrix(upstream)))
})
