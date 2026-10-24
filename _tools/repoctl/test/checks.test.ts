import * as NodeServices from "@effect/platform-node/NodeServices"
import * as Effect from "effect/Effect"
import assert from "node:assert/strict"
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import test from "node:test"
import { checkOverrideFolders } from "../src/checks.ts"
import type { Upstream } from "../src/upstream.ts"

const revision = "0123456789abcdef0123456789abcdef01234567"
const secondRevision = "1123456789abcdef0123456789abcdef01234567"

const writeFiles = (repository: string, directory: string, names: ReadonlyArray<string>) => {
  mkdirSync(join(repository, directory), { recursive: true })
  for (const name of names) {
    writeFileSync(join(repository, directory, name), "")
  }
}

const typescript: typeof Upstream.Type.components.typescript = {
  "7.0.0": { gitHead: revision, provider: "typescript-go" },
  "7.1.0": { gitHead: secondRevision, provider: "typescript" }
}

const withRepository = async(run: (repository: string, check: () => Promise<void>) => Promise<void>) => {
  const repository = mkdtempSync(join(tmpdir(), "repoctl-override-folders-"))
  const check = () =>
    Effect.runPromise(checkOverrideFolders(repository, typescript).pipe(Effect.provide(NodeServices.layer)))
  try {
    writeFiles(repository, "_patches/typescript", ["001-a.patch", "002-b.patch", "README.md"])
    writeFiles(repository, "_patches/typescript-go", ["001-x.patch"])
    writeFiles(repository, "_patches/tsrs", ["001-t.patch"])
    writeFiles(repository, "_tools/gen_shims/providers/typescript/ast", ["integration.go"])
    writeFiles(repository, "_tools/gen_shims/providers/typescript-go/ast", ["integration.go"])
    await run(repository, check)
  } finally {
    rmSync(repository, { recursive: true, force: true })
  }
}

test("accepts the provider folders and override folders of recorded TypeScript commits", () =>
  withRepository(async(repository, check) => {
    // The plain provider folders (`typescript-go` included) are no overrides.
    await check()

    writeFiles(repository, `_patches/typescript-${secondRevision}`, ["001-a.patch", "002-b.patch", "README.md"])
    writeFiles(repository, `_patches/typescript-go-${revision}`, ["001-x.patch"])
    // Shim override folders are self-contained and may hold other packages than the provider's.
    writeFiles(repository, `_tools/gen_shims/providers/typescript-${secondRevision}/tsoptions`, ["compatibility.go"])
    await check()
  }))

test("fails on an override folder that names no recorded TypeScript commit of its provider", () =>
  withRepository(async(repository, check) => {
    // The commit of the typescript-go component, under the typescript provider.
    writeFiles(repository, `_tools/gen_shims/providers/typescript-${revision}/ast`, ["compatibility.go"])
    await assert.rejects(check(), (error: Error) => {
      assert.match(
        error.message,
        new RegExp(`Stale override folder _tools/gen_shims/providers/typescript-${revision}: .*Delete it`)
      )
      return true
    })
    rmSync(join(repository, `_tools/gen_shims/providers/typescript-${revision}`), { recursive: true })

    const unknown = "2123456789abcdef0123456789abcdef01234567"
    writeFiles(repository, `_patches/typescript-go-${unknown}`, ["001-x.patch"])
    await assert.rejects(check(), new RegExp(`Stale override folder _patches/typescript-go-${unknown}`))
  }))

test("requires a patch override folder to mirror the provider's folder patch for patch", () =>
  withRepository(async(repository, check) => {
    const mirror = `_patches/typescript-${secondRevision}`
    writeFiles(repository, mirror, ["001-a.patch", "002-b.patch"])
    await check()

    // A patch added to _patches/typescript only, and one only in the mirror (READMEs are not patches).
    writeFiles(repository, "_patches/typescript", ["003-c.patch"])
    writeFiles(repository, mirror, ["004-d.patch", "README.md"])
    await assert.rejects(check(), (error: Error) => {
      assert.match(error.message, new RegExp(`${mirror} must mirror _patches/typescript patch for patch`))
      assert.match(error.message, new RegExp(`Missing from ${mirror}: 003-c\\.patch`))
      assert.match(error.message, /Not in _patches\/typescript: 004-d\.patch/)
      return true
    })
  }))
