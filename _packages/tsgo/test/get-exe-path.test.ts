import { execFileSync } from "node:child_process"
import { copyFile, mkdir, mkdtemp, realpath, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { pathToFileURL } from "node:url"
import { afterEach, describe, expect, it } from "vitest"
import upstreamJson from "../upstream.json" with { type: "json" }

const temporaryDirectories: Array<string> = []
const executable = process.platform === "win32" ? "tsc.exe" : "tsc"
const latest = upstreamJson.tags.typescript.latest

afterEach(async () => {
  await Promise.all(temporaryDirectories.splice(0).map((directory) => rm(directory, { recursive: true, force: true })))
})

const writePackage = async (directory: string, packageJson: Record<string, unknown>) => {
  await mkdir(directory, { recursive: true })
  await writeFile(join(directory, "package.json"), JSON.stringify(packageJson))
}

const makeProject = async (typescriptVersion: string) => {
  const directory = await realpath(await mkdtemp(join(tmpdir(), "effect-tsgo-get-exe-path-")))
  temporaryDirectories.push(directory)
  const nodeModules = join(directory, "node_modules")
  const helperDirectory = join(nodeModules, "@effect", "tsgo")
  await writePackage(helperDirectory, { name: "@effect/tsgo", type: "module" })
  await mkdir(join(helperDirectory, "lib"))
  await copyFile(new URL("../lib/getExePath.js", import.meta.url), join(helperDirectory, "lib", "getExePath.js"))

  const platformName = `@effect/tsgo-${process.platform}-${process.arch}`
  const platformDirectory = join(nodeModules, ...platformName.split("/"))
  await writePackage(platformDirectory, { name: platformName })
  await mkdir(join(platformDirectory, "lib"))
  await writeFile(join(platformDirectory, "lib", "upstream.json"), JSON.stringify(upstreamJson))
  await writeFile(join(platformDirectory, "lib", executable), latest)
  for (const version of Object.keys(upstreamJson.components.typescript)) {
    if (version === latest) continue
    const artifactDirectory = join(platformDirectory, "artifacts", "typescript", version)
    await mkdir(artifactDirectory, { recursive: true })
    await writeFile(join(artifactDirectory, executable), version)
  }

  const components: Record<string, { readonly gitHead: string }> = upstreamJson.components.typescript
  await writePackage(join(nodeModules, "typescript"), {
    name: "typescript",
    version: typescriptVersion,
    gitHead: components[typescriptVersion]!.gitHead
  })
  return { directory, platformDirectory, helper: join(helperDirectory, "lib", "getExePath.js") }
}

const getExePath = (project: { readonly directory: string; readonly helper: string }) =>
  execFileSync(process.execPath, [
    "--input-type=module",
    "--eval",
    `import getExePath from ${JSON.stringify(pathToFileURL(project.helper).href)}\nprocess.stdout.write(getExePath())`
  ], { cwd: project.directory, encoding: "utf8" })

describe("getExePath", () => {
  it.each(Object.keys(upstreamJson.components.typescript))(
    "resolves the packaged compiler matching installed TypeScript %s",
    async (version) => {
      const project = await makeProject(version)
      expect(getExePath(project)).toBe(
        version === latest
          ? join(project.platformDirectory, "lib", executable)
          : join(project.platformDirectory, "artifacts", "typescript", version, executable)
      )
    }
  )
})
