import assert from "node:assert/strict"
import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import test from "node:test"
import { readUpstream, syncPackageMetadata, validateUpstream } from "./upstream.mjs"

test("validates every upstream profile", () => {
  const upstream = readUpstream()
  assert.equal(validateUpstream(upstream), upstream)

  assert.throws(
    () => validateUpstream({ ...upstream, stable: { ...upstream.stable, tsGitHead: "invalid" } }),
    /stable\.tsGitHead/,
  )
})

test("writes binary compatibility metadata to platform packages", async () => {
  const packagesPath = await mkdtemp(join(tmpdir(), "effect-tsgo-upstream-"))
  const platformPath = join(packagesPath, "tsgo-linux-x64")

  try {
    await mkdir(platformPath)
    await writeFile(join(platformPath, "package.json"), `${JSON.stringify({
      name: "@effect/tsgo-linux-x64",
      version: "0.0.0",
    }, null, 2)}\n`)

    const upstream = readUpstream()
    syncPackageMetadata(upstream, packagesPath)

    const packageJson = JSON.parse(await readFile(join(platformPath, "package.json"), "utf8"))
    assert.deepEqual(packageJson.effectTsgo, {
      binaries: {
        tsc: {
          tsVersion: upstream.stable.tsVersion,
          tsGitHead: upstream.stable.tsGitHead,
        },
        "tsc-next": {
          tsVersion: upstream.next.tsVersion,
          tsGitHead: upstream.next.tsGitHead,
        },
      },
    })
  } finally {
    await rm(packagesPath, { recursive: true, force: true })
  }
})
