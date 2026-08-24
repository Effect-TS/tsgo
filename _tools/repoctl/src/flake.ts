import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"
import * as Path from "effect/Path"
import { runCommand, runCommandCapture } from "./process.ts"
import { readUpstream } from "./upstream.ts"

const vendorHashPattern = /vendorHash = (?:("[^"]+")|lib\.fakeHash);/

export class FlakeUpdateError extends Data.TaggedError("FlakeUpdateError")<{
  readonly reason: string
}> {
  get message(): string {
    return `Unable to update Nix flake: ${this.reason}`
  }
}

const replaceRequired = (text: string, pattern: RegExp, replacement: string, name: string) => {
  if (!pattern.test(text)) {
    throw new FlakeUpdateError({ reason: `Unable to find ${name} in flake.nix` })
  }
  return text.replace(pattern, replacement)
}

export const updateFlakeInputs = (flake: string, typescriptRevision: string) =>
  replaceRequired(
    flake,
    /github:microsoft\/TypeScript\/[0-9a-f]{40}/,
    `github:microsoft/TypeScript/${typescriptRevision}`,
    "typescript-src"
  )

export const updateVendorHash = (flake: string, hash: string) =>
  replaceRequired(flake, vendorHashPattern, `vendorHash = ${JSON.stringify(hash)};`, "vendorHash")

export const invalidateVendorHash = (flake: string) =>
  replaceRequired(flake, vendorHashPattern, "vendorHash = lib.fakeHash;", "vendorHash")

const replacementVendorHash = (output: string) => {
  const messageHash = output.match(/To correct the hash mismatch[^\n]*use "([^"]+)"/)?.[1]
  const reportedHash = output.match(/got:\s+(sha256-[^\s]+)/)?.[1]
  return messageHash ?? reportedHash
}

const buildFlake = (repositoryRoot: string) => {
  const extraArguments = process.env.NIX_BUILD_ARGS?.trim().split(/\s+/).filter(Boolean) ?? []
  return runCommandCapture("nix", repositoryRoot, [
    "build",
    ".#effect-tsgo",
    "--no-write-lock-file",
    "-L",
    ...extraArguments
  ])
}

export const updateFlake = Effect.fnUntraced(function*(repositoryRoot: string) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const upstream = yield* readUpstream(repositoryRoot)
  const next = upstream.components.typescript[upstream.tags.typescript.next]!
  if (next.provider !== "typescript") {
    return yield* new FlakeUpdateError({
      reason: `The Nix flake requires the TypeScript provider, but ${upstream.tags.typescript.next} uses ${next.provider}`
    })
  }

  const flakePath = path.join(repositoryRoot, "flake.nix")
  const flake = yield* fs.readFileString(flakePath).pipe(
    Effect.mapError((error) => new FlakeUpdateError({ reason: error.message }))
  )
  const updatedInputs = yield* Effect.try({
    try: () => updateFlakeInputs(flake, next.gitHead),
    catch: (error) => error instanceof FlakeUpdateError
      ? error
      : new FlakeUpdateError({ reason: String(error) })
  })
  const flakeWithFakeHash = yield* Effect.try({
    try: () => invalidateVendorHash(updatedInputs),
    catch: (error) => error instanceof FlakeUpdateError
      ? error
      : new FlakeUpdateError({ reason: String(error) })
  })
  yield* fs.writeFileString(flakePath, flakeWithFakeHash).pipe(
    Effect.mapError((error) => new FlakeUpdateError({ reason: error.message }))
  )
  yield* runCommand("nix", repositoryRoot, ["flake", "lock"])

  const initialBuild = yield* buildFlake(repositoryRoot)
  if (initialBuild.exitCode === 0) {
    yield* Effect.log("Nix flake inputs and vendor hash are current")
    return
  }

  const hash = replacementVendorHash(initialBuild.output)
  if (hash === undefined) {
    return yield* new FlakeUpdateError({
      reason: `Cannot extract a replacement vendor hash from nix build output:\n${initialBuild.output}`
    })
  }
  const flakeWithHash = yield* Effect.try({
    try: () => updateVendorHash(updatedInputs, hash),
    catch: (error) => error instanceof FlakeUpdateError
      ? error
      : new FlakeUpdateError({ reason: String(error) })
  })
  yield* fs.writeFileString(flakePath, flakeWithHash).pipe(
    Effect.mapError((error) => new FlakeUpdateError({ reason: error.message }))
  )

  const finalBuild = yield* buildFlake(repositoryRoot)
  if (finalBuild.exitCode !== 0) {
    return yield* new FlakeUpdateError({
      reason: `Nix build still fails after updating the vendor hash:\n${finalBuild.output}`
    })
  }
  yield* Effect.log(`Updated Nix vendor hash to ${hash}`)
})
