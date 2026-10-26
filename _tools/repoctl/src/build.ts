import * as Console from "effect/Console"
import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"
import * as Path from "effect/Path"
import { constants } from "node:fs"
import { access, appendFile } from "node:fs/promises"
import { dirname, join } from "node:path"
import { ensureEffectFixtures } from "./fixtures.ts"
import { runCommand, runCommandString } from "./process.ts"
import { buildRustBinary, missingRustBinaryTools, tsrsCheckedOut, tsrsSetupCommand } from "./rust.ts"
import {
  readUpstream,
  resolveMaterializedTypeScriptSource,
  type ComponentName,
  typeScriptSourceEntrypoint,
  type Upstream
} from "./upstream.ts"

export const buildTargets = {
  "darwin-arm64": { goos: "darwin", goarch: "arm64" },
  "darwin-x64": { goos: "darwin", goarch: "amd64" },
  "win32-x64": { goos: "windows", goarch: "amd64" },
  "win32-arm64": { goos: "windows", goarch: "arm64" },
  "linux-x64": { goos: "linux", goarch: "amd64" },
  "linux-arm64": { goos: "linux", goarch: "arm64" },
  "linux-arm": { goos: "linux", goarch: "arm", goarm: "6" }
} as const

export type BuildTarget = keyof typeof buildTargets

export const oxlintBuildTargets = {
  "darwin-arm64": {
    ...buildTargets["darwin-arm64"],
    rustTarget: "aarch64-apple-darwin",
    codeTarget: "darwin-arm64",
    napiArgs: []
  },
  "darwin-x64": {
    ...buildTargets["darwin-x64"],
    rustTarget: "x86_64-apple-darwin",
    codeTarget: "darwin-x64",
    napiArgs: []
  },
  "win32-x64": {
    ...buildTargets["win32-x64"],
    rustTarget: "x86_64-pc-windows-msvc",
    codeTarget: "win32-x64",
    napiArgs: []
  },
  "win32-arm64": {
    ...buildTargets["win32-arm64"],
    rustTarget: "aarch64-pc-windows-msvc",
    codeTarget: "win32-arm64",
    napiArgs: []
  },
  "linux-x64": {
    ...buildTargets["linux-x64"],
    rustTarget: "x86_64-unknown-linux-gnu",
    codeTarget: "linux-x64-gnu",
    napiArgs: ["--use-napi-cross"]
  },
  "linux-arm64": {
    ...buildTargets["linux-arm64"],
    rustTarget: "aarch64-unknown-linux-gnu",
    codeTarget: "linux-arm64-gnu",
    napiArgs: ["--use-napi-cross"]
  }
} as const

export type OxlintBuildTarget = keyof typeof oxlintBuildTargets

// The patched tsrs binary (crates/ets_tsrs, binary `tsrs`), released per tsrs version (upstream.json components.tsrs)
// for the targets tsrs publishes a glibc or macOS binary for (tsrs/.github/workflows/release.yml), each built natively
// on the runner tsrs uses (ubuntu-22.04 keeps the glibc requirement at 2.35). `effect-tsgo patch --tsrs` swaps it in
// for the binary of the user's installed `@tsrs/<target>` package; tsrs's musl targets are reported as unsupported.
export const tsrsBuildTargets = {
  "darwin-arm64": { rustTarget: "aarch64-apple-darwin", runner: "macos-14" },
  "linux-x64": { rustTarget: "x86_64-unknown-linux-gnu", runner: "ubuntu-22.04" },
  "linux-arm64": { rustTarget: "aarch64-unknown-linux-gnu", runner: "ubuntu-22.04-arm" }
} as const satisfies Partial<Record<BuildTarget, { readonly rustTarget: string; readonly runner: string }>>

export type TsrsBuildTarget = keyof typeof tsrsBuildTargets

export const isTsrsBuildTarget = (target: string): target is TsrsBuildTarget => target in tsrsBuildTargets

export const tsrsBinaryName = "tsrs"

