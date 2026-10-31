import * as Console from "effect/Console"
import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"
import * as Path from "effect/Path"
import { ensureEffectFixtures } from "./fixtures.ts"
import { portCheck } from "./port.ts"
import { runCommand } from "./process.ts"
import {
  getTypeScriptSource,
  parseTypeScriptOverrideFolderName,
  readUpstream,
  typeScriptPatchesRoot,
  typeScriptShimProvidersRoot,
  type Upstream
} from "./upstream.ts"

export class StaleOverrideFolderError extends Data.TaggedError("StaleOverrideFolderError")<{
  readonly folder: string
}> {
  get message(): string {
    return `Stale override folder ${this.folder}: its name targets no TypeScript component of _packages/tsgo/upstream.json ` +
      "(provider and gitHead), so nothing uses it. Delete it, or rename it to the gitHead of the TypeScript it was " +
      "made for (see _patches/typescript/README.md)."
  }
}

export class PatchParityError extends Data.TaggedError("PatchParityError")<{
  readonly canonical: string
  readonly mirror: string
  readonly missing: ReadonlyArray<string>
  readonly extra: ReadonlyArray<string>
}> {
  get message(): string {
    return [
      `${this.mirror} must mirror ${this.canonical} patch for patch (same file names), backported to the TypeScript ` +
      `it targets: every change to ${this.canonical} is made to ${this.mirror} too (see ${this.mirror}/README.md).`,
      ...(this.missing.length === 0 ? [] : [`Missing from ${this.mirror}: ${this.missing.join(", ")}`]),
      ...(this.extra.length === 0 ? [] : [`Not in ${this.canonical}: ${this.extra.join(", ")}`])
    ].join("\n")
  }
}

const effectV3FixturesRoot = "testdata/tests/effect-v3"

export class MissingEffectV3MarkerError extends Data.TaggedError("MissingEffectV3MarkerError")<{
  readonly fixtures: ReadonlyArray<string>
}> {
  get message(): string {
    return [
      `${this.fixtures.length} fixture(s) under ${effectV3FixturesRoot} lack a \`// @effect-v3\` line. The language ` +
      "service tests (quick fixes, refactors, completions) pick the Effect version from that marker, not the folder, " +
      "so without it they run against Effect v4. Add `// @effect-v3` as the first line:",
      ...this.fixtures.map((fixture) => `  ${fixture}`)
    ].join("\n")
  }
}

// Mirrors the marker test in etstesthooks/init.go: a `// @effect-v3` line anywhere in the fixture.
const hasEffectV3Marker = (content: string) => content.startsWith("// @effect-v3") || content.includes("\n// @effect-v3")

// Every fixture of the v3 suite carries the `// @effect-v3` marker.
export const checkEffectV3FixtureMarkers = Effect.fnUntraced(function*(repositoryRoot: string) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const root = path.join(repositoryRoot, effectV3FixturesRoot)
  const fixtures: Array<string> = []
  for (const name of (yield* fs.readDirectory(root)).sort()) {
    if (name.endsWith(".ts") && !hasEffectV3Marker(yield* fs.readFileString(path.join(root, name)))) {
      fixtures.push(`${effectV3FixturesRoot}/${name}`)
    }
  }
  if (fixtures.length > 0) {
    return yield* new MissingEffectV3MarkerError({ fixtures })
  }
})

// The `<provider>-<gitHead>` override folders (upstream.ts, getTypeScriptComponentSource). Node only: names, no
// checkout needed.
// - Each one must name the gitHead of a TypeScript component with that provider, or it is stale.
// - A patch override folder holds the provider's patches backported to an older TypeScript, so it must contain the
//   same patch files as the provider's folder. Shim override folders are not compared: they are self-contained and
//   legitimately hold other packages (a change to the provider's shim folder is mirrored by hand where it applies).
export const checkOverrideFolders = Effect.fnUntraced(function*(
  repositoryRoot: string,
  typescript: typeof Upstream.Type.components.typescript
) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const components = Object.values(typescript)
  const patchNames = (directory: string) =>
    fs.readDirectory(path.join(repositoryRoot, directory)).pipe(
      Effect.map((files) => files.filter((file) => file.endsWith(".patch")).sort())
    )
  for (const root of [typeScriptPatchesRoot, typeScriptShimProvidersRoot]) {
    if (!(yield* fs.exists(path.join(repositoryRoot, root)))) {
      continue
    }
    for (const name of (yield* fs.readDirectory(path.join(repositoryRoot, root))).sort()) {
      const override = parseTypeScriptOverrideFolderName(name)
      if (override === undefined) {
        continue
      }
      const folder = `${root}/${name}`
      if (!components.some((component) =>
        component.provider === override.provider && component.gitHead === override.gitHead
      )) {
        return yield* new StaleOverrideFolderError({ folder })
      }
      if (root !== typeScriptPatchesRoot) {
        continue
      }
      const canonical = getTypeScriptSource(override.provider).patchDir
      const expected = (yield* fs.exists(path.join(repositoryRoot, canonical))) ? yield* patchNames(canonical) : []
      const actual = yield* patchNames(folder)
      const missing = expected.filter((patch) => !actual.includes(patch))
      const extra = actual.filter((patch) => !expected.includes(patch))
      if (missing.length > 0 || extra.length > 0) {
        return yield* new PatchParityError({ canonical, mirror: folder, missing, extra })
      }
    }
  }
})

export const runChecks = Effect.fnUntraced(function*(repositoryRoot: string) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  yield* Console.log("Checking TypeScript override folders")
  yield* checkOverrideFolders(repositoryRoot, (yield* readUpstream(repositoryRoot)).components.typescript)
  yield* Console.log("Checking Effect v3 fixture markers")
  yield* checkEffectV3FixtureMarkers(repositoryRoot)
  yield* ensureEffectFixtures(repositoryRoot)
  yield* Console.log("Checking Go packages")
  yield* runCommand("go", repositoryRoot, ["build", "./..."], false, { CGO_ENABLED: "0" })
  yield* Console.log("Checking shim generator")
  const generatorRoot = path.join(repositoryRoot, "_tools", "gen_shims")
  yield* runCommand("go", generatorRoot, ["build", "./..."], false, {
    CGO_ENABLED: "0",
    GOWORK: "off"
  }).pipe(Effect.ensuring(Effect.all([
    fs.remove(path.join(generatorRoot, "gen_shims"), { force: true }),
    fs.remove(path.join(generatorRoot, "gen_shims.exe"), { force: true })
  ]).pipe(Effect.ignore)))
  yield* Console.log("Checking custom lint plugin")
  yield* runCommand("go", path.join(repositoryRoot, "_tools", "customlint"), ["build", "./..."], false, {
    CGO_ENABLED: "0",
    GOWORK: "off"
  })
  // `repoctl tsrs port-check`: Node and Go only, no tsrs checkout or cargo.
  yield* Console.log("Checking Rust port bookkeeping")
  yield* portCheck(repositoryRoot)
  yield* Console.log("Checking CLI package")
  yield* runCommand("pnpm", path.join(repositoryRoot, "_packages", "tsgo"), [
    "exec",
    "tsc",
    "-b",
    "tsconfig.json"
  ])
})
