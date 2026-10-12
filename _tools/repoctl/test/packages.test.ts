import * as NodeServices from "@effect/platform-node/NodeServices"
import * as Effect from "effect/Effect"
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import assert from "node:assert/strict"
import test from "node:test"
import { fileURLToPath } from "node:url"
import { buildTargets, oxlintBindingName, oxlintBuildTargets, verifyReleaseArtifacts } from "../src/build.ts"
import { assembleReleaseArtifacts, bundleUpstream, preparePlatformPackages } from "../src/packages.ts"
import { buildReleasePlan } from "../src/releasePlan.ts"
import { decodeUpstream } from "../src/upstream.ts"

const repositoryRoot = fileURLToPath(new URL("../../..", import.meta.url))
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
    for (const target of Object.keys(oxlintBuildTargets)) {
      const packageRoot = join(repository, "_packages", `tsgo-oxlint-${target}`)
      mkdirSync(packageRoot, { recursive: true })
      writeFileSync(join(packageRoot, "package.json"), JSON.stringify({
        name: `@effect/tsgo-oxlint-${target}`,
        publishConfig: { access: "public" },
        files: ["artifacts/", "README.md"]
      }))
    }

    const artifactsDirectory = join(repository, "_release-artifacts")
    const decodedUpstream = await Effect.runPromise(decodeUpstream(JSON.stringify(upstream)))
    for (const artifact of buildReleasePlan(decodedUpstream)) {
      const directory = join(artifactsDirectory, artifact.artifactName)
      mkdirSync(directory, { recursive: true })
      writeFileSync(join(directory, artifact.fileName), artifact.version)
      if (artifact.component === "typescript") {
        writeFileSync(join(directory, "lib.d.ts"), `lib ${artifact.version}`)
        writeFileSync(join(directory, "lib.es5.d.ts"), `es5 ${artifact.version}`)
      }
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

    const compilerArtifact = buildReleasePlan(decodedUpstream).find((artifact) => artifact.component === "typescript")!
    const libraryPath = join(artifactsDirectory, compilerArtifact.artifactName, "lib.d.ts")
    rmSync(libraryPath)
    await assert.rejects(
      Effect.runPromise(
        assembleReleaseArtifacts(repository, artifactsDirectory).pipe(Effect.provide(NodeServices.layer))
      ),
      /Missing TypeScript libraries/
    )
    writeFileSync(libraryPath, `lib ${compilerArtifact.version}`)

    await Effect.runPromise(
      assembleReleaseArtifacts(repository, artifactsDirectory).pipe(Effect.provide(NodeServices.layer))
    )
    await Effect.runPromise(preparePlatformPackages(repository).pipe(Effect.provide(NodeServices.layer)))

    for (const target of Object.keys(buildTargets)) {
      const packageRoot = join(repository, "_packages", `tsgo-${target}`)
      const extension = target.startsWith("win32-") ? ".exe" : ""
      assert.equal(readFileSync(join(packageRoot, "lib", `tsc${extension}`), "utf8"), "7.0.0")
      const packageJson = JSON.parse(readFileSync(join(packageRoot, "package.json"), "utf8"))
      assert.deepEqual(packageJson.files, [
        "artifacts/", `lib/tsc${extension}`, "README.md", "lib/upstream.json", "lib/lib*.d.ts"
      ])
      assert.equal(readFileSync(join(packageRoot, "lib", "lib.d.ts"), "utf8"), "lib 7.0.0")
      assert.equal(readFileSync(join(packageRoot, "lib", "lib.es5.d.ts"), "utf8"), "es5 7.0.0")
      assert.equal(existsSync(join(packageRoot, "artifacts", "typescript", "7.0.0")), false)
      const next = join(packageRoot, "artifacts", "typescript", "7.1.0")
      assert.equal(readFileSync(join(next, `tsc${extension}`), "utf8"), "7.1.0")
      assert.equal(readFileSync(join(next, "lib.d.ts"), "utf8"), "lib 7.1.0")
      assert.equal(readFileSync(join(next, "lib.es5.d.ts"), "utf8"), "es5 7.1.0")
      assert.deepEqual(
        packageJson.publishConfig.executableFiles,
        [`./lib/tsc${extension}`, `./artifacts/typescript/7.1.0/tsc${extension}`]
      )
      assert.equal(existsSync(join(packageRoot, "artifacts", "oxlint")), false)
      assert.equal(existsSync(join(packageRoot, "artifacts", "oxlint-tsgolint")), false)
    }

    for (const target of Object.keys(oxlintBuildTargets) as Array<keyof typeof oxlintBuildTargets>) {
      const packageRoot = join(repository, "_packages", `tsgo-oxlint-${target}`)
      const extension = target.startsWith("win32-") ? ".exe" : ""
      const binding = join(packageRoot, "artifacts", "oxlint", "1.0.0", oxlintBindingName(target))
      const tsgolint = join(packageRoot, "artifacts", "oxlint-tsgolint", "1.0.0", `tsgolint${extension}`)
      assert.equal(readFileSync(binding, "utf8"), "1.0.0")
      assert.equal(readFileSync(tsgolint, "utf8"), "1.0.0")
      assert.equal(existsSync(join(packageRoot, "artifacts", "typescript")), false)
      const packageJson = JSON.parse(readFileSync(join(packageRoot, "package.json"), "utf8"))
      assert.deepEqual(packageJson.files, ["artifacts/", "README.md"])
      assert.deepEqual(packageJson.publishConfig, {
        access: "public",
        executableFiles: [`./artifacts/oxlint-tsgolint/1.0.0/tsgolint${extension}`]
      })
    }

    await Effect.runPromise(verifyReleaseArtifacts(repository).pipe(Effect.provide(NodeServices.layer)))
    for (const [packageName, artifact] of [
      ["tsgo-linux-x64", "typescript/7.0.0"],
      ["tsgo-linux-x64", "oxlint/1.0.0"],
      ["tsgo-oxlint-darwin-arm64", "typescript/7.1.0"]
    ]) {
      const directory = join(repository, "_packages", packageName, "artifacts", ...artifact.split("/"))
      mkdirSync(directory, { recursive: true })
      writeFileSync(join(directory, "artifact"), "unexpected")
      await assert.rejects(
        Effect.runPromise(verifyReleaseArtifacts(repository).pipe(Effect.provide(NodeServices.layer))),
        new RegExp(`Unexpected artifacts in _packages/${packageName}: ${artifact}`)
      )
      rmSync(directory, { recursive: true })
    }
  } finally {
    rmSync(repository, { recursive: true, force: true })
  }
})

