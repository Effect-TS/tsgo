import * as NodeServices from "@effect/platform-node/NodeServices"
import * as Effect from "effect/Effect"
import assert from "node:assert/strict"
import { mkdirSync, mkdtempSync, rmSync, statSync, utimesSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import test from "node:test"
import { gunzipSync } from "node:zlib"
import { createFixtureArchive, ensureEffectFixtures } from "../src/fixtures.ts"

const writeProfile = (repository: string, profile: string, version: string) => {
  const root = join(repository, "testdata", "tests", profile)
  const packageRoot = join(root, "node_modules", "example")
  mkdirSync(join(packageRoot, "nested"), { recursive: true })
  mkdirSync(join(packageRoot, "nested", "a".repeat(110)), { recursive: true })
  writeFileSync(join(root, "package.json"), JSON.stringify({ dependencies: { example: `^${version}` } }))
  writeFileSync(join(packageRoot, "package.json"), JSON.stringify({ name: "example", version }))
  writeFileSync(join(packageRoot, "nested", "index.d.ts"), "export interface Example {}\n")
  writeFileSync(join(packageRoot, "nested", "a".repeat(110), "long.d.ts"), "export type Long = true\n")
}

test("generates deterministic embedded Effect fixtures", async() => {
  const repository = mkdtempSync(join(tmpdir(), "repoctl-fixtures-"))
  try {
    writeProfile(repository, "effect-v3", "3.0.0")
    writeProfile(repository, "effect-v4", "4.0.0")

    const first = await createFixtureArchive(repository)
    const second = await createFixtureArchive(repository)

    assert.deepEqual(first.archive, second.archive)
    assert.equal(first.archive[9], 0xff)
    assert.equal(gunzipSync(first.archive).subarray(257, 263).toString(), "ustar\0")
    assert.deepEqual(first.manifest.profiles["effect-v4"], {
      requested: { example: "^4.0.0" },
      resolved: { example: "4.0.0" }
    })
    assert.match(first.manifest.treeSha256, /^[a-f0-9]{64}$/)
  } finally {
    rmSync(repository, { recursive: true, force: true })
  }
})

test("does not rewrite unchanged embedded Effect fixtures", async() => {
  const repository = mkdtempSync(join(tmpdir(), "repoctl-fixtures-"))
  try {
    writeProfile(repository, "effect-v3", "3.0.0")
    writeProfile(repository, "effect-v4", "4.0.0")
    const output = join(repository, "internal", "bundledeffect", "testfixtures.tar.gz")

    await Effect.runPromise(ensureEffectFixtures(repository).pipe(Effect.provide(NodeServices.layer)))
    const timestamp = new Date("2000-01-01T00:00:00Z")
    utimesSync(output, timestamp, timestamp)
    await Effect.runPromise(ensureEffectFixtures(repository).pipe(Effect.provide(NodeServices.layer)))

    assert.equal(statSync(output).mtime.toISOString(), timestamp.toISOString())
  } finally {
    rmSync(repository, { recursive: true, force: true })
  }
})
