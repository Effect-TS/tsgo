import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"
import * as Path from "effect/Path"
import * as Schema from "effect/Schema"
import { appendFile, mkdtemp, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { maxSatisfying } from "semver"
import { runCommand, runCommandString } from "./process.ts"

export type ComponentName = "typescript" | "oxlint-tsgolint" | "oxlint"

const NonEmptyString = Schema.String.check(Schema.isNonEmpty())
const GitRevision = Schema.String.check(Schema.isPattern(/^[0-9a-f]{40}$/))
const Component = Schema.Struct({ gitHead: GitRevision })
const TsgolintComponent = Schema.Struct({
  gitHead: GitRevision,
  dependencies: Schema.Struct({ typescript: NonEmptyString })
})
const ProfileDependencies = Schema.Record(Schema.String, NonEmptyString)
const RuntimeProfile = Schema.Struct({
  name: NonEmptyString,
  description: NonEmptyString,
  dependencies: ProfileDependencies
})
export const Upstream = Schema.Struct({
  schemaVersion: Schema.Literal(4),
  tags: Schema.Struct({
    typescript: Schema.Struct({
      latest: NonEmptyString,
      next: NonEmptyString
    }),
    oxlint: Schema.Struct({ latest: NonEmptyString }),
    "oxlint-tsgolint": Schema.Struct({ latest: NonEmptyString })
  }),
  components: Schema.Struct({
    typescript: Schema.Record(Schema.String, Component),
    "oxlint-tsgolint": Schema.Record(Schema.String, TsgolintComponent),
    oxlint: Schema.Record(Schema.String, Component)
  }),
  profiles: Schema.Array(RuntimeProfile)
})
const UpstreamFromString = Schema.fromJsonString(Upstream)

export class UpstreamManifestError extends Data.TaggedError("UpstreamManifestError")<{
  readonly reason: string
}> {
  get message(): string {
    return `Unable to read _packages/tsgo/upstream.json: ${this.reason}`
  }
}

const validateUpstream = (upstream: typeof Upstream.Type) => {
  const names = new Set<string>()
  for (const profile of upstream.profiles) {
    if (names.has(profile.name)) {
      return Effect.fail(new UpstreamManifestError({ reason: `Duplicate upstream profile: ${profile.name}` }))
    }
    names.add(profile.name)
  }
  for (const tag of ["latest", "next"] as const) {
    if (upstream.components.typescript[upstream.tags.typescript[tag]] === undefined) {
      return Effect.fail(new UpstreamManifestError({
        reason: `TypeScript ${tag} references unknown version ${upstream.tags.typescript[tag]}`
      }))
    }
  }
  for (const component of ["oxlint-tsgolint", "oxlint"] as const) {
    const version = upstream.tags[component].latest
    if (upstream.components[component][version] === undefined) {
      return Effect.fail(new UpstreamManifestError({
        reason: `${component} latest references unknown version ${version}`
      }))
    }
  }
  for (const [version, component] of Object.entries(upstream.components["oxlint-tsgolint"])) {
    if (upstream.components.typescript[component.dependencies.typescript] === undefined) {
      return Effect.fail(new UpstreamManifestError({
        reason: `oxlint-tsgolint ${version} references unknown TypeScript ${component.dependencies.typescript}`
      }))
    }
  }
  const components = upstream.components as Readonly<Record<string, Readonly<Record<string, unknown>>>>
  for (const profile of upstream.profiles) {
    for (const [component, version] of Object.entries(profile.dependencies)) {
      if (components[component]?.[version] === undefined) {
        return Effect.fail(new UpstreamManifestError({
          reason: `Profile ${profile.name} references unknown ${component} ${version}`
        }))
      }
    }
  }
  for (const name of ["vite-plus"]) {
    const profile = upstream.profiles.find((profile) => profile.name === name)
    if (profile === undefined) {
      return Effect.fail(new UpstreamManifestError({ reason: `Missing upstream profile: ${name}` }))
    }
    for (const component of ["oxlint", "oxlint-tsgolint"]) {
      if (profile.dependencies[component] === undefined) {
        return Effect.fail(new UpstreamManifestError({
          reason: `Profile ${name} must depend on ${component}`
        }))
      }
    }
  }
  if (upstream.profiles.some((profile) =>
    Object.keys(profile.dependencies).some((component) => !(component in upstream.components)))) {
    return Effect.fail(new UpstreamManifestError({
      reason: "Upstream profile contains an unknown component dependency"
    }))
  }
  return Effect.succeed(upstream)
}

export const decodeUpstream = (text: string) =>
  Schema.decodeUnknownEffect(UpstreamFromString)(text).pipe(
    Effect.mapError((error) => new UpstreamManifestError({ reason: error.message })),
    Effect.flatMap(validateUpstream)
  )

export const readUpstream = Effect.fnUntraced(function*(repositoryRoot: string) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const manifestPath = path.join(repositoryRoot, "_packages", "tsgo", "upstream.json")
  const text = yield* fs.readFileString(manifestPath).pipe(
    Effect.mapError((error) => new UpstreamManifestError({ reason: error.message }))
  )
  return yield* decodeUpstream(text)
})

