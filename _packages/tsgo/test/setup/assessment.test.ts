import * as NodeServices from "@effect/platform-node/NodeServices"
import * as Effect from "effect/Effect"
import * as Option from "effect/Option"
import { mkdtemp, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { afterEach, describe, expect, it } from "vitest"
import { createAssessmentInput } from "../../src/cli/setup/assessment.js"

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
})