export const oxlintBindingName = (target: OxlintBuildTarget) =>
  `oxlint.${oxlintBuildTargets[target].codeTarget}${target.startsWith("win32-") ? "-msvc" : ""}.node`

export class BuildError extends Data.TaggedError("BuildError")<{
  readonly reason: string
}> {
  get message(): string {
    return `Build failed: ${this.reason}`
  }
}

export const componentArtifact = (
  repositoryRoot: string,
  target: BuildTarget,
  component: "typescript" | "oxlint-tsgolint" | "oxlint" | "tsrs",
  version: string,
  binaryName: string
) => {
  const executable = target.startsWith("win32-") && !binaryName.endsWith(".node") ? `${binaryName}.exe` : binaryName
  return {
    binaryName,
    path: join(repositoryRoot, "_packages", `tsgo-${target}`, "artifacts", component, version, executable)
  }
}

export const latestTypeScriptCompiler = (repositoryRoot: string, target: BuildTarget) =>
  join(repositoryRoot, "_packages", `tsgo-${target}`, "lib", target.startsWith("win32-") ? "tsc.exe" : "tsc")

export const packagedTypeScriptCompiler = (
  repositoryRoot: string,
  target: BuildTarget,
  upstream: typeof Upstream.Type,
  version: string
) =>
  version === upstream.tags.typescript.latest
    ? latestTypeScriptCompiler(repositoryRoot, target)
    : componentArtifact(repositoryRoot, target, "typescript", version, "tsc").path

export const oxlintArtifacts = (
  repositoryRoot: string,
  targetName: OxlintBuildTarget,
  oxlintVersion: string,
  tsgolintVersion: string
) => {
  const packageDirectory = join(repositoryRoot, "_packages", `tsgo-${targetName}`, "artifacts")
  const windows = targetName.startsWith("win32-")
  const bindingName = oxlintBindingName(targetName)
  return {
    packageDirectory,
    bindingName,
    bindingSourceName: bindingName,
    bindingPath: join(packageDirectory, "oxlint", oxlintVersion, bindingName),
    tsgolintPath: join(
      packageDirectory,
      "oxlint-tsgolint",
      tsgolintVersion,
      windows ? "tsgolint.exe" : "tsgolint"
    )
  }
}

const validateArtifact = Effect.fnUntraced(function*(artifact: string) {
  const fs = yield* FileSystem.FileSystem
  const info = yield* fs.stat(artifact).pipe(
    Effect.mapError((error) => new BuildError({ reason: `Missing artifact ${artifact}: ${error.message}` }))
  )
  if (info.size === 0n) {
    return yield* new BuildError({ reason: `Artifact is empty: ${artifact}` })
  }
})

const validateExecutableArtifact = Effect.fnUntraced(function*(artifact: string) {
  yield* validateArtifact(artifact)
  yield* Effect.tryPromise({
    try: () => access(artifact, constants.X_OK),
    catch: () => new BuildError({ reason: `Artifact is not executable: ${artifact}` })
  })
})

export const copyTypeScriptLibraries = Effect.fnUntraced(function*(source: string, destination: string) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const libraries = (yield* fs.readDirectory(source)).filter((name) => name.endsWith(".d.ts"))
  if (!libraries.includes("lib.d.ts")) {
    return yield* new BuildError({ reason: `Missing TypeScript libraries in ${source}` })
  }
  yield* fs.makeDirectory(destination, { recursive: true })
  for (const library of libraries) {
    yield* validateArtifact(path.join(source, library))
    yield* fs.copyFile(path.join(source, library), path.join(destination, library))
  }
})

export const buildCli = Effect.fnUntraced(function*(repositoryRoot: string) {
  const path = yield* Path.Path
  yield* Console.log("Building CLI package")
  yield* runCommand("pnpm", path.join(repositoryRoot, "_packages", "tsgo"), [
    "exec",
    "tsdown",
    "--config",
    "tsdown.config.ts"
  ])
  yield* validateArtifact(path.join(repositoryRoot, "_packages", "tsgo", "dist", "effect-tsgo.cjs"))
})