export const getComponent = Effect.fnUntraced(function*(
  upstream: typeof Upstream.Type,
  name: ComponentName,
  requestedVersion?: string
) {
  const version = requestedVersion ?? (name === "typescript" ? upstream.tags.typescript.next : undefined)
  if (version === undefined) {
    return yield* new UpstreamManifestError({ reason: `A version is required for component ${name}` })
  }
  const component = name === "oxlint-tsgolint"
    ? upstream.components["oxlint-tsgolint"][version]
    : upstream.components[name][version]
  if (component === undefined) {
    return yield* new UpstreamManifestError({ reason: `Unknown ${name} component version: ${version}` })
  }
  const typescriptVersion = name === "oxlint-tsgolint"
    ? upstream.components["oxlint-tsgolint"][version]!.dependencies.typescript
    : name === "typescript" ? version : upstream.tags.typescript.next
  const typescript = upstream.components.typescript[typescriptVersion]
  if (typescript === undefined) {
    return yield* new UpstreamManifestError({
      reason: `Component ${name} ${version} references unknown TypeScript ${typescriptVersion}`
    })
  }
  return {
    name,
    version,
    gitHead: component.gitHead,
    typescript: {
      version: typescriptVersion,
      gitHead: typescript.gitHead
    }
  }
})

const NpmMetadata = Schema.fromJsonString(Schema.Struct({
  version: NonEmptyString,
  gitHead: GitRevision
}))

interface TypeScriptMetadata {
  readonly npmVersion: string
  readonly gitHead: string
}

interface TypeScriptGoCommit {
  readonly sha: string
  readonly message: string
}

interface OxlintSelection {
  readonly oxlintVersion: string
  readonly tsgolintVersion: string
}

interface VitePlusSelection extends OxlintSelection {
  readonly vitePlusVersion: string
}

const fetchTypeScriptMetadata = Effect.fnUntraced(function*(repositoryRoot: string, spec: string) {
  const output = yield* runCommandString("npm", repositoryRoot, ["view", spec, "version", "gitHead", "--json"])
  const metadata = yield* Schema.decodeUnknownEffect(NpmMetadata)(output).pipe(
    Effect.mapError((error) => new UpstreamManifestError({
      reason: `Unable to resolve ${spec}: ${error.message}`
    }))
  )
  return { npmVersion: metadata.version, gitHead: metadata.gitHead }
})

const NpmVersionResult = Schema.fromJsonString(Schema.Union([
  NonEmptyString,
  Schema.Array(NonEmptyString).check(Schema.isMinLength(1))
]))

export const decodeLatestNpmVersion = (output: string, packageSpec: string, versionSpec: string) =>
  Schema.decodeUnknownEffect(NpmVersionResult)(output).pipe(
    Effect.flatMap((result) => {
      const versions = typeof result === "string" ? [result] : result
      const version = maxSatisfying(versions, versionSpec)
      return version === null
        ? Effect.fail(new UpstreamManifestError({ reason: `No version satisfies ${packageSpec}` }))
        : Effect.succeed(version)
    }),
    Effect.mapError((error) => new UpstreamManifestError({
      reason: `Unable to resolve ${packageSpec}: ${error.message}`
    }))
  )

