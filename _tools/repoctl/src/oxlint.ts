import * as Console from "effect/Console"
import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"
import * as Path from "effect/Path"
import { runCommand, runCommandString } from "./process.ts"
import { generateSubmoduleArtifacts } from "./submodules.ts"
import { getProfile, readUpstream } from "./upstream.ts"

export class OxlintGenerationError extends Data.TaggedError("OxlintGenerationError")<{
  readonly reason: string
}> {
  get message(): string {
    return `Unable to generate Oxlint integration: ${this.reason}`
  }
}

const parseGitlink = (entry: string, expectedPath: string) => {
  const [mode, , revision, path] = entry.trim().split(/\s+/)
  if (mode !== "160000" || path !== expectedPath || !/^[0-9a-f]{40}$/.test(revision ?? "")) {
    throw new OxlintGenerationError({ reason: `Invalid ${expectedPath} gitlink: ${entry.trim()}` })
  }
  return revision!
}

const readGitlink = Effect.fnUntraced(function*(checkout: string, revision: string, expectedPath: string) {
  const entry = yield* runCommandString("git", checkout, ["ls-tree", revision, expectedPath])
  return yield* Effect.try({
    try: () => parseGitlink(entry, expectedPath),
    catch: (error) => error instanceof OxlintGenerationError
      ? error
      : new OxlintGenerationError({ reason: String(error) })
  })
})

const parseJson = <A>(text: string, source: string) => Effect.try({
  try: () => JSON.parse(text) as A,
  catch: (error) => new OxlintGenerationError({ reason: `Unable to parse ${source}: ${String(error)}` })
})

const applyPatchDirectory = Effect.fnUntraced(function*(checkout: string, directory: string, label: string) {
  const fs = yield* FileSystem.FileSystem
  if (!(yield* fs.exists(directory))) return
  const patches = (yield* fs.readDirectory(directory))
    .filter((file) => file.endsWith(".patch"))
    .sort((left, right) => Buffer.compare(Buffer.from(left), Buffer.from(right)))
  for (const patchName of patches) {
    const patch = `${directory}/${patchName}`
    yield* Console.log(`Applying ${label} patch: ${patch}`)
    yield* runCommand("git", checkout, ["apply", "--check", patch], true)
    yield* runCommand("git", checkout, ["apply", patch])
  }
})

const synchronizeCollections = Effect.fnUntraced(function*(typescriptGo: string, tsgolint: string) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const source = path.join(typescriptGo, "internal", "collections")
  const target = path.join(tsgolint, "internal", "collections")
  yield* fs.remove(target, { recursive: true, force: true })
  yield* fs.makeDirectory(target, { recursive: true })
  for (const name of yield* fs.readDirectory(source)) {
    const file = path.join(source, name)
    if (!name.endsWith("_test.go") && (yield* fs.stat(file)).type === "File") {
      yield* fs.copyFile(file, path.join(target, name))
    }
  }
})

interface GoModEdit {
  readonly Require?: ReadonlyArray<{ readonly Path: string; readonly Version: string }>
  readonly Replace?: ReadonlyArray<{ readonly Old: { readonly Path: string } }>
}

const configureWorkspace = Effect.fnUntraced(function*(repositoryRoot: string, tsgolint: string) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  yield* runCommand("go", repositoryRoot, ["work", "edit", "-use=./tsgolint"])
  const module = yield* parseJson<GoModEdit>(
    yield* runCommandString(
      "go",
      repositoryRoot,
      ["mod", "edit", "-json", path.join(tsgolint, "go.mod")],
      { GOWORK: "off" }
    ),
    path.join(tsgolint, "go.mod")
  )
  const versions = new Map((module.Require ?? []).map(({ Path, Version }) => [Path, Version]))
  const prefix = "github.com/microsoft/typescript-go/shim/"
  const replacements = (module.Replace ?? [])
    .filter(({ Old }) => Old.Path.startsWith(prefix))
    .map(({ Old }) => ({
      module: `${Old.Path}@${versions.get(Old.Path) ?? "v0.0.0"}`,
      shim: `./shim/${Old.Path.slice(prefix.length)}`,
      shimPath: path.join(repositoryRoot, "shim", Old.Path.slice(prefix.length))
    }))
    .sort((left, right) => left.module < right.module ? -1 : left.module > right.module ? 1 : 0)

  for (const replacement of replacements) {
    if (!(yield* fs.exists(replacement.shimPath))) {
      return yield* new OxlintGenerationError({ reason: `Shared shim path does not exist: ${replacement.shim}` })
    }
    yield* runCommand("go", repositoryRoot, [
      "work",
      "edit",
      `-replace=${replacement.module}=${replacement.shim}`
    ])
  }

  const resolvedTypeScriptGo = (yield* runCommandString("go", repositoryRoot, [
    "list",
    "-m",
    "-f",
    "{{.Dir}}",
    "github.com/microsoft/typescript-go"
  ], { GOWORK: path.join(repositoryRoot, "go.work") })).trim()
  const resolvedChecker = (yield* runCommandString("go", repositoryRoot, [
    "list",
    "-m",
    "-f",
    "{{.Dir}}",
    "github.com/microsoft/typescript-go/shim/checker"
  ], { GOWORK: path.join(repositoryRoot, "go.work") })).trim()
  if ((yield* fs.realPath(resolvedTypeScriptGo)) !== (yield* fs.realPath(path.join(repositoryRoot, "typescript-go")))) {
    return yield* new OxlintGenerationError({ reason: "Go workspace does not resolve the shared TypeScript-Go checkout" })
  }
  if ((yield* fs.realPath(resolvedChecker)) !== (yield* fs.realPath(path.join(repositoryRoot, "shim", "checker")))) {
    return yield* new OxlintGenerationError({ reason: "Go workspace does not resolve the shared checker shim" })
  }
})