export const buildLocal = Effect.fnUntraced(function*(repositoryRoot: string) {
  const path = yield* Path.Path
  const binary = path.join(repositoryRoot, "tsgo")
  const compiler = yield* resolveMaterializedTypeScriptSource(repositoryRoot)
  yield* ensureEffectFixtures(repositoryRoot)
  yield* Console.log("Building local Go binary")
  yield* runCommand("go", repositoryRoot, [
    "build",
    "-o",
    binary,
    typeScriptSourceEntrypoint(compiler)
  ], false, { CGO_ENABLED: "0" })
  yield* validateArtifact(binary)
  yield* buildCli(repositoryRoot)
  yield* buildLocalRustBinary(repositoryRoot)
})

// The Rust engine for local use (dev profile, which is optimized: Cargo.toml). Optional: skipped with a message
// without tsrs/ (the default setup), cargo or CMake.
const buildLocalRustBinary = Effect.fnUntraced(function*(repositoryRoot: string) {
  if (!(yield* tsrsCheckedOut(repositoryRoot))) {
    yield* Console.log(`Skipping the local tsrs binary: tsrs/ is not checked out (${tsrsSetupCommand})`)
    return
  }
  const missing = yield* missingRustBinaryTools(repositoryRoot)
  if (missing !== undefined) {
    yield* Console.log(`Skipping the local tsrs binary: ${missing}`)
    return
  }
  const binary = yield* buildRustBinary(repositoryRoot, { release: false })
  yield* validateExecutableArtifact(binary)
  yield* Console.log(`Built the local tsrs binary: ${binary}`)
})

// The release build of the patched tsrs binary of one tsrs version for one platform, natively on that platform (tsrs's
// CLI links mimalloc, a C library built with CMake, which rules out a plain cross build). Requires the tsrs checkout of
// that version (`repoctl submodules setup --component tsrs --version <version>`).
export const buildTsrs = Effect.fnUntraced(function*(
  repositoryRoot: string,
  version: string,
  targetName: TsrsBuildTarget
) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const upstream = yield* readUpstream(repositoryRoot)
  const component = upstream.components.tsrs?.[version]
  if (component === undefined) {
    return yield* new BuildError({ reason: `Unknown tsrs component version ${version}` })
  }
  const host = `${process.platform}-${process.arch}`
  if (host !== targetName) {
    return yield* new BuildError({ reason: `The tsrs binary for ${targetName} is built natively, found host ${host}` })
  }
  const checkoutRevision = (yield* runCommandString("git", repositoryRoot, ["-C", "tsrs", "rev-parse", "HEAD"])).trim()
  if (checkoutRevision !== component.gitHead) {
    return yield* new BuildError({
      reason: `tsrs ${version} expects checkout ${component.gitHead}, found ${checkoutRevision}`
    })
  }
  const missing = yield* missingRustBinaryTools(repositoryRoot)
  if (missing !== undefined) {
    return yield* new BuildError({ reason: `Unable to build the tsrs binary: ${missing}` })
  }
  const binary = yield* buildRustBinary(repositoryRoot, {
    release: true,
    rustTarget: tsrsBuildTargets[targetName].rustTarget
  })
  const artifact = componentArtifact(repositoryRoot, targetName, "tsrs", version, tsrsBinaryName)
  yield* Console.log(`Copying tsrs ${version} for ${targetName} -> ${artifact.path}`)
  yield* fs.remove(path.dirname(artifact.path), { recursive: true, force: true })
  yield* fs.makeDirectory(path.dirname(artifact.path), { recursive: true })
  yield* fs.copyFile(binary, artifact.path)
  yield* copyTypeScriptLibraries(
    path.join(repositoryRoot, "tsrs", "crates", "tsrs_vfs", "libs"),
    path.dirname(artifact.path)
  )
  yield* fs.chmod(artifact.path, 0o755)
  yield* validateExecutableArtifact(artifact.path)
  yield* runCommand(artifact.path, repositoryRoot, ["--version"])

  if (process.env.GITHUB_OUTPUT !== undefined) {
    yield* Effect.tryPromise(() => appendFile(
      process.env.GITHUB_OUTPUT!,
      `component=tsrs\nnpm_version=${version}\nartifact_path=${path.dirname(artifact.path)}\n`
    ))
  }
  return artifact.path
})