const resolveLatestMatchingVersion = Effect.fnUntraced(function*(
  repositoryRoot: string,
  packageName: string,
  spec: string
) {
  const packageSpec = `${packageName}@${spec}`
  const output = yield* runCommandString("npm", repositoryRoot, ["view", packageSpec, "version", "--json"])
  return yield* decodeLatestNpmVersion(output, packageSpec, spec)
})

interface ResolvedRuntime {
  readonly oxlint: TypeScriptMetadata
  readonly tsgolint: TypeScriptMetadata
  readonly ts: TypeScriptMetadata
}

interface BuildUpstreamOptions {
  readonly next: TypeScriptMetadata
  readonly latest: TypeScriptMetadata
  readonly oxlint: ResolvedRuntime
  readonly vitePlus: ResolvedRuntime & { readonly vitePlusVersion: string }
}

const sortedRecord = <A>(entries: ReadonlyArray<readonly [string, A]>): Record<string, A> =>
  Object.fromEntries([...entries].sort(([left], [right]) => Buffer.compare(Buffer.from(left), Buffer.from(right))))

export const buildUpstream = ({ latest, next, oxlint, vitePlus }: BuildUpstreamOptions): typeof Upstream.Type => {
  const typescript = new Map<string, { readonly gitHead: string }>()
  const tsgolint = new Map<string, {
    readonly gitHead: string
    readonly dependencies: { readonly typescript: string }
  }>()
  const oxlintComponents = new Map<string, { readonly gitHead: string }>()

  for (const metadata of [latest, next, oxlint.ts, vitePlus.ts]) {
    typescript.set(metadata.npmVersion, { gitHead: metadata.gitHead })
  }
  for (const runtime of [oxlint, vitePlus]) {
    tsgolint.set(runtime.tsgolint.npmVersion, {
      gitHead: runtime.tsgolint.gitHead,
      dependencies: { typescript: runtime.ts.npmVersion }
    })
    oxlintComponents.set(runtime.oxlint.npmVersion, { gitHead: runtime.oxlint.gitHead })
  }

  return {
    schemaVersion: 4,
    tags: {
      typescript: {
        latest: latest.npmVersion,
        next: next.npmVersion
      },
      oxlint: { latest: oxlint.oxlint.npmVersion },
      "oxlint-tsgolint": { latest: oxlint.tsgolint.npmVersion }
    },
    components: {
      typescript: sortedRecord([...typescript]),
      "oxlint-tsgolint": sortedRecord([...tsgolint]),
      oxlint: sortedRecord([...oxlintComponents])
    },
    profiles: [
      {
        name: "vite-plus",
        description: `Vite+ ${vitePlus.vitePlusVersion} compatibility runtime`,
        dependencies: {
          oxlint: vitePlus.oxlint.npmVersion,
          "oxlint-tsgolint": vitePlus.tsgolint.npmVersion
        }
      }
    ]
  }
}

export const findTypeScriptVersion = (
  versions: Readonly<Record<string, { readonly gitHead?: unknown }>>,
  gitHead: string
) => Object.entries(versions).find(([, metadata]) => metadata.gitHead === gitHead)?.[0]

export const formatTSConfigSchema = (value: unknown): string | undefined => {
  if (typeof value !== "object" || value === null || Array.isArray(value) ||
    !("definitions" in value) || typeof value.definitions !== "object" || value.definitions === null ||
    Array.isArray(value.definitions)) {
    return undefined
  }
  return `${JSON.stringify(value, null, 2)}\n`
}

export const formatOxlintConfigurationSchema = (value: unknown): string | undefined => {
  if (typeof value !== "object" || value === null || Array.isArray(value) ||
    !("definitions" in value) || typeof value.definitions !== "object" || value.definitions === null ||
    Array.isArray(value.definitions) || !("properties" in value) || typeof value.properties !== "object" ||
    value.properties === null || Array.isArray(value.properties)) {
    return undefined
  }
  return `${JSON.stringify(value, null, 2)}\n`
}

