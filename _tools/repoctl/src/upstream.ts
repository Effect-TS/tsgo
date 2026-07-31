import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"
import * as Path from "effect/Path"
import * as Schema from "effect/Schema"
import { appendFile } from "node:fs/promises"
import { runCommandString } from "./process.ts"

export type ProfileName = "next" | "latest" | "oxlint"

const NonEmptyString = Schema.String.check(Schema.isNonEmpty())
const GitRevision = Schema.String.check(Schema.isPattern(/^[0-9a-f]{40}$/))
const UpstreamIdentity = Schema.Struct({
  version: NonEmptyString,
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

type TypeScriptMetadata = typeof NpmMetadata.Type

const fetchTypeScriptMetadata = Effect.fnUntraced(function*(repositoryRoot: string, spec: string) {
  const output = yield* runCommandString("npm", repositoryRoot, ["view", spec, "version", "gitHead", "--json"])
  return yield* Schema.decodeUnknownEffect(NpmMetadata)(output).pipe(
    Effect.mapError((error) => new UpstreamManifestError({
      reason: `Unable to resolve ${spec}: ${error.message}`
    }))
  )
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

export const updateUpstream = Effect.fnUntraced(function*(repositoryRoot: string) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const upstream = yield* readUpstream(repositoryRoot)
  const nextBefore = yield* getProfile(upstream, "next")
  const latestBefore = yield* getProfile(upstream, "latest")
  const [next, latest] = yield* Effect.all([
    fetchTypeScriptMetadata(repositoryRoot, "typescript@next"),
    fetchTypeScriptMetadata(repositoryRoot, "typescript@latest")
  ], { concurrency: "unbounded" })
  const updated = updateTypeScriptProfiles(upstream, { next, latest })
  const hasChanges = nextBefore.ts.version !== next.version ||
    nextBefore.ts.gitHead !== next.gitHead ||
    latestBefore.ts.version !== latest.version ||
    latestBefore.ts.gitHead !== latest.gitHead

  if (hasChanges) {
    yield* fs.writeFileString(
      path.join(repositoryRoot, "_packages", "tsgo", "upstream.json"),
      `${JSON.stringify(updated, null, 2)}\n`
    ).pipe(Effect.mapError((error) => new UpstreamManifestError({ reason: error.message })))
  }

  const outputs = {
    has_changes: String(hasChanges),
    next_spec: "typescript@next",
    next_previous_version: nextBefore.ts.version,
    next_previous_git_head: nextBefore.ts.gitHead,
    next_version: next.version,
    next_git_head: next.gitHead,
    latest_spec: "typescript@latest",
    latest_previous_version: latestBefore.ts.version,
    latest_previous_git_head: latestBefore.ts.gitHead,
    latest_version: latest.version,
    latest_git_head: latest.gitHead
  }
  if (process.env.GITHUB_OUTPUT !== undefined) {
    yield* Effect.tryPromise(() => appendFile(
      process.env.GITHUB_OUTPUT!,
      Object.entries(outputs).map(([name, value]) => `${name}=${value}\n`).join("")
    ))
  }
  yield* Effect.log(hasChanges ? "Updated upstream TypeScript profiles" : "Upstream TypeScript profiles are current")
  return outputs
})