const buildTsgolint = Effect.fnUntraced(function*(
  repositoryRoot: string,
  version: string,
  targetName: OxlintBuildTarget
) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const tsgolintSource = path.join(repositoryRoot, "tsgolint", "cmd", "tsgolint", "effect_rules_generated.go")
  if (!(yield* fs.exists(tsgolintSource))) {
    return yield* new BuildError({ reason: `Missing generated source ${tsgolintSource}; run profile codegen first` })
  }

  const upstream = yield* readUpstream(repositoryRoot)
  const component = upstream.components["oxlint-tsgolint"][version]
  if (component === undefined) {
    return yield* new BuildError({ reason: `Unknown oxlint-tsgolint component version ${version}` })
  }
  const gitHead = (yield* runCommandString("git", repositoryRoot, ["-C", "tsgolint", "rev-parse", "HEAD"])).trim()
  if (gitHead !== component.gitHead) {
    return yield* new BuildError({
      reason: `oxlint-tsgolint ${version} expects checkout ${component.gitHead}, found ${gitHead}`
    })
  }
  const target = oxlintBuildTargets[targetName]
  const artifact = componentArtifact(repositoryRoot, targetName, "oxlint-tsgolint", version, "tsgolint")
  yield* fs.makeDirectory(path.dirname(artifact.path), { recursive: true })
  yield* Console.log(`Building tsgolint for ${targetName}`)
  yield* runCommand("go", repositoryRoot, [
    "build",
    "-trimpath",
    "-ldflags=-s -w",
    "-o",
    artifact.path,
    "./tsgolint/cmd/tsgolint"
  ], false, {
    GOWORK: path.join(repositoryRoot, "go.work"),
    CGO_ENABLED: "0",
    GOOS: target.goos,
    GOARCH: target.goarch
  })
  yield* validateArtifact(artifact.path)

  if (process.env.GITHUB_OUTPUT !== undefined) {
    yield* Effect.tryPromise(() => appendFile(
      process.env.GITHUB_OUTPUT!,
      `component=oxlint-tsgolint\nnpm_version=${version}\nartifact_path=${artifact.path}\n`
    ))
  }
  return artifact.path
})

const buildOxlintBinding = Effect.fnUntraced(function*(
  repositoryRoot: string,
  version: string,
  targetName: OxlintBuildTarget
) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const oxlintSource = path.join(
    repositoryRoot,
    "oxlint",
    "crates",
    "oxc_linter",
    "src",
    "rules",
    "effecttsgo",
    "floating_effect.rs"
  )
  if (!(yield* fs.exists(oxlintSource))) {
    return yield* new BuildError({ reason: `Missing generated source ${oxlintSource}; run profile codegen first` })
  }

  const upstream = yield* readUpstream(repositoryRoot)
  const component = upstream.components.oxlint[version]
  if (component === undefined) {
    return yield* new BuildError({ reason: `Unknown oxlint component version ${version}` })
  }
  const gitHead = (yield* runCommandString("git", repositoryRoot, ["-C", "oxlint", "rev-parse", "HEAD"])).trim()
  if (gitHead !== component.gitHead) {
    return yield* new BuildError({ reason: `oxlint ${version} expects checkout ${component.gitHead}, found ${gitHead}` })
  }
  const target = oxlintBuildTargets[targetName]
  const artifact = componentArtifact(repositoryRoot, targetName, "oxlint", version, oxlintBindingName(targetName))
  const bindingSourceName = oxlintBindingName(targetName)
  const oxlint = path.join(repositoryRoot, "oxlint")
  const oxlintApp = path.join(oxlint, "apps", "oxlint")
  yield* fs.makeDirectory(path.dirname(artifact.path), { recursive: true })
  yield* Console.log(`Building Oxlint N-API addon for ${targetName}`)
  yield* runCommand("corepack", oxlint, ["pnpm", "install", "--frozen-lockfile"])
  yield* runCommand("corepack", oxlintApp, [
    "pnpm",
    "run",
    "build-napi-release",
    "--target",
    target.rustTarget,
    ...target.napiArgs
  ])
  const bindingSource = path.join(oxlintApp, "src-js", bindingSourceName)
  yield* validateArtifact(bindingSource)
  yield* fs.copyFile(bindingSource, artifact.path)
  yield* validateArtifact(artifact.path)
  yield* runCommand("corepack", oxlintApp, ["pnpm", "run", "build-js"])
  yield* validateArtifact(path.join(oxlintApp, "dist", "cli.js"))

  if (process.env.GITHUB_OUTPUT !== undefined) {
    yield* Effect.tryPromise(() => appendFile(
      process.env.GITHUB_OUTPUT!,
      `component=oxlint\nnpm_version=${version}\nartifact_path=${artifact.path}\n`
    ))
  }
  return artifact.path
})