const fetchJson = Effect.fnUntraced(function*(url: string, authenticate = false) {
  const token = authenticate ? process.env.GH_TOKEN ?? process.env.GITHUB_TOKEN : undefined
  const response = yield* Effect.tryPromise({
    try: () => fetch(url, {
      headers: {
        "User-Agent": "effect-tsgo-repoctl",
        ...(token === undefined ? {} : { Authorization: `Bearer ${token}` })
      }
    }),
    catch: (error) => new UpstreamManifestError({ reason: `Unable to fetch ${url}: ${String(error)}` })
  })
  if (!response.ok) {
    return yield* new UpstreamManifestError({ reason: `Unable to fetch ${url}: HTTP ${response.status}` })
  }
  return yield* Effect.tryPromise({
    try: () => response.json(),
    catch: (error) => new UpstreamManifestError({ reason: `Unable to parse ${url}: ${String(error)}` })
  })
})

const GitHubComparison = Schema.Struct({
  total_commits: Schema.Number,
  commits: Schema.Array(Schema.Struct({
    sha: GitRevision,
    commit: Schema.Struct({ message: NonEmptyString })
  }))
})

const fetchTypeScriptGoCommits = Effect.fnUntraced(function*(before: string, after: string) {
  if (before === after) {
    return []
  }
  const commits: Array<TypeScriptGoCommit> = []
  let page = 1
  let totalCommits = 0
  do {
    const url = [
      `https://api.github.com/repos/microsoft/typescript-go/compare/${before}...${after}`,
      `?per_page=100&page=${page}`
    ].join("")
    const comparison = yield* fetchJson(url, true)
    const decoded = yield* Schema.decodeUnknownEffect(GitHubComparison)(comparison).pipe(
      Effect.mapError((error) => new UpstreamManifestError({
        reason: `Unable to read TypeScript-Go comparison: ${error.message}`
      }))
    )
    totalCommits = decoded.total_commits
    if (decoded.commits.length === 0 && commits.length < totalCommits) {
      return yield* new UpstreamManifestError({ reason: "TypeScript-Go comparison ended before all commits were read" })
    }
    commits.push(...decoded.commits.map(({ commit, sha }) => ({
      sha,
      message: commit.message.split(/[\r\n]/, 1)[0]!
    })))
    page++
  } while (commits.length < totalCommits)
  return commits
})

export interface UpstreamUpdateDescriptionOptions {
  readonly before: typeof Upstream.Type
  readonly after: typeof Upstream.Type
  readonly nextCommits: ReadonlyArray<TypeScriptGoCommit>
  readonly schemaChanged: boolean
  readonly oxlintSchemaChanged: boolean
}

