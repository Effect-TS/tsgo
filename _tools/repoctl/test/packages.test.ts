import * as NodeServices from "@effect/platform-node/NodeServices"
import * as Effect from "effect/Effect"
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import assert from "node:assert/strict"
import test from "node:test"
import { buildTargets, oxlintBuildTargets } from "../src/build.ts"
import { assembleReleaseArtifacts, bundleUpstream, preparePlatformPackages } from "../src/packages.ts"
import { buildReleasePlan } from "../src/releasePlan.ts"
import { decodeUpstream } from "../src/upstream.ts"

const revision = "0123456789abcdef0123456789abcdef01234567"

test("bundles upstream metadata in every platform package", async() => {
  const repository = mkdtempSync(join(tmpdir(), "repoctl-packages-"))
  const upstream = `${JSON.stringify({
    schemaVersion: 5,
    tags: {
      typescript: { latest: "7.0.0", next: "7.1.0" },
      oxlint: { latest: "1.0.0" },
      "oxlint-tsgolint": { latest: "1.0.0" }
    },
    components: {
      typescript: {
        "7.0.0": { gitHead: revision, provider: "typescript-go" },
        "7.1.0": { gitHead: revision, provider: "typescript" }
      },
      "oxlint-tsgolint": {
        "1.0.0": { gitHead: revision, dependencies: { typescript: "7.0.0" } }
      },
      oxlint: { "1.0.0": { gitHead: revision } }
    },
    profiles: [
      {
        name: "vite-plus",
        description: "Vite+ 1.0.0 compatibility runtime",
        dependencies: { oxlint: "1.0.0", "oxlint-tsgolint": "1.0.0" }
      }
    ]
  }, null, 2)}\n`

  try {
    mkdirSync(join(repository, "_packages", "tsgo"), { recursive: true })
    writeFileSync(join(repository, "_packages", "tsgo", "upstream.json"), upstream)

    await Effect.runPromise(bundleUpstream(repository).pipe(Effect.provide(NodeServices.layer)))

    for (const target of Object.keys(buildTargets)) {
      assert.equal(
        readFileSync(join(repository, "_packages", `tsgo-${target}`, "lib", "upstream.json"), "utf8"),
        upstream
      )
    }
  } finally {
    rmSync(repository, { recursive: true, force: true })
  }
})

test("prepares versioned platform artifacts and executable manifests", async() => {
  const repository = mkdtempSync(join(tmpdir(), "repoctl-prepare-packages-"))
  const upstream = {
    schemaVersion: 5,
    tags: {
      typescript: { latest: "7.0.0", next: "7.1.0" },
      oxlint: { latest: "1.0.0" },
      "oxlint-tsgolint": { latest: "1.0.0" }
    },
    components: {
      typescript: {
        "7.0.0": { gitHead: revision, provider: "typescript-go" },
        "7.1.0": { gitHead: revision, provider: "typescript" }
      },
      "oxlint-tsgolint": {
        "1.0.0": { gitHead: revision, dependencies: { typescript: "7.0.0" } }
      },
      oxlint: { "1.0.0": { gitHead: revision } }
    },
    profiles: [
      {
        name: "vite-plus",
        description: "Vite+ 1.0.0 compatibility runtime",
        dependencies: { oxlint: "1.0.0", "oxlint-tsgolint": "1.0.0" }
      }
    ]
  }

  try {
    mkdirSync(join(repository, "_packages", "tsgo"), { recursive: true })
    writeFileSync(join(repository, "_packages", "tsgo", "upstream.json"), `${JSON.stringify(upstream, null, 2)}\n`)
    for (const target of Object.keys(buildTargets)) {
      const packageRoot = join(repository, "_packages", `tsgo-${target}`)
      const extension = target.startsWith("win32-") ? ".exe" : ""
      mkdirSync(packageRoot, { recursive: true })
      writeFileSync(join(packageRoot, "package.json"), JSON.stringify({
        name: `@effect/tsgo-${target}`,
        publishConfig: { access: "public", executableFiles: [] },
        files: ["artifacts/", `lib/tsc${extension}`, "README.md", "lib/upstream.json"]
      }), { flag: "w" })
    }

    const artifactsDirectory = join(repository, "_release-artifacts")
    const decodedUpstream = await Effect.runPromise(decodeUpstream(JSON.stringify(upstream)))
    for (const artifact of buildReleasePlan(decodedUpstream)) {
      const directory = join(artifactsDirectory, artifact.artifactName)
      mkdirSync(directory, { recursive: true })
      writeFileSync(join(directory, artifact.fileName), artifact.version)
    }

    const unexpected = join(artifactsDirectory, "unexpected")
    mkdirSync(unexpected)
    await assert.rejects(
      Effect.runPromise(
        assembleReleaseArtifacts(repository, artifactsDirectory).pipe(Effect.provide(NodeServices.layer))
      ),
      /unexpected artifacts: unexpected/
    )
    rmSync(unexpected, { recursive: true })

    await Effect.runPromise(
      assembleReleaseArtifacts(repository, artifactsDirectory).pipe(Effect.provide(NodeServices.layer))
    )
    await Effect.runPromise(preparePlatformPackages(repository).pipe(Effect.provide(NodeServices.layer)))

    for (const target of Object.keys(buildTargets)) {
      const packageRoot = join(repository, "_packages", `tsgo-${target}`)
      const extension = target.startsWith("win32-") ? ".exe" : ""
      assert.equal(readFileSync(join(packageRoot, "lib", `tsc${extension}`), "utf8"), "7.0.0")
      const packageJson = JSON.parse(readFileSync(join(packageRoot, "package.json"), "utf8"))
      assert.deepEqual(packageJson.files, ["artifacts/", `lib/tsc${extension}`, "README.md", "lib/upstream.json"])
      assert(packageJson.publishConfig.executableFiles.includes(
        `./artifacts/typescript/7.1.0/tsc${extension}`
      ))
      assert.equal(
        packageJson.publishConfig.executableFiles.some((file: string) => file.includes("oxlint-tsgolint")),
        target in oxlintBuildTargets
      )
    }
  } finally {
    rmSync(repository, { recursive: true, force: true })
  }
})
