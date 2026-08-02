import * as NodeServices from "@effect/platform-node/NodeServices"
import * as Effect from "effect/Effect"
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { afterEach, describe, expect, it } from "vitest"
import {
  experimentalOxlintTarget,
  patchResolvedTargets,
  unpatchResolvedTargets
} from "../src/cli/experimentalOxlint.js"

const temporaryDirectories: Array<string> = []

const makeTemporaryDirectory = async () => {
  const directory = await mkdtemp(join(tmpdir(), "effect-tsgo-oxlint-"))
  temporaryDirectories.push(directory)
  return directory
}

afterEach(async () => {
  await Promise.all(temporaryDirectories.splice(0).map((directory) => rm(directory, { recursive: true, force: true })))
})

describe("experimental Oxlint integration", () => {
  it("maps supported platforms to installed package names", () => {
    expect(experimentalOxlintTarget("linux", "x64", true)).toEqual({
      codeTarget: "linux-x64-gnu",
      effectPackage: "@effect/tsgo-linux-x64",
      oxlintPackage: "@oxlint/binding-linux-x64-gnu",
      tsgolintPackage: "@oxlint-tsgolint/linux-x64",
      tsgolintExecutable: "tsgolint"
    })
    expect(experimentalOxlintTarget("win32", "arm64", false)).toEqual({
      codeTarget: "win32-arm64",
      effectPackage: "@effect/tsgo-win32-arm64",
      oxlintPackage: "@oxlint/binding-win32-arm64-msvc",
      tsgolintPackage: "@oxlint-tsgolint/win32-arm64",
      tsgolintExecutable: "tsgolint.exe"
    })
    expect(() => experimentalOxlintTarget("linux", "x64", false)).toThrow(/musl/)
    expect(() => experimentalOxlintTarget("linux", "arm", true)).toThrow(/Unsupported/)
  })

  it("patches and restores all resolved targets", async () => {
    const directory = await makeTemporaryDirectory()
    const addon = join(directory, "oxlint.node")
    const addonReplacement = join(directory, "effect-oxlint.node")
    const tsgolint = join(directory, "tsgolint")
    const tsgolintReplacement = join(directory, "effect-tsgolint")
    await Promise.all([
      writeFile(addon, "original-addon"),
      writeFile(addonReplacement, "effect-addon"),
      writeFile(tsgolint, "original-tsgolint"),
      writeFile(tsgolintReplacement, "effect-tsgolint")
    ])

    const targets = [
      { label: "Oxlint binding", targetPath: addon, replacementPath: addonReplacement, executable: false },
      { label: "tsgolint", targetPath: tsgolint, replacementPath: tsgolintReplacement, executable: true }
    ]
    await Effect.runPromise(patchResolvedTargets(targets).pipe(Effect.provide(NodeServices.layer)))

    expect(await readFile(addon, "utf8")).toBe("effect-addon")
    expect(await readFile(tsgolint, "utf8")).toBe("effect-tsgolint")
    expect(await readFile(addon + ".original", "utf8")).toBe("original-addon")
    expect(await readFile(tsgolint + ".original", "utf8")).toBe("original-tsgolint")

    await Effect.runPromise(unpatchResolvedTargets(targets).pipe(Effect.provide(NodeServices.layer)))
    expect(await readFile(addon, "utf8")).toBe("original-addon")
    expect(await readFile(tsgolint, "utf8")).toBe("original-tsgolint")
  })

  it("does not mutate any target when preflight fails", async () => {
    const directory = await makeTemporaryDirectory()
    const addon = join(directory, "oxlint.node")
    const replacement = join(directory, "effect-oxlint.node")
    await writeFile(addon, "original-addon")
    await writeFile(replacement, "effect-addon")

    const targets = [
      { label: "Oxlint binding", targetPath: addon, replacementPath: replacement, executable: false },
      {
        label: "tsgolint",
        targetPath: join(directory, "missing-tsgolint"),
        replacementPath: join(directory, "missing-replacement"),
        executable: true
      }
    ]
    await expect(Effect.runPromise(patchResolvedTargets(targets).pipe(Effect.provide(NodeServices.layer))))
      .rejects.toThrow(/missing-tsgolint/)
    expect(await readFile(addon, "utf8")).toBe("original-addon")
  })
})