export const formatUpstreamUpdateDescription = ({
  after,
  before,
  nextCommits,
  oxlintSchemaChanged,
  schemaChanged
}: UpstreamUpdateDescriptionOptions): string => {
  const beforeVitePlus = before.profiles.find(({ name }) => name === "vite-plus")!
  const afterVitePlus = after.profiles.find(({ name }) => name === "vite-plus")!
  const beforeVitePlusVersion = /^Vite\+ (.+) compatibility runtime$/.exec(beforeVitePlus.description)?.[1]
  const afterVitePlusVersion = /^Vite\+ (.+) compatibility runtime$/.exec(afterVitePlus.description)?.[1]
  const versionUpdates = [
    {
      label: "TypeScript next",
      packageName: "typescript",
      spec: "typescript@next",
      previous: before.tags.typescript.next,
      updated: after.tags.typescript.next
    },
    {
      label: "TypeScript latest",
      packageName: "typescript",
      spec: "typescript@latest",
      previous: before.tags.typescript.latest,
      updated: after.tags.typescript.latest
    },
    {
      label: "Oxlint",
      packageName: "oxlint",
      spec: "oxlint@latest",
      previous: before.tags.oxlint.latest,
      updated: after.tags.oxlint.latest
    },
    {
      label: "Oxlint TypeScript-Go lint plugin",
      packageName: "oxlint-tsgolint",
      spec: "oxlint-tsgolint@latest",
      previous: before.tags["oxlint-tsgolint"].latest,
      updated: after.tags["oxlint-tsgolint"].latest
    },
    {
      label: "Vite+",
      packageName: "vite-plus",
      spec: "vite-plus@latest",
      previous: beforeVitePlusVersion,
      updated: afterVitePlusVersion
    },
    {
      label: "Vite+ Oxlint runtime",
      packageName: "oxlint",
      spec: "oxlint",
      previous: beforeVitePlus.dependencies.oxlint,
      updated: afterVitePlus.dependencies.oxlint
    },
    {
      label: "Vite+ TypeScript-Go lint runtime",
      packageName: "oxlint-tsgolint",
      spec: "oxlint-tsgolint",
      previous: beforeVitePlus.dependencies["oxlint-tsgolint"],
      updated: afterVitePlus.dependencies["oxlint-tsgolint"]
    }
  ].filter((update): update is typeof update & { readonly previous: string; readonly updated: string } =>
    update.previous !== undefined && update.updated !== undefined && update.previous !== update.updated)
  const previousNext = before.components.typescript[before.tags.typescript.next]!.gitHead
  const updatedNext = after.components.typescript[after.tags.typescript.next]!.gitHead
  const sections = ["Automated update of upstream metadata, generated TypeScript next-tag sources, and Nix inputs."]

  if (versionUpdates.length > 0) {
    sections.push([
      "## Version updates",
      "",
      ...versionUpdates.map(({ label, packageName, previous, spec, updated }) =>
        `- ${label}: [\`${spec}\`](https://www.npmjs.com/package/${packageName}/v/${updated}) \`${previous}\` -> \`${updated}\``)
    ].join("\n"))
  }
  if (previousNext !== updatedNext) {
    sections.push([
      "## TypeScript-Go",
      "",
      `- Previous commit: [\`${previousNext}\`](https://github.com/microsoft/typescript-go/commit/${previousNext})`,
      `- Updated commit: [\`${updatedNext}\`](https://github.com/microsoft/typescript-go/commit/${updatedNext})`,
      `- Compare: https://github.com/microsoft/typescript-go/compare/${previousNext}...${updatedNext}`
    ].join("\n"))
  }
  if (nextCommits.length > 0) {
    sections.push([
      "## Upstream commits",
      "",
      ...nextCommits.map(({ message, sha }) =>
        `- [${sha.slice(0, 7)}](https://github.com/microsoft/typescript-go/commit/${sha}) ${message}`)
    ].join("\n"))
  }
  const otherUpdates = [
    ...(schemaChanged ? ["- Refreshed the tsconfig schema from JSON Schema Store."] : []),
    ...(oxlintSchemaChanged ? ["- Refreshed the Oxlint configuration schema from the selected package."] : [])
  ]
  if (otherUpdates.length > 0) {
    sections.push(["## Other updates", "", ...otherUpdates].join("\n"))
  }
  return sections.join("\n\n")
}

export const formatGitHubOutputs = (outputs: Readonly<Record<string, string>>) =>
  Object.entries(outputs).map(([name, value]) => {
    const normalizedValue = value.replace(/\r\n?/g, "\n")
    if (!normalizedValue.includes("\n")) {
      return `${name}=${normalizedValue}\n`
    }
    let delimiter = `repoctl_${name}`
    const lines = new Set(normalizedValue.split("\n"))
    while (lines.has(delimiter)) {
      delimiter += "_"
    }
    return `${name}<<${delimiter}\n${normalizedValue}\n${delimiter}\n`
  }).join("")

const resolveRemoteTag = Effect.fnUntraced(function*(repositoryRoot: string, repository: string, tag: string) {
  const output = yield* runCommandString("git", repositoryRoot, [
    "ls-remote",
    repository,
    `refs/tags/${tag}`,
    `refs/tags/${tag}^{}`
  ])
  const revisions = new Map(output.trim().split("\n").filter(Boolean).map((line) => {
    const [revision, ref] = line.split(/\s+/, 2)
    return [ref, revision] as const
  }))
  const revision = revisions.get(`refs/tags/${tag}^{}`) ?? revisions.get(`refs/tags/${tag}`)
  if (revision === undefined || !/^[0-9a-f]{40}$/.test(revision)) {
    return yield* new UpstreamManifestError({ reason: `Unable to resolve ${repository} tag ${tag}` })
  }
  return revision
})

