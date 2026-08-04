import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"
import * as Path from "effect/Path"
import * as Schema from "effect/Schema"
import { appendFile, mkdtemp, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { runCommand, runCommandString } from "./process.ts"

export type ProfileName = "next" | "latest" | "oxlint"

const NonEmptyString = Schema.String.check(Schema.isNonEmpty())
const GitRevision = Schema.String.check(Schema.isPattern(/^[0-9a-f]{40}$/))
const UpstreamIdentity = Schema.Struct({
  npmVersion: NonEmptyString,
  gitHead: GitRevision
})
const TypeScriptProfile = Schema.Struct({
  kind: Schema.Literal("ts"),
  name: NonEmptyString,
  ts: UpstreamIdentity,
  binName: Schema.Literals(["tsc", "tsc-next"])
})
const OxlintProfile = Schema.Struct({
  kind: Schema.Literal("oxlint"),
  name: NonEmptyString,
  ts: UpstreamIdentity,
  tsgolint: UpstreamIdentity,
  oxlint: UpstreamIdentity
})
export const Upstream = Schema.Struct({
  schemaVersion: Schema.Literal(2),
  profiles: Schema.Array(Schema.Union([TypeScriptProfile, OxlintProfile])).check(Schema.isMinLength(1))
})
const UpstreamFromString = Schema.fromJsonString(Upstream)

export class UpstreamManifestError extends Data.TaggedError("UpstreamManifestError")<{
  readonly reason: string
}> {
  get message(): string {
    return `Unable to read _packages/tsgo/upstream.json: ${this.reason}`
  }
}

const validateUniqueNames = <A extends {
  readonly profiles: ReadonlyArray<{
    readonly name: string
    readonly kind: string
    readonly binName?: string
  }>
}>(upstream: A) => {
  const names = new Set<string>()
  const binNames = new Set<string>()
  for (const profile of upstream.profiles) {
    if (names.has(profile.name)) {
      return Effect.fail(new UpstreamManifestError({ reason: `Duplicate upstream profile: ${profile.name}` }))
    }
    names.add(profile.name)
    if (profile.binName !== undefined) {
      if (binNames.has(profile.binName)) {
        return Effect.fail(new UpstreamManifestError({
          reason: `Duplicate TypeScript binary name: ${profile.binName}`
        }))
      }
      binNames.add(profile.binName)
    }
  }
  for (const [name, kind, binName] of [
    ["next", "ts", "tsc-next"],
    ["latest", "ts", "tsc"],
    ["oxlint", "oxlint", undefined]
  ] as const) {
    if (!upstream.profiles.some((profile) =>
      profile.name === name && profile.kind === kind && (binName === undefined || profile.binName === binName))) {
      return Effect.fail(new UpstreamManifestError({ reason: `Upstream profile ${name} must have kind ${kind}` }))
    }
  }
  return Effect.succeed(upstream)
}

export const decodeUpstream = (text: string) =>
  Schema.decodeUnknownEffect(UpstreamFromString)(text).pipe(
    Effect.mapError((error) => new UpstreamManifestError({ reason: error.message })),
    Effect.flatMap(validateUniqueNames)
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

export const getProfile = Effect.fnUntraced(function*(
  upstream: typeof Upstream.Type,
  name: string
) {
  const profile = upstream.profiles.find((profile) => profile.name === name)
  if (profile === undefined) {
    return yield* new UpstreamManifestError({ reason: `Unknown upstream profile: ${name}` })
  }
  return profile
})

const NpmMetadata = Schema.fromJsonString(Schema.Struct({
  version: NonEmptyString,
  gitHead: GitRevision
}))

interface TypeScriptMetadata {
  readonly npmVersion: string
  readonly gitHead: string
}

interface OxlintMetadata {
  readonly ts: TypeScriptMetadata
  readonly tsgolint: TypeScriptMetadata
  readonly oxlint: TypeScriptMetadata
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

export const updateTypeScriptProfiles = (
  upstream: typeof Upstream.Type,
  updates: Readonly<Record<"next" | "latest", TypeScriptMetadata>>
) => ({
  ...upstream,
  profiles: upstream.profiles.map((profile) =>
    profile.name === "next" || profile.name === "latest"
      ? { ...profile, ts: updates[profile.name] }
      : profile)
})

export const updateOxlintProfile = (
  upstream: typeof Upstream.Type,
  update: OxlintMetadata
) => ({
  ...upstream,
  profiles: upstream.profiles.map((profile) => profile.name === "oxlint" ? { ...profile, ...update } : profile)
})

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

const fetchJson = Effect.fnUntraced(function*(url: string) {
  const response = yield* Effect.tryPromise({
    try: () => fetch(url, { headers: { "User-Agent": "effect-tsgo-repoctl" } }),
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

const fetchOxlintMetadata = Effect.fnUntraced(function*(repositoryRoot: string) {
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
  const tsgolintRange = oxlintPackage.peerDependencies["oxlint-tsgolint"]
  const tsgolintOutput = yield* runCommandString("npm", repositoryRoot, [
    "view",
    `oxlint-tsgolint@${tsgolintRange}`,
    "version",
    "--json"
  ])
  const tsgolintVersion = yield* Schema.decodeUnknownEffect(Schema.fromJsonString(NonEmptyString))(tsgolintOutput).pipe(
    Effect.mapError((error) => new UpstreamManifestError({
      reason: `Unable to resolve oxlint-tsgolint@${tsgolintRange}: ${error.message}`
    }))
  )
  const [oxlintGitHead, tsgolintGitHead] = yield* Effect.all([
    resolveRemoteTag(repositoryRoot, "https://github.com/oxc-project/oxc.git", `apps_v${oxlintPackage.version}`),
    resolveRemoteTag(repositoryRoot, "https://github.com/oxc-project/tsgolint.git", `v${tsgolintVersion}`)
  ], { concurrency: "unbounded" })
  const typescriptGitHead = yield* readRemoteGitlink(
    "https://github.com/oxc-project/tsgolint.git",
    tsgolintGitHead,
    "typescript-go"
  )
  const packument = yield* fetchJson("https://registry.npmjs.org/typescript")
  if (typeof packument !== "object" || packument === null || !("versions" in packument) ||
    typeof packument.versions !== "object" || packument.versions === null) {
    return yield* new UpstreamManifestError({ reason: "TypeScript npm metadata does not contain versions" })
  }
  const typescriptVersion = findTypeScriptVersion(
    packument.versions as Record<string, { readonly gitHead?: unknown }>,
    typescriptGitHead
  )
  if (typescriptVersion === undefined) {
    return yield* new UpstreamManifestError({
      reason: `No TypeScript npm version has git head ${typescriptGitHead}`
    })
  }
  return {
    oxlint: { npmVersion: oxlintPackage.version, gitHead: oxlintGitHead },
    tsgolint: { npmVersion: tsgolintVersion, gitHead: tsgolintGitHead },
    ts: { npmVersion: typescriptVersion, gitHead: typescriptGitHead }
  }
})

export const updateUpstream = Effect.fnUntraced(function*(repositoryRoot: string) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const upstream = yield* readUpstream(repositoryRoot)
  const nextBefore = yield* getProfile(upstream, "next")
  const latestBefore = yield* getProfile(upstream, "latest")
  const [next, latest, oxlint, remoteSchema] = yield* Effect.all([
    fetchTypeScriptMetadata(repositoryRoot, "typescript@next"),
    fetchTypeScriptMetadata(repositoryRoot, "typescript@latest"),
    fetchOxlintMetadata(repositoryRoot),
    fetchJson("https://json.schemastore.org/tsconfig")
  ], { concurrency: "unbounded" })
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
  const updated = updateOxlintProfile(updateTypeScriptProfiles(upstream, { next, latest }), oxlint)
  const profilesChanged = JSON.stringify(updated) !== JSON.stringify(upstream)
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
  const hasChanges = profilesChanged || schemaChanged || oxlintSchemaChanged

  if (profilesChanged) {
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
    next_previous_version: nextBefore.ts.npmVersion,
    next_previous_git_head: nextBefore.ts.gitHead,
    next_version: next.npmVersion,
    next_git_head: next.gitHead,
    latest_spec: "typescript@latest",
    latest_previous_version: latestBefore.ts.npmVersion,
    latest_previous_git_head: latestBefore.ts.gitHead,
    latest_version: latest.npmVersion,
    latest_git_head: latest.gitHead
  }
  if (process.env.GITHUB_OUTPUT !== undefined) {
    yield* Effect.tryPromise(() => appendFile(
      process.env.GITHUB_OUTPUT!,
      Object.entries(outputs).map(([name, value]) => `${name}=${value}\n`).join("")
    ))
  }
  yield* Effect.log(hasChanges ? "Updated upstream metadata" : "Upstream metadata is current")
  return outputs
})
