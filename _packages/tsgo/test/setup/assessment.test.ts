import * as NodeServices from "@effect/platform-node/NodeServices"
import * as Effect from "effect/Effect"
import * as Option from "effect/Option"
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { afterEach, describe, expect, it } from "vitest"
import { assess, createAssessmentInput } from "../../src/cli/setup/assessment.js"

const temporaryDirectories: Array<string> = []

afterEach(async() => {
  await Promise.all(temporaryDirectories.splice(0).map((directory) => rm(directory, { recursive: true, force: true })))
})

describe("createAssessmentInput", () => {
  it("reads an existing .oxlintrc.json from the current directory", async() => {
    const directory = await mkdtemp(join(tmpdir(), "effect-tsgo-setup-"))
    temporaryDirectories.push(directory)
    const oxlintConfigText = '{ "rules": { "no-debugger": "error" } }\n'
    await Promise.all([
      writeFile(join(directory, "package.json"), "{}\n"),
      writeFile(join(directory, ".oxlintrc.json"), oxlintConfigText)
    ])

    const input = await Effect.runPromise(createAssessmentInput(directory, {
      fileName: join(directory, "tsconfig.json"),
      text: "{}\n"
    }).pipe(Effect.provide(NodeServices.layer)))

    expect(Option.getOrThrow(input.oxlintConfig)).toEqual({
      fileName: join(directory, ".oxlintrc.json"),
      text: oxlintConfigText
    })
  })

  it("reads an existing .zed/settings.json without normalizing its JSONC", async() => {
    const directory = await mkdtemp(join(tmpdir(), "effect-tsgo-setup-"))
    temporaryDirectories.push(directory)
    const zedDirectory = join(directory, ".zed")
    const zedSettingsText = `{
  // This comment must survive assessment.
  "theme": "One Dark"
}
`
    await mkdir(zedDirectory)
    await Promise.all([
      writeFile(join(directory, "package.json"), "{}\n"),
      writeFile(join(zedDirectory, "settings.json"), zedSettingsText)
    ])

    const input = await Effect.runPromise(createAssessmentInput(directory, {
      fileName: join(directory, "tsconfig.json"),
      text: "{}\n"
    }).pipe(Effect.provide(NodeServices.layer)))

    expect(Option.getOrThrow(input.zedSettings)).toEqual({
      fileName: join(directory, ".zed", "settings.json"),
      text: zedSettingsText
    })
  })

  it("reports a missing .zed/settings.json as absent", async() => {
    const directory = await mkdtemp(join(tmpdir(), "effect-tsgo-setup-"))
    temporaryDirectories.push(directory)
    await writeFile(join(directory, "package.json"), "{}\n")

    const input = await Effect.runPromise(createAssessmentInput(directory, {
      fileName: join(directory, "tsconfig.json"),
      text: "{}\n"
    }).pipe(Effect.provide(NodeServices.layer)))

    expect(input.zedSettings).toEqual(Option.none())
  })
})

describe("assess", () => {
  const createInput = (zedSettingsText: string) => ({
    packageJson: {
      fileName: "/project/package.json",
      text: "{}"
    },
    tsconfig: {
      fileName: "/project/tsconfig.json",
      text: "{}"
    },
    oxlintConfig: Option.none(),
    vscodeSettings: Option.none(),
    zedSettings: Option.some({
      fileName: "/project/.zed/settings.json",
      text: zedSettingsText
    })
  })

  it("assesses valid Zed JSONC with comments and trailing commas", () => {
    const state = assess(createInput(`{
  // Keep comments accepted by Zed.
  "theme": "One Dark",
}`))

    expect(Option.getOrThrow(state.zedSettings).parsed).toEqual({
      theme: "One Dark"
    })
  })

  it("rejects malformed existing Zed JSONC with its path", () => {
    expect(() => assess(createInput(`{
  "theme":,
}`))).toThrow("Invalid editor settings at /project/.zed/settings.json.")
  })
})