const readRemoteGitlink = (repository: string, revision: string, gitlink: string) => Effect.scoped(Effect.gen(function*() {
  const directory = yield* Effect.acquireRelease(
    Effect.tryPromise({
      try: () => mkdtemp(join(tmpdir(), "effect-tsgo-upstream-")),
      catch: (error) => new UpstreamManifestError({ reason: `Unable to create temporary repository: ${String(error)}` })
    }),
    (directory) => Effect.promise(() => rm(directory, { recursive: true, force: true }))
  )
  yield* runCommand("git", directory, ["init", "--quiet", "--bare"])
  yield* runCommand("git", directory, ["fetch", "--quiet", "--depth", "1", repository, revision])
  const output = (yield* runCommandString("git", directory, ["ls-tree", "FETCH_HEAD", gitlink])).trim()
  const match = /^160000 commit ([0-9a-f]{40})\s/.exec(output)
  if (match?.[1] === undefined) {
    return yield* new UpstreamManifestError({ reason: `Unable to resolve ${gitlink} at ${revision}` })
  }
  return match[1]
}))

const fetchLatestOxlintSelection = Effect.fnUntraced(function*(repositoryRoot: string) {
  const oxlintOutput = yield* runCommandString("npm", repositoryRoot, [
    "view",
    "oxlint@latest",
    "version",
    "peerDependencies",
    "--json"
  ])
  const oxlintPackage = yield* Schema.decodeUnknownEffect(Schema.fromJsonString(Schema.Struct({
    version: NonEmptyString,
    peerDependencies: Schema.Struct({ "oxlint-tsgolint": NonEmptyString })
  })))(oxlintOutput).pipe(
    Effect.mapError((error) => new UpstreamManifestError({ reason: `Unable to resolve oxlint@latest: ${error.message}` }))
  )
  return {
    oxlintVersion: oxlintPackage.version,
    tsgolintVersion: yield* resolveLatestMatchingVersion(
      repositoryRoot,
      "oxlint-tsgolint",
      oxlintPackage.peerDependencies["oxlint-tsgolint"]
    )
  }
})

const fetchVitePlusSelection = Effect.fnUntraced(function*(repositoryRoot: string) {
  const output = yield* runCommandString("npm", repositoryRoot, [
    "view",
    "vite-plus@latest",
    "version",
    "dependencies",
    "--json"
  ])
  const vitePlus = yield* Schema.decodeUnknownEffect(Schema.fromJsonString(Schema.Struct({
    version: NonEmptyString,
    dependencies: Schema.Struct({
      oxlint: NonEmptyString,
      "oxlint-tsgolint": NonEmptyString
    })
  })))(output).pipe(
    Effect.mapError((error) => new UpstreamManifestError({
      reason: `Unable to resolve vite-plus@latest: ${error.message}`
    }))
  )
  const [oxlintVersion, tsgolintVersion] = yield* Effect.all([
    resolveLatestMatchingVersion(repositoryRoot, "oxlint", vitePlus.dependencies.oxlint),
    resolveLatestMatchingVersion(repositoryRoot, "oxlint-tsgolint", vitePlus.dependencies["oxlint-tsgolint"])
  ], { concurrency: "unbounded" })
  return { vitePlusVersion: vitePlus.version, oxlintVersion, tsgolintVersion }
})

const resolveOxlintComponent = Effect.fnUntraced(function*(repositoryRoot: string, npmVersion: string) {
  const gitHead = yield* resolveRemoteTag(
    repositoryRoot,
    "https://github.com/oxc-project/oxc.git",
    `apps_v${npmVersion}`
  )
  return { npmVersion, gitHead }
})

