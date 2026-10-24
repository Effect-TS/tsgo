import * as NodeServices from "@effect/platform-node/NodeServices"
import * as Effect from "effect/Effect"
import {
  chmodSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync
} from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import assert from "node:assert/strict"
import test from "node:test"
import { buildTargets, oxlintBuildTargets, tsrsBuildTargets, verifyReleaseArtifacts } from "../src/build.ts"
import { assembleReleaseArtifacts, bundleUpstream, preparePlatformPackages } from "../src/packages.ts"
import { buildReleasePlan } from "../src/releasePlan.ts"
import { decodeUpstream } from "../src/upstream.ts"

const revision = "0123456789abcdef0123456789abcdef01234567"
const tsrsVersion = "0.18.0-ts7.1.0"
const tsrsVersions = [tsrsVersion, "0.19.0-ts7.1.0"]

test("bundles upstream metadata in every platform package", async() => {
  const repository = mkdtempSync(join(tmpdir(), "repoctl-packages-"))
  const upstream = `${JSON.stringify({
    schemaVersion: 5,
    tags: {
      typescript: { latest: "7.0.0", beta: "7.1.0", next: "7.1.0" },
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
      typescript: { latest: "7.0.0", beta: "7.1.0", next: "7.1.0" },
      oxlint: { latest: "1.0.0" },
      "oxlint-tsgolint": { latest: "1.0.0" },
      tsrs: { latest: tsrsVersion }
    },
    components: {
      typescript: {
        "7.0.0": { gitHead: revision, provider: "typescript-go" },
        "7.1.0": { gitHead: revision, provider: "typescript" }
      },
      "oxlint-tsgolint": {
        "1.0.0": { gitHead: revision, dependencies: { typescript: "7.0.0" } }
      },
      oxlint: { "1.0.0": { gitHead: revision } },
      tsrs: Object.fromEntries(tsrsVersions.map((version) => [
        version,
        { gitHead: revision, typescript: { version: "7.1.0", gitHead: revision } }
      ]))
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
      if (artifact.component === "typescript") {
        writeFileSync(join(directory, "lib.d.ts"), `lib ${artifact.version}`)
        writeFileSync(join(directory, "lib.es5.d.ts"), `es5 ${artifact.version}`)
      }
    }
    assert.deepEqual(
      readdirSync(artifactsDirectory).filter((name) => name.includes("__tsrs__")).sort(),
      tsrsVersions.flatMap((version) =>
        ["darwin-arm64", "linux-arm64", "linux-x64"].map((target) => `${target}__tsrs__${version}`)).sort()
    )

    const tsrsArtifactPath = join(artifactsDirectory, `linux-x64__tsrs__${tsrsVersion}`, "tsrs")
    writeFileSync(`${tsrsArtifactPath}.debug`, "debug")
    await assert.rejects(
      Effect.runPromise(
        assembleReleaseArtifacts(repository, artifactsDirectory).pipe(Effect.provide(NodeServices.layer))
      ),
      /linux-x64__tsrs__0\.18\.0-ts7\.1\.0 must contain tsrs only, found tsrs, tsrs\.debug/
    )
    rmSync(`${tsrsArtifactPath}.debug`)

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
        packageJson.publishConfig.executableFiles.filter((file: string) => !file.includes("oxlint-tsgolint")),
        [
          `./lib/tsc${extension}`,
          `./artifacts/typescript/7.1.0/tsc${extension}`,
          ...(target in tsrsBuildTargets ? tsrsVersions.map((version) => `./artifacts/tsrs/${version}/tsrs`) : [])
        ]
      )
      for (const version of tsrsVersions) {
        const tsrsBinary = join(packageRoot, "artifacts", "tsrs", version, "tsrs")
        assert.equal(existsSync(tsrsBinary), target in tsrsBuildTargets)
        if (target in tsrsBuildTargets) {
          assert.equal(readFileSync(tsrsBinary, "utf8"), version)
          assert.equal(statSync(tsrsBinary).mode & 0o777, 0o755)
        }
      }
      assert.equal(
        packageJson.publishConfig.executableFiles.some((file: string) => file.includes("oxlint-tsgolint")),
        target in oxlintBuildTargets
      )
    }

    await Effect.runPromise(verifyReleaseArtifacts(repository).pipe(Effect.provide(NodeServices.layer)))
    const strayTsrs = join(repository, "_packages", "tsgo-win32-x64", "artifacts", "tsrs")
    mkdirSync(strayTsrs)
    await assert.rejects(
      Effect.runPromise(verifyReleaseArtifacts(repository).pipe(Effect.provide(NodeServices.layer))),
      /tsrs is not built for win32-x64/
    )
    rmSync(strayTsrs, { recursive: true })
    const linuxTsrs = join(repository, "_packages", "tsgo-linux-x64", "artifacts", "tsrs", tsrsVersion, "tsrs")
    chmodSync(linuxTsrs, 0o644)
    await assert.rejects(
      Effect.runPromise(verifyReleaseArtifacts(repository).pipe(Effect.provide(NodeServices.layer))),
      /Artifact is not executable: .*linux-x64.*tsrs/
    )
    chmodSync(linuxTsrs, 0o755)
    const duplicate = join(repository, "_packages", "tsgo-linux-x64", "artifacts", "typescript", "7.0.0")
    mkdirSync(duplicate)
    writeFileSync(join(duplicate, "tsc"), "7.0.0")
    await assert.rejects(
      Effect.runPromise(verifyReleaseArtifacts(repository).pipe(Effect.provide(NodeServices.layer))),
      /The latest TypeScript compiler ships only in lib\//
    )
  } finally {
    rmSync(repository, { recursive: true, force: true })
  }
})