const buildTsc = Effect.fnUntraced(function*(
  repositoryRoot: string,
  version: string,
  targetName: BuildTarget
) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  yield* ensureEffectFixtures(repositoryRoot)
  const upstream = yield* readUpstream(repositoryRoot)
  const component = upstream.components.typescript[version]
  if (component === undefined) {
    return yield* new BuildError({ reason: `Unknown TypeScript component version ${version}` })
  }
  const compiler = yield* resolveMaterializedTypeScriptSource(repositoryRoot)
  const checkoutRevision = (yield* runCommandString("git", repositoryRoot, [
    "-C",
    compiler.checkoutDir,
    "rev-parse",
    "HEAD"
  ])).trim()
  if (checkoutRevision !== component.gitHead) {
    return yield* new BuildError({
      reason: `TypeScript ${version} expects ${compiler.repositorySlug} ${component.gitHead}, found ${checkoutRevision}`
    })
  }

  const target = buildTargets[targetName]
  const artifact = componentArtifact(repositoryRoot, targetName, "typescript", version, "tsc")
  yield* fs.makeDirectory(path.dirname(artifact.path), { recursive: true })
  yield* Console.log(`Building TypeScript ${version} for ${targetName} -> ${artifact.path}`)
  yield* runCommand("go", repositoryRoot, [
    "build",
    "-tags=noembed",
    "-ldflags=-s -w",
    "-o",
    artifact.path,
    typeScriptSourceEntrypoint(compiler)
  ], false, {
    CGO_ENABLED: "0",
    GOOS: target.goos,
    GOARCH: target.goarch,
    GOARM: "goarm" in target ? target.goarm : ""
  })
  yield* validateArtifact(artifact.path)
  yield* copyTypeScriptLibraries(
    path.join(repositoryRoot, compiler.checkoutDir, compiler.moduleDir, "internal", "bundled", "libs"),
    path.dirname(artifact.path)
  )

  if (process.env.GITHUB_OUTPUT !== undefined) {
    yield* Effect.tryPromise(() => appendFile(
      process.env.GITHUB_OUTPUT!,
      `component=typescript\nnpm_version=${version}\nartifact_path=${path.dirname(artifact.path)}\n`
    ))
  }
  return artifact
})

export const buildArtifact = Effect.fnUntraced(function*(
  repositoryRoot: string,
  component: ComponentName,
  version: string,
  target: BuildTarget
) {
  if (component === "typescript") {
    return yield* buildTsc(repositoryRoot, version, target)
  }
  if (component === "tsrs") {
    if (!isTsrsBuildTarget(target)) {
      return yield* new BuildError({ reason: `tsrs does not support target ${target}` })
    }
    return yield* buildTsrs(repositoryRoot, version, target)
  }
  if (!(target in oxlintBuildTargets)) {
    return yield* new BuildError({ reason: `${component} does not support target ${target}` })
  }
  const oxlintTarget = target as OxlintBuildTarget
  if (component === "oxlint-tsgolint") {
    return yield* buildTsgolint(repositoryRoot, version, oxlintTarget)
  }
  return yield* buildOxlintBinding(repositoryRoot, version, oxlintTarget)
})