const resolveTsgolintComponent = Effect.fnUntraced(function*(
  repositoryRoot: string,
  npmVersion: string,
  typescriptVersions: Readonly<Record<string, { readonly gitHead?: unknown }>>
) {
  const gitHead = yield* resolveRemoteTag(
    repositoryRoot,
    "https://github.com/oxc-project/tsgolint.git",
    `v${npmVersion}`
  )
  const typescriptGitHead = yield* readRemoteGitlink(
    "https://github.com/oxc-project/tsgolint.git",
    gitHead,
    "typescript-go"
  )
  const typescriptVersion = findTypeScriptVersion(
    typescriptVersions,
    typescriptGitHead
  )
  if (typescriptVersion === undefined) {
    return yield* new UpstreamManifestError({
      reason: `No TypeScript npm version has git head ${typescriptGitHead}`
    })
  }
  return {
    npmVersion,
    gitHead,
    ts: { npmVersion: typescriptVersion, gitHead: typescriptGitHead }
  }
})

const resolveRuntime = Effect.fnUntraced(function*(
  selection: OxlintSelection,
  oxlint: ReadonlyMap<string, TypeScriptMetadata>,
  tsgolint: ReadonlyMap<string, TypeScriptMetadata & { readonly ts: TypeScriptMetadata }>
) {
  const resolvedOxlint = oxlint.get(selection.oxlintVersion)
  const resolvedTsgolint = tsgolint.get(selection.tsgolintVersion)
  if (resolvedOxlint === undefined || resolvedTsgolint === undefined) {
    return yield* new UpstreamManifestError({ reason: "Unable to resolve selected Oxlint runtime components" })
  }
  return { oxlint: resolvedOxlint, tsgolint: resolvedTsgolint, ts: resolvedTsgolint.ts }
})