test("publishes the Oxlint builds in @effect/tsgo-oxlint and its platform packages", () => {
  const readPackage = (directory: string) =>
    JSON.parse(readFileSync(join(repositoryRoot, "_packages", directory, "package.json"), "utf8"))
  const cli = readPackage("tsgo")
  const oxlint = readPackage("tsgo-oxlint")
  const oxlintPackages = Object.keys(oxlintBuildTargets).map((target) => `@effect/tsgo-oxlint-${target}`)

  assert.deepEqual(
    Object.keys(cli.optionalDependencies).sort(),
    Object.keys(buildTargets).map((target) => `@effect/tsgo-${target}`).sort()
  )
  assert.equal(oxlint.version, cli.version)
  assert.deepEqual(oxlint.peerDependencies, { "@effect/tsgo": "workspace:*" })
  assert.deepEqual(
    oxlint.optionalDependencies,
    Object.fromEntries(oxlintPackages.map((name) => [name, "workspace:*"]))
  )
  for (const target of Object.keys(oxlintBuildTargets)) {
    const [os, cpu] = target.split("-")
    const platform = readPackage(`tsgo-oxlint-${target}`)
    assert.equal(platform.name, `@effect/tsgo-oxlint-${target}`)
    assert.equal(platform.version, cli.version)
    assert.deepEqual(platform.files, ["artifacts/", "README.md"])
    assert.deepEqual(platform.os, [os])
    assert.deepEqual(platform.cpu, [cpu])
    assert.deepEqual(platform.libc, os === "linux" ? ["glibc"] : undefined)
  }

  const changesets = JSON.parse(readFileSync(join(repositoryRoot, ".changeset", "config.json"), "utf8"))
  for (const name of ["@effect/tsgo-oxlint", ...oxlintPackages]) {
    assert.ok(changesets.fixed[0].includes(name), `${name} is released with @effect/tsgo`)
  }
})
