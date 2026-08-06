import * as NodeServices from "@effect/platform-node/NodeServices"
import * as Effect from "effect/Effect"
import * as Command from "effect/unstable/cli/Command"
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { afterEach, describe, expect, it } from "vitest"
import { setupCommand } from "../../src/cli/setup/index.js"

const originalCwd = process.cwd()

afterEach(() => {
  process.chdir(originalCwd)
})

const runSetup = (args: ReadonlyArray<string>) =>
  Effect.runPromise(
    Command.runWith(setupCommand, { version: "test" })(args).pipe(Effect.provide(NodeServices.layer))
  )

describe("setup command", () => {
  it("previews or applies recommended defaults without prompting", async () => {
    const projectDir = await mkdtemp(join(tmpdir(), "effect-tsgo-setup-"))
    const packageJsonPath = join(projectDir, "package.json")
    const tsconfigPath = join(projectDir, "tsconfig.json")
    await writeFile(packageJsonPath, JSON.stringify({ name: "test-project", devDependencies: {} }, null, 2))
    await writeFile(tsconfigPath, JSON.stringify({ compilerOptions: {} }, null, 2))
    process.chdir(projectDir)

    try {
      const args = ["--non-interactive", "--project", "tsconfig.json", "--accept-defaults"]
      await runSetup(args)
      expect(await readFile(packageJsonPath, "utf8")).not.toContain("@effect/tsgo")

      await runSetup([...args, "--apply"])
      expect(await readFile(packageJsonPath, "utf8")).toContain("@effect/tsgo")
      expect(await readFile(tsconfigPath, "utf8")).toContain("@effect/language-service")
    } finally {
      process.chdir(originalCwd)
      await rm(projectDir, { recursive: true, force: true })
    }
  })

  it("fails instead of prompting when the project is missing", async () => {
    await expect(runSetup(["--non-interactive", "--accept-defaults"]))
      .rejects.toThrow("Non-interactive setup requires --project")
  })
})
