import * as Console from "effect/Console"
import * as Effect from "effect/Effect"
import { compare } from "semver"
import { runCommandString } from "./process.ts"
import { buildReleasePlan, type ReleaseArtifactPlan } from "./releasePlan.ts"
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
  // Each version is built on its own, so only the smoke test depends on the pair: test every oxlint and tsgolint
  // version at least once instead of every known pair. Start from the default pair, then greedily add the known pair
  // covering the most untested versions, preferring the newest oxlint and then the newest tsgolint.
  const untested = new Set<string>()
  for (const runtime of runtimes.values()) {
    untested.add(`oxlint\0${runtime.oxlintVersion}`)
    untested.add(`tsgolint\0${runtime.tsgolintVersion}`)
  }
  const coverage = (runtime: { readonly oxlintVersion: string; readonly tsgolintVersion: string }) =>
    Number(untested.has(`oxlint\0${runtime.oxlintVersion}`)) +
    Number(untested.has(`tsgolint\0${runtime.tsgolintVersion}`))
  const candidates = [...runtimes].sort(([, left], [, right]) =>
    compare(right.oxlintVersion, left.oxlintVersion) || compare(right.tsgolintVersion, left.tsgolintVersion)
  )
  const selected: Array<[string, typeof defaultRuntime]> = []
  let next: [string, typeof defaultRuntime] | undefined = [
    `${defaultRuntime.oxlintVersion}\0${defaultRuntime.tsgolintVersion}`,
    defaultRuntime
  ]
  while (next !== undefined) {
    selected.push(next)
    untested.delete(`oxlint\0${next[1].oxlintVersion}`)
    untested.delete(`tsgolint\0${next[1].tsgolintVersion}`)
    next = candidates.reduce<[string, typeof defaultRuntime] | undefined>(
      (best, candidate) =>
        coverage(candidate[1]) > (best === undefined ? 0 : coverage(best[1])) ? candidate : best,
      undefined
    )
  }
  return {
    include: selected
      .sort(([left], [right]) => Buffer.compare(Buffer.from(left), Buffer.from(right)))
      .map(([, runtime]) => ({
        name: `oxlint (${runtime.oxlintVersion}) + oxlint-tsgolint (${runtime.tsgolintVersion})`,
        oxlint: { component: "oxlint", version: runtime.oxlintVersion },
        tsgolint: { component: "oxlint-tsgolint", version: runtime.tsgolintVersion }
      }))
  }
}

// The cache a release job restores, and saves when no earlier job did (setup-repo): typescript, and the
// oxlint-tsgolint builds on it, share the Go caches of their TypeScript; oxlint and tsrs the Rust build of their target.
const releaseCacheGroup = (upstream: typeof Upstream.Type, artifact: ReleaseArtifactPlan) => {
  switch (artifact.component) {
    case "typescript":
      return `go\0${artifact.version}`
    case "oxlint-tsgolint":
      return `go\0${upstream.components["oxlint-tsgolint"][artifact.version]?.dependencies.typescript}`
    default:
      return `${artifact.component}\0${artifact.target}`
  }
}

// The jobs run max-parallel at a time in matrix order, so order them in rounds: the first job of every cache group,
// which fills its cache, comes before the second of any group, which then restores it instead of racing to save it.
// Within a round the longer jobs (by timeout) start first.
export const buildReleaseMatrix = (upstream: typeof Upstream.Type) => {
  const groupSizes = new Map<string, number>()
  const ranked = buildReleasePlan(upstream).map((artifact) => {
    const group = releaseCacheGroup(upstream, artifact)
    const round = groupSizes.get(group) ?? 0
    groupSizes.set(group, round + 1)
    return { artifact, round }
  })
  return {
    include: ranked
      .sort((left, right) =>
        left.round - right.round || right.artifact.timeoutMinutes - left.artifact.timeoutMinutes
      )
      .map(({ artifact }) => artifact)
  }
}

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

export const printReleaseMatrix = Effect.fnUntraced(function*(repositoryRoot: string) {
  const upstream = yield* readUpstream(repositoryRoot)
  yield* Console.log(JSON.stringify(buildReleaseMatrix(upstream)))
})
