import * as NodeServices from "@effect/platform-node/NodeServices"
import * as Effect from "effect/Effect"
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { basename, dirname, join } from "node:path"
import { afterEach, describe, expect, it } from "vitest"
import upstreamJson from "../upstream.json" with { type: "json" }
import { renderOxlintEnv, resolveOxlintEnv } from "../src/cli/oxlintEnv.js"
import { experimentalOxlintTarget, type ReplacementResolver } from "../src/patcher/index.js"

const temporaryDirectories: Array<string> = []

const makeTemporaryDirectory = async () => {
  const directory = await mkdtemp(join(tmpdir(), "effect-tsgo-oxlint-env-"))
  temporaryDirectories.push(directory)
  return directory
}

const writePackage = async (directory: string, packageName: string, packageJson: Record<string, unknown>) => {
  const packageDirectory = join(directory, "node_modules", ...packageName.split("/"))
  await mkdir(packageDirectory, { recursive: true })
  await writeFile(join(packageDirectory, "package.json"), JSON.stringify(packageJson))
  return packageDirectory
}

const writeBinary = async (filePath: string, contents: string) => {
  await mkdir(dirname(filePath), { recursive: true })
  await writeFile(filePath, contents)
}

const writeOxlintProject = async (versions: { readonly oxlint: string; readonly tsgolint?: string }) => {
  const directory = await makeTemporaryDirectory()
  const platform = experimentalOxlintTarget(process.platform, process.arch, true)
  const oxlintDirectory = await writePackage(directory, "oxlint", { version: versions.oxlint })
  const bindingDirectory = await writePackage(directory, platform.oxlintPackage, {
    version: versions.oxlint,
    main: `oxlint.${platform.codeTarget}.node`
  })
  await writeBinary(join(bindingDirectory, `oxlint.${platform.codeTarget}.node`), "oxlint")
  await writeBinary(join(oxlintDirectory, "dist", "index.d.ts"), "declarations")
  if (versions.tsgolint !== undefined) {
    await writePackage(directory, "oxlint-tsgolint", { version: versions.tsgolint })
    const tsgolintDirectory = await writePackage(directory, platform.tsgolintPackage, { version: versions.tsgolint })
    await writeBinary(join(tsgolintDirectory, platform.tsgolintExecutable), "tsgolint")
  }
  return directory
}

const run = <A, E>(effect: Effect.Effect<A, E, NodeServices.NodeServices>) =>
  Effect.runPromise(effect.pipe(Effect.provide(NodeServices.layer)))

afterEach(async () => {
  await Promise.all(temporaryDirectories.splice(0).map((directory) => rm(directory, { recursive: true, force: true })))
})

describe("oxlint-env", () => {
  it("points Oxlint at the packaged artifacts for the installed versions", async () => {
    const oxlint = upstreamJson.tags.oxlint.latest
    const tsgolint = upstreamJson.tags["oxlint-tsgolint"].latest
    const directory = await writeOxlintProject({ oxlint, tsgolint })
    const packaged = join(directory, "effect-tsgo-platform", "artifacts")
    const resolver: ReplacementResolver = (target) =>
      Effect.succeed({
        path: join(packaged, target.component, target.packageVersion, basename(target.binaryPath)),
        fileHash: target.fileHash
      })
    const platform = experimentalOxlintTarget(process.platform, process.arch, true)

    await expect(run(resolveOxlintEnv(directory, resolver))).resolves.toEqual([
      {
        name: "NAPI_RS_NATIVE_LIBRARY_PATH",
        value: join(packaged, "oxlint", oxlint, `oxlint.${platform.codeTarget}.node`)
      },
      {
        name: "OXLINT_TSGOLINT_PATH",
        value: join(packaged, "oxlint-tsgolint", tsgolint, platform.tsgolintExecutable)
      }
    ])
  })

  it("rejects installed versions without a packaged build", async () => {
    const directory = await writeOxlintProject({ oxlint: "1.0.0", tsgolint: upstreamJson.tags["oxlint-tsgolint"].latest })

    await expect(run(resolveOxlintEnv(directory))).rejects.toMatchObject({
      _tag: "UnsupportedTargetPackageVersionError",
      target: { component: "oxlint", packageVersion: "1.0.0" },
      supportedVersions: Object.keys(upstreamJson.components.oxlint).sort()
    })
  })

  it("requires oxlint-tsgolint to be installed", async () => {
    const directory = await writeOxlintProject({ oxlint: upstreamJson.tags.oxlint.latest })

    await expect(run(resolveOxlintEnv(directory))).rejects.toMatchObject({
      _tag: "DiscoveryError",
      message: "Unable to discover an installed oxlint-tsgolint binary."
    })
  })

  it("quotes paths for POSIX shells and PowerShell", () => {
    const variables = [
      { name: "NAPI_RS_NATIVE_LIBRARY_PATH", value: "/Users/O'Brien/my project/oxlint.node" },
      { name: "OXLINT_TSGOLINT_PATH", value: "C:\\Users\\O\u2019Brien\\$HOME\\tsgolint.exe" }
    ]

    expect(renderOxlintEnv(variables, "sh")).toBe([
      "export NAPI_RS_NATIVE_LIBRARY_PATH='/Users/O'\\''Brien/my project/oxlint.node'",
      "export OXLINT_TSGOLINT_PATH='C:\\Users\\O\u2019Brien\\$HOME\\tsgolint.exe'"
    ].join("\n"))
    expect(renderOxlintEnv(variables, "powershell")).toBe([
      "$env:NAPI_RS_NATIVE_LIBRARY_PATH = '/Users/O''Brien/my project/oxlint.node'",
      "$env:OXLINT_TSGOLINT_PATH = 'C:\\Users\\O\u2019\u2019Brien\\$HOME\\tsgolint.exe'"
    ].join("\n"))
  })
})
