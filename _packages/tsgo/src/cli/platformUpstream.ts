import * as Effect from "effect/Effect"
import * as Schema from "effect/Schema"

const TypeScriptIdentity = Schema.Struct({
  version: Schema.String,
  gitHead: Schema.String
})

const TypeScriptProfile = Schema.Struct({
  kind: Schema.Literal("ts"),
  name: Schema.String,
  ts: TypeScriptIdentity,
  binName: Schema.Literals(["tsc", "tsc-next"])
})

const OtherProfile = Schema.Struct({
  kind: Schema.Literal("oxlint"),
  name: Schema.String
})

const PlatformUpstream = Schema.Struct({
  schemaVersion: Schema.Literal(2),
  profiles: Schema.Array(Schema.Union([TypeScriptProfile, OtherProfile]))
})

const PlatformUpstreamFromString = Schema.fromJsonString(PlatformUpstream)

export interface PackagedTypeScriptProfile {
  readonly binaryName: string
  readonly tsVersion: string
  readonly tsGitHead: string
}

export const decodePackagedTypeScriptProfiles = (text: string) =>
  Schema.decodeUnknownEffect(PlatformUpstreamFromString)(text).pipe(
    Effect.map((upstream) => upstream.profiles
      .filter((profile): profile is typeof TypeScriptProfile.Type => profile.kind === "ts")
      .map((profile) => ({
        binaryName: profile.binName,
        tsVersion: profile.ts.version,
        tsGitHead: profile.ts.gitHead
      }))
      .sort((left, right) => left.binaryName === "tsc" ? -1 : right.binaryName === "tsc" ? 1 : 0))
  )