export const updateUpstream = Effect.fnUntraced(function*(repositoryRoot: string) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const upstream = yield* readUpstream(repositoryRoot)
  const nextBefore = {
    npmVersion: upstream.tags.typescript.next,
    gitHead: upstream.components.typescript[upstream.tags.typescript.next]!.gitHead
  }
  const latestBefore = {
    npmVersion: upstream.tags.typescript.latest,
    gitHead: upstream.components.typescript[upstream.tags.typescript.latest]!.gitHead
  }
  const [next, latest, oxlintSelection, vitePlusSelection, remoteSchema, packument] = yield* Effect.all([
    fetchTypeScriptMetadata(repositoryRoot, "typescript@next"),
    fetchTypeScriptMetadata(repositoryRoot, "typescript@latest"),
    fetchLatestOxlintSelection(repositoryRoot),
    fetchVitePlusSelection(repositoryRoot),
    fetchJson("https://json.schemastore.org/tsconfig"),
    fetchJson("https://registry.npmjs.org/typescript")
  ], { concurrency: "unbounded" })
  if (typeof packument !== "object" || packument === null || !("versions" in packument) ||
    typeof packument.versions !== "object" || packument.versions === null) {
    return yield* new UpstreamManifestError({ reason: "TypeScript npm metadata does not contain versions" })
  }
  const oxlintVersions = Array.from(new Set([
    oxlintSelection.oxlintVersion,
    vitePlusSelection.oxlintVersion
  ])).sort((left, right) => Buffer.compare(Buffer.from(left), Buffer.from(right)))
  const tsgolintVersions = Array.from(new Set([
    oxlintSelection.tsgolintVersion,
    vitePlusSelection.tsgolintVersion
  ])).sort((left, right) => Buffer.compare(Buffer.from(left), Buffer.from(right)))
  const [resolvedOxlint, resolvedTsgolint] = yield* Effect.all([
    Effect.forEach(oxlintVersions, (version) => resolveOxlintComponent(repositoryRoot, version), {
      concurrency: "unbounded"
    }),
    Effect.forEach(tsgolintVersions, (version) => resolveTsgolintComponent(
      repositoryRoot,
      version,
      packument.versions as Record<string, { readonly gitHead?: unknown }>
    ), { concurrency: "unbounded" })
  ], { concurrency: "unbounded" })
  const oxlintByVersion = new Map(resolvedOxlint.map((component) => [component.npmVersion, component]))
  const tsgolintByVersion = new Map(resolvedTsgolint.map((component) => [component.npmVersion, component]))
  const oxlint = yield* resolveRuntime(oxlintSelection, oxlintByVersion, tsgolintByVersion)
  const vitePlus = {
    ...(yield* resolveRuntime(vitePlusSelection, oxlintByVersion, tsgolintByVersion)),
    vitePlusVersion: vitePlusSelection.vitePlusVersion
  }
  const schema = formatTSConfigSchema(remoteSchema)
  if (schema === undefined) {
    return yield* new UpstreamManifestError({ reason: "The JSON Schema Store tsconfig schema is invalid" })
  }
  const remoteOxlintSchema = yield* fetchJson(
    `https://unpkg.com/oxlint@${oxlint.oxlint.npmVersion}/configuration_schema.json`
  )
  const oxlintSchema = formatOxlintConfigurationSchema(remoteOxlintSchema)
  if (oxlintSchema === undefined) {
    return yield* new UpstreamManifestError({
      reason: `The oxlint@${oxlint.oxlint.npmVersion} configuration schema is invalid`
    })
  }
  const updated = buildUpstream({ next, latest, oxlint, vitePlus })
  const metadataChanged = JSON.stringify(updated) !== JSON.stringify(upstream)
  const schemaPath = path.join(repositoryRoot, "_tools", "tsconfig-base-schema.json")
  const currentSchema = yield* fs.readFileString(schemaPath).pipe(
    Effect.mapError((error) => new UpstreamManifestError({ reason: error.message }))
  )
  const schemaChanged = schema !== currentSchema
  const oxlintSchemaPath = path.join(repositoryRoot, "_tools", "oxlint-configuration-base-schema.json")
  const currentOxlintSchema = yield* fs.readFileString(oxlintSchemaPath).pipe(
    Effect.mapError((error) => new UpstreamManifestError({ reason: error.message }))
  )
  const oxlintSchemaChanged = oxlintSchema !== currentOxlintSchema
  const hasChanges = metadataChanged || schemaChanged || oxlintSchemaChanged

  const nextCommits = yield* fetchTypeScriptGoCommits(nextBefore.gitHead, next.gitHead)
  const description = formatUpstreamUpdateDescription({
    before: upstream,
    after: updated,
    nextCommits,
    schemaChanged,
    oxlintSchemaChanged
  })

  if (metadataChanged) {
    yield* fs.writeFileString(
      path.join(repositoryRoot, "_packages", "tsgo", "upstream.json"),
      `${JSON.stringify(updated, null, 2)}\n`
    ).pipe(Effect.mapError((error) => new UpstreamManifestError({ reason: error.message })))
  }
  if (schemaChanged) {
    yield* fs.writeFileString(schemaPath, schema).pipe(
      Effect.mapError((error) => new UpstreamManifestError({ reason: error.message }))
    )
  }
  if (oxlintSchemaChanged) {
    yield* fs.writeFileString(oxlintSchemaPath, oxlintSchema).pipe(
      Effect.mapError((error) => new UpstreamManifestError({ reason: error.message }))
    )
  }

  const outputs = {
    has_changes: String(hasChanges),
    schema_changed: String(schemaChanged),
    oxlint_schema_changed: String(oxlintSchemaChanged),
    next_spec: "typescript@next",
    next_previous_version: nextBefore.npmVersion,
    next_previous_git_head: nextBefore.gitHead,
    next_version: next.npmVersion,
    next_git_head: next.gitHead,
    latest_spec: "typescript@latest",
    latest_previous_version: latestBefore.npmVersion,
    latest_previous_git_head: latestBefore.gitHead,
    latest_version: latest.npmVersion,
    latest_git_head: latest.gitHead,
    description
  }
  if (process.env.GITHUB_OUTPUT !== undefined) {
    yield* Effect.tryPromise(() => appendFile(
      process.env.GITHUB_OUTPUT!,
      formatGitHubOutputs(outputs)
    ))
  }
  yield* Effect.log(hasChanges ? "Updated upstream metadata" : "Upstream metadata is current")
  return outputs
})
