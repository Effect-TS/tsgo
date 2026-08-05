import * as Effect from "effect/Effect"
import * as Schema from "effect/Schema"

const Component = Schema.Struct({
  gitHead: Schema.String
})

const PlatformUpstream = Schema.Struct({
  schemaVersion: Schema.Literal(4),
  tags: Schema.Struct({
    typescript: Schema.Struct({
      latest: Schema.String,
      next: Schema.String
    })
  }),
  components: Schema.Struct({
    typescript: Schema.Record(Schema.String, Component)
  })
})

const PlatformUpstreamFromString = Schema.fromJsonString(PlatformUpstream)

export interface PackagedTypeScriptProfile {
  readonly binaryName: string
  readonly artifactPath: string
  readonly tsVersion: string
  readonly tsGitHead: string
}

export const decodePackagedTypeScriptProfiles = (text: string) =>
  Schema.decodeUnknownEffect(PlatformUpstreamFromString)(text).pipe(
    Effect.map((upstream) => Object.entries(upstream.components.typescript)
      .map(([version, component]) => ({
        binaryName: version === upstream.tags.typescript.latest
          ? "tsc"
          : version === upstream.tags.typescript.next ? "tsc-next" : `tsc-${version}`,
        artifactPath: `artifacts/typescript/${version}/tsc`,
        tsVersion: version,
        tsGitHead: component.gitHead
      }))
      .sort((left, right) => left.binaryName === "tsc" ? -1 : right.binaryName === "tsc" ? 1 : 0))
  )