export const prepareOxlintProfile = Effect.fnUntraced(function*(repositoryRoot: string) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const upstream = yield* readUpstream(repositoryRoot)
  const profile = yield* getProfile(upstream, "oxlint")
  if (profile.kind !== "oxlint") {
    return yield* new OxlintGenerationError({ reason: "The oxlint profile has an unexpected kind" })
  }

  const typescriptGo = path.join(repositoryRoot, "typescript-go")
  const tsgolint = path.join(repositoryRoot, "tsgolint")
  const oxlint = path.join(repositoryRoot, "oxlint")
  const tsgolintTypeScriptGo = yield* readGitlink(tsgolint, profile.tsgolint.gitHead, "typescript-go")
  if (tsgolintTypeScriptGo !== profile.ts.gitHead) {
    return yield* new OxlintGenerationError({
      reason: `tsgolint TypeScript-Go revision ${tsgolintTypeScriptGo} does not match profile ${profile.ts.gitHead}`
    })
  }

  yield* runCommand("git", tsgolint, ["fetch", "--quiet", "--depth", "50", "--tags", "origin", profile.tsgolint.gitHead])
  const oxlintPackage = yield* parseJson<{ readonly version?: string }>(
    yield* fs.readFileString(path.join(oxlint, "apps", "oxlint", "package.json")),
    path.join(oxlint, "apps", "oxlint", "package.json")
  )
  if (oxlintPackage.version !== profile.oxlint.npmVersion) {
    return yield* new OxlintGenerationError({
      reason: `Oxlint version ${oxlintPackage.version} does not match profile ${profile.oxlint.npmVersion}`
    })
  }

  yield* runCommand("git", repositoryRoot, ["config", "-f", ".gitmodules", "submodule.oxlint.ignore", "dirty"])
  yield* runCommand("git", repositoryRoot, ["config", "-f", ".gitmodules", "submodule.tsgolint.ignore", "dirty"])
  yield* runCommand("git", typescriptGo, ["submodule", "sync", "--recursive"])
  yield* runCommand("git", typescriptGo, [
    "submodule",
    "update",
    "--init",
    "--force",
    "--depth",
    "1",
    "_submodules/TypeScript"
  ])
  const typescriptRevision = yield* readGitlink(typescriptGo, profile.ts.gitHead, "_submodules/TypeScript")
  const actualTypeScript = (yield* runCommandString("git", path.join(typescriptGo, "_submodules", "TypeScript"), [
    "rev-parse",
    "HEAD"
  ])).trim()
  if (actualTypeScript !== typescriptRevision) {
    return yield* new OxlintGenerationError({
      reason: `TypeScript checkout ${actualTypeScript} does not match gitlink ${typescriptRevision}`
    })
  }

  yield* applyPatchDirectory(typescriptGo, path.join(tsgolint, "patches"), "tsgolint TypeScript-Go")
  yield* applyPatchDirectory(
    typescriptGo,
    path.join(repositoryRoot, "_patches", "typescript-go"),
    "Effect TypeScript-Go"
  )
  yield* applyPatchDirectory(tsgolint, path.join(repositoryRoot, "_patches", "tsgolint"), "Effect tsgolint")
  yield* applyPatchDirectory(oxlint, path.join(repositoryRoot, "_patches", "oxlint"), "Effect Oxlint")

  yield* runCommand("git", repositoryRoot, ["add", ".gitmodules", "oxlint", "tsgolint", "typescript-go"])
  yield* Console.log("Oxlint profile prepared")
})

export const generateTsgolintWorkspace = Effect.fnUntraced(function*(repositoryRoot: string) {
  const path = yield* Path.Path
  const typescriptGo = path.join(repositoryRoot, "typescript-go")
  const tsgolint = path.join(repositoryRoot, "tsgolint")
  yield* synchronizeCollections(typescriptGo, tsgolint)
  yield* generateSubmoduleArtifacts(repositoryRoot, [path.join(tsgolint, "shim")])
  yield* configureWorkspace(repositoryRoot, tsgolint)
})
