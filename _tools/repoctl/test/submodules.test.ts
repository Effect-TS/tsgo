import * as NodeServices from "@effect/platform-node/NodeServices"
import * as Effect from "effect/Effect"
import assert from "node:assert/strict"
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { dirname, join } from "node:path"
import test from "node:test"
import { installIntegrationSources } from "../src/submodules.ts"
import { typeScriptSources } from "../src/upstream.ts"

const createRepository = () => {
  const repository = mkdtempSync(join(tmpdir(), "repoctl-integrations-"))
  mkdirSync(join(repository, "typescript", "tsc", "internal", "checker"), { recursive: true })
  writeFileSync(join(repository, "typescript", ".git"), "gitdir: ../.git/modules/typescript\n")
  return repository
}

const writeIntegrationSource = (repository: string, content: string) => {
  const path = join(repository, "_integrations", "typescript", "tsc", "internal", "checker", "integration.go")
  mkdirSync(dirname(path), { recursive: true })
  writeFileSync(path, content)
}

const install = (repository: string) =>
  Effect.runPromise(installIntegrationSources(repository, typeScriptSources.typescript).pipe(
    Effect.provide(NodeServices.layer)
  ))

test("installs durable integration sources into the compiler checkout", async() => {
  const repository = createRepository()
  try {
    writeIntegrationSource(repository, "package checker\n\nconst first = true\n")
    await install(repository)

    const installed = join(repository, "typescript", "tsc", "internal", "checker", "integration.go")
    assert.equal(readFileSync(installed, "utf8"), "package checker\n\nconst first = true\n")

    writeIntegrationSource(repository, "package checker\n\nconst second = true\n")
    await install(repository)
    assert.equal(readFileSync(installed, "utf8"), "package checker\n\nconst second = true\n")
  } finally {
    rmSync(repository, { recursive: true, force: true })
  }
})

test("skips a provider without integration sources", async() => {
  const repository = createRepository()
  try {
    await install(repository)
    assert.equal(
      existsSync(join(repository, "typescript", "tsc", "internal", "checker", "integration.go")),
      false
    )
  } finally {
    rmSync(repository, { recursive: true, force: true })
  }
})

test("skips an unmaterialized compiler checkout", async() => {
  const repository = createRepository()
  try {
    rmSync(join(repository, "typescript", ".git"))
    writeIntegrationSource(repository, "package checker\n")
    await install(repository)
    assert.equal(
      existsSync(join(repository, "typescript", "tsc", "internal", "checker", "integration.go")),
      false
    )
  } finally {
    rmSync(repository, { recursive: true, force: true })
  }
})
