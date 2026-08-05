import * as NodeServices from "@effect/platform-node/NodeServices"
import * as Effect from "effect/Effect"
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { afterEach, describe, expect, it } from "vitest"
import { discoverBinaries, experimentalOxlintTarget } from "../src/patcher/index.js"

const temporaryDirectories: Array<string> = []

const makeTemporaryDirectory = async () => {
  const directory = await mkdtemp(join(tmpdir(), "effect-tsgo-oxlint-"))
  temporaryDirectories.push(directory)
  return directory
}

const writePackage = async (directory: string, packageName: string, packageJson: Record<string, unknown>) => {
  const packageDirectory = join(directory, "node_modules", ...packageName.split("/"))
  await mkdir(packageDirectory, { recursive: true })
  await writeFile(join(packageDirectory, "package.json"), JSON.stringify(packageJson))
  return packageDirectory
}

afterEach(async () => {
  await Promise.all(temporaryDirectories.splice(0).map((directory) => rm(directory, { recursive: true, force: true })))
})

describe("experimental Oxlint discovery", () => {
  it("maps supported platforms to installed package names", () => {
    expect(experimentalOxlintTarget("linux", "x64", true)).toEqual({
      codeTarget: "linux-x64-gnu",
      oxlintPackage: "@oxlint/binding-linux-x64-gnu",
      tsgolintPackage: "@oxlint-tsgolint/linux-x64",
      tsgolintExecutable: "tsgolint"
    })
    expect(experimentalOxlintTarget("win32", "arm64", false)).toEqual({
      codeTarget: "win32-arm64",
      oxlintPackage: "@oxlint/binding-win32-arm64-msvc",
      tsgolintPackage: "@oxlint-tsgolint/win32-arm64",
      tsgolintExecutable: "tsgolint.exe"
    })
    expect(() => experimentalOxlintTarget("linux", "x64", false)).toThrow(/musl/)
    expect(() => experimentalOxlintTarget("linux", "arm", true)).toThrow(/Unsupported/)
  })

  it("returns normalized binaries without resolving Effect artifacts", async () => {
    const directory = await makeTemporaryDirectory()
    const platform = experimentalOxlintTarget(process.platform, process.arch, true)
    await writePackage(directory, "oxlint", { version: "1.0.0" })
    await writePackage(directory, "oxlint-tsgolint", { version: "2.0.0" })
    const bindingDirectory = await writePackage(directory, platform.oxlintPackage, {
      version: "1.0.1",
      main: "oxlint.node"
    })
    const tsgolintDirectory = await writePackage(directory, platform.tsgolintPackage, { version: "2.0.1" })

    const discovered = await Effect.runPromise(
      discoverBinaries(directory).pipe(Effect.provide(NodeServices.layer))
    )
    expect(discovered).toEqual([
      {
        component: "oxlint",
        packageName: platform.oxlintPackage,
        packageVersion: "1.0.1",
        binaryPath: join(bindingDirectory, "oxlint.node")
      },
      {
        component: "oxlint-tsgolint",
        packageName: platform.tsgolintPackage,
        packageVersion: "2.0.1",
        binaryPath: join(tsgolintDirectory, platform.tsgolintExecutable)
      }
    ])
  })

  it("uses the package containing the TypeScript binary as its identity", async () => {
    const directory = await makeTemporaryDirectory()
    await writePackage(directory, "typescript", { version: "7.0.0" })
    const platformPackage = `@typescript/typescript-${process.platform}-${process.arch}`
    const platformDirectory = await writePackage(directory, platformPackage, { version: "7.0.1" })

    const discovered = await Effect.runPromise(
      discoverBinaries(directory).pipe(Effect.provide(NodeServices.layer))
    )
    expect(discovered).toContainEqual({
      component: "typescript",
      packageName: platformPackage,
      packageVersion: "7.0.1",
      binaryPath: join(platformDirectory, "lib", process.platform === "win32" ? "tsc.exe" : "tsc")
    })
  })

  it("discovers Oxlint binaries installed as dependencies of vite-plus", async () => {
    const directory = await makeTemporaryDirectory()
    const platform = experimentalOxlintTarget(process.platform, process.arch, true)
    const vitePlusDirectory = await writePackage(directory, "vite-plus", { version: "1.0.0" })
    await writePackage(vitePlusDirectory, "oxlint", { version: "1.0.0" })
    await writePackage(vitePlusDirectory, "oxlint-tsgolint", { version: "2.0.0" })
    const bindingDirectory = await writePackage(vitePlusDirectory, platform.oxlintPackage, {
      version: "1.0.1",
      main: "oxlint.node"
    })
    const tsgolintDirectory = await writePackage(
      vitePlusDirectory,
      platform.tsgolintPackage,
      { version: "2.0.1" }
    )

    const discovered = await Effect.runPromise(
      discoverBinaries(directory).pipe(Effect.provide(NodeServices.layer))
    )
    expect(discovered).toEqual([
      {
        component: "oxlint",
        packageName: platform.oxlintPackage,
        packageVersion: "1.0.1",
        binaryPath: join(bindingDirectory, "oxlint.node")
      },
      {
        component: "oxlint-tsgolint",
        packageName: platform.tsgolintPackage,
        packageVersion: "2.0.1",
        binaryPath: join(tsgolintDirectory, platform.tsgolintExecutable)
      }
    ])
  })
})