export const verifyReleaseArtifacts = Effect.fnUntraced(function*(repositoryRoot: string) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const upstream = yield* readUpstream(repositoryRoot)
  const sourceUpstream = yield* fs.readFileString(path.join(repositoryRoot, "_packages", "tsgo", "upstream.json"))
  for (const version of Object.keys(upstream.components.typescript)) {
    for (const target of Object.keys(buildTargets) as Array<BuildTarget>) {
      const binary = packagedTypeScriptCompiler(repositoryRoot, target, upstream, version)
      yield* validateArtifact(binary)
      yield* validateArtifact(join(dirname(binary), "lib.d.ts"))
      const libraries = (yield* fs.readDirectory(dirname(binary))).filter((name) =>
        /^lib(?:\..+)?\.d\.ts$/.test(name))
      for (const library of libraries) {
        yield* validateArtifact(join(dirname(binary), library))
      }
    }
  }
  for (const target of Object.keys(buildTargets) as Array<BuildTarget>) {
    const packagedUpstream = path.join(repositoryRoot, "_packages", `tsgo-${target}`, "lib", "upstream.json")
    const text = yield* fs.readFileString(packagedUpstream).pipe(
      Effect.mapError((error) => new BuildError({ reason: `Missing platform metadata ${packagedUpstream}: ${error.message}` }))
    )
    if (text !== sourceUpstream) {
      return yield* new BuildError({ reason: `Platform metadata does not match source: ${packagedUpstream}` })
    }
  }
  for (const target of Object.keys(oxlintBuildTargets) as Array<OxlintBuildTarget>) {
    for (const version of Object.keys(upstream.components["oxlint-tsgolint"])) {
      const artifact = componentArtifact(repositoryRoot, target, "oxlint-tsgolint", version, "tsgolint").path
      if (target.startsWith("win32-")) {
        yield* validateArtifact(artifact)
      } else {
        yield* validateExecutableArtifact(artifact)
      }
    }
    for (const version of Object.keys(upstream.components.oxlint)) {
      yield* validateArtifact(
        componentArtifact(repositoryRoot, target, "oxlint", version, oxlintBindingName(target)).path
      )
    }
  }
  for (const target of Object.keys(buildTargets) as Array<BuildTarget>) {
    const artifactDirectory = path.join(repositoryRoot, "_packages", `tsgo-${target}`, "artifacts", "tsrs")
    if (isTsrsBuildTarget(target)) {
      for (const version of Object.keys(upstream.components.tsrs ?? {})) {
        const binary = componentArtifact(repositoryRoot, target, "tsrs", version, tsrsBinaryName).path
        yield* validateExecutableArtifact(binary)
        yield* validateArtifact(join(dirname(binary), "lib.d.ts"))
        for (const library of (yield* fs.readDirectory(dirname(binary))).filter((name) => name.endsWith(".d.ts"))) {
          yield* validateArtifact(join(dirname(binary), library))
        }
      }
    } else if (yield* fs.exists(artifactDirectory)) {
      return yield* new BuildError({ reason: `tsrs is not built for ${target}, found ${artifactDirectory}` })
    }
  }
  for (const target of Object.keys(buildTargets) as Array<BuildTarget>) {
    const duplicate = dirname(componentArtifact(repositoryRoot, target, "typescript", upstream.tags.typescript.latest, "tsc").path)
    if (yield* fs.exists(duplicate)) {
      return yield* new BuildError({ reason: `The latest TypeScript compiler ships only in lib/, found ${duplicate}` })
    }
  }
  yield* Console.log("Verified release artifacts for all components and targets")
})
