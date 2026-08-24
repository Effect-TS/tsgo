import * as Console from "effect/Console"
import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"
import * as Path from "effect/Path"
import * as ChildProcess from "effect/unstable/process/ChildProcess"
import * as ChildProcessSpawner from "effect/unstable/process/ChildProcessSpawner"
import { runCommand } from "./process.ts"
import { type ComponentName, getComponent, readUpstream, type TypeScriptSource } from "./upstream.ts"

const repositories = {
  tsgolint: "https://github.com/oxc-project/tsgolint.git",
  oxlint: "https://github.com/oxc-project/oxc.git"
} as const

export class UnexpectedSubmodulePathError extends Data.TaggedError("UnexpectedSubmodulePathError")<{
  readonly name: string
  readonly path: string
}> {
  get message(): string {
    return `Submodule ${this.name} uses unexpected path ${this.path}`
  }
}

export class UnmanagedSubmodulePathError extends Data.TaggedError("UnmanagedSubmodulePathError")<{
  readonly name: string
  readonly path: string
}> {
  get message(): string {
    return `Refusing to replace unmanaged path ${this.path} while setting up submodule ${this.name}`
  }
}

const runGit = (cwd: string, args: ReadonlyArray<string>, quiet = false) =>
  runCommand("git", cwd, args, quiet)

const readGit = Effect.fnUntraced(function*(cwd: string, args: ReadonlyArray<string>) {
  const spawner = yield* ChildProcessSpawner.ChildProcessSpawner
  yield* runGit(cwd, args, true)
  return (yield* spawner.string(ChildProcess.make("git", args, { cwd }))).trim()
})

const checkoutRevision = Effect.fnUntraced(function*(checkout: string, revision: string) {
  const spawner = yield* ChildProcessSpawner.ChildProcessSpawner
  const hasCommit = yield* spawner.exitCode(ChildProcess.make(
    "git",
    ["cat-file", "-e", `${revision}^{commit}`],
    { cwd: checkout, stdout: "ignore", stderr: "ignore" }
  ))
  if (hasCommit !== ChildProcessSpawner.ExitCode(0)) {
    yield* runGit(checkout, ["fetch", "--depth", "1", "origin", revision])
  }
  yield* runGit(checkout, ["checkout", "--detach", revision])
})

export const cloneSubmodules = Effect.fnUntraced(function*(
  repositoryRoot: string,
  componentName: ComponentName,
  version?: string
) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const spawner = yield* ChildProcessSpawner.ChildProcessSpawner
  const upstream = yield* readUpstream(repositoryRoot)
  const selected = yield* getComponent(upstream, componentName, version)
  const compiler = selected.typescript.source
  const required = [
    { name: compiler.checkoutDir, repository: compiler.repository, revision: selected.typescript.gitHead },
    ...(selected.name === "oxlint-tsgolint"
      ? [{ name: "tsgolint", repository: repositories.tsgolint, revision: selected.gitHead }] as const
      : []),
    ...(selected.name === "oxlint"
      ? [{ name: "oxlint", repository: repositories.oxlint, revision: selected.gitHead }] as const
      : [])
  ] as const
  const reused = new Set<string>()
  const configuredUnused: Array<{
    readonly name: "typescript-go" | "typescript" | "tsgolint" | "oxlint"
    readonly hasGitmodulesConfig: boolean
  }> = []

  for (const name of ["typescript-go", "typescript", "tsgolint", "oxlint"] as const) {
    const target = required.find((target) => target.name === name)
    const configuredExitCode = yield* spawner.exitCode(ChildProcess.make(
      "git",
      ["config", "-f", ".gitmodules", "--get", `submodule.${name}.path`],
      { cwd: repositoryRoot, stdout: "ignore", stderr: "ignore" }
    ))
    const configured = configuredExitCode === ChildProcessSpawner.ExitCode(0)
    const configuredPath = configured
      ? yield* readGit(repositoryRoot, ["config", "-f", ".gitmodules", "--get", `submodule.${name}.path`])
      : undefined
    if (configuredPath !== undefined && configuredPath !== name) {
      return yield* new UnexpectedSubmodulePathError({ name, path: configuredPath })
    }
    const trackedExitCode = yield* spawner.exitCode(ChildProcess.make(
      "git",
      ["ls-files", "--error-unmatch", "--", name],
      { cwd: repositoryRoot, stdout: "ignore", stderr: "ignore" }
    ))
    const tracked = trackedExitCode === ChildProcessSpawner.ExitCode(0)
    const localConfigExitCode = yield* spawner.exitCode(ChildProcess.make(
      "git",
      ["config", "--get-regexp", `^submodule\\.${name}\\.`],
      { cwd: repositoryRoot, stdout: "ignore", stderr: "ignore" }
    ))
    const locallyConfigured = localConfigExitCode === ChildProcessSpawner.ExitCode(0)
    const checkout = path.join(repositoryRoot, name)
    const checkoutExists = yield* fs.exists(checkout)
    const initialized = yield* fs.exists(path.join(checkout, ".git"))
    const managed = configured || tracked || locallyConfigured || initialized
    if (checkoutExists && !managed) {
      if (target !== undefined) {
        return yield* new UnmanagedSubmodulePathError({ name, path: checkout })
      }
      continue
    }
    const configuredRepositoryMatches = target !== undefined && configured &&
      (yield* readGit(repositoryRoot, ["config", "-f", ".gitmodules", "--get", `submodule.${name}.url`])) ===
        target.repository
    if (configuredRepositoryMatches && !initialized) {
      yield* runGit(repositoryRoot, ["submodule", "sync", "--", name])
      yield* runGit(repositoryRoot, ["submodule", "update", "--init", "--depth", "1", "--", name])
    }
    const repositoryMatches = configuredRepositoryMatches &&
      (yield* readGit(checkout, ["remote", "get-url", "origin"])) === target.repository
    if (target !== undefined && repositoryMatches) {
      yield* Console.log(`Reusing ${name} for ${target.revision}`)
      yield* runGit(checkout, ["reset", "--hard"])
      yield* runGit(checkout, ["clean", "-fdx"])
      yield* runGit(checkout, ["stash", "clear"])
      yield* checkoutRevision(checkout, target.revision)
      yield* runGit(checkout, ["reset", "--hard", target.revision])
      yield* runGit(checkout, ["clean", "-fdx"])
      reused.add(name)
      continue
    }

    if (configured) {
      yield* runGit(repositoryRoot, ["submodule", "deinit", "--force", "--", name])
    }

    const gitDirectory = yield* readGit(repositoryRoot, [
      "rev-parse",
      "--path-format=absolute",
      "--git-path",
      `modules/${name}`
    ])
    yield* fs.remove(checkout, { recursive: true, force: true })
    yield* fs.remove(gitDirectory, { recursive: true, force: true })

    if (name === "tsgolint" && target === undefined) {
      yield* runCommand("go", repositoryRoot, ["work", "edit", "-dropuse=./tsgolint"])
    }

    if (configured || tracked || locallyConfigured) {
      configuredUnused.push({ name, hasGitmodulesConfig: configured })
    }
  }

  if (configuredUnused.length > 0) {
    yield* runGit(repositoryRoot, [
      "rm",
      "--cached",
      "--force",
      "--ignore-unmatch",
      "--",
      ...configuredUnused.map(({ name }) => name)
    ])
    for (const optional of configuredUnused) {
      if (optional.hasGitmodulesConfig) {
        yield* runGit(repositoryRoot, [
          "config",
          "-f",
          ".gitmodules",
          "--remove-section",
          `submodule.${optional.name}`
        ])
      }
      const localConfigExitCode = yield* spawner.exitCode(ChildProcess.make(
        "git",
        ["config", "--get-regexp", `^submodule\\.${optional.name}\\.`],
        { cwd: repositoryRoot, stdout: "ignore", stderr: "ignore" }
      ))
      if (localConfigExitCode === ChildProcessSpawner.ExitCode(0)) {
        yield* runGit(repositoryRoot, ["config", "--remove-section", `submodule.${optional.name}`])
      }
    }
    yield* runGit(repositoryRoot, ["add", ".gitmodules"])
  }

  for (const target of required) {
    if (reused.has(target.name)) {
      continue
    }

    yield* Console.log(`Cloning ${target.name} ${target.revision} for ${selected.name} ${selected.version}`)
    yield* runGit(repositoryRoot, [
      "submodule",
      "add",
      "--name",
      target.name,
      "--depth",
      "1",
      target.repository,
      target.name
    ])
    yield* checkoutRevision(path.join(repositoryRoot, target.name), target.revision)
  }

  yield* runGit(repositoryRoot, [
    "config",
    "-f",
    ".gitmodules",
    `submodule.${compiler.checkoutDir}.ignore`,
    "dirty"
  ])
  yield* runGit(repositoryRoot, ["add", ".gitmodules", ...required.map((target) => target.name)])
  return selected
})

export const patchSubmodules = Effect.fnUntraced(function*(repositoryRoot: string, compiler: TypeScriptSource) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const targets = [
    { name: compiler.checkoutDir, patches: path.join(repositoryRoot, compiler.patchDir) },
    { name: "tsgolint", patches: path.join(repositoryRoot, "_patches", "tsgolint") },
    { name: "oxlint", patches: path.join(repositoryRoot, "_patches", "oxlint") }
  ] as const

  for (const target of targets) {
    const checkout = path.join(repositoryRoot, target.name)
    if (!(yield* fs.exists(path.join(checkout, ".git"))) || !(yield* fs.exists(target.patches))) {
      continue
    }

    const patches = (yield* fs.readDirectory(target.patches))
      .filter((file) => file.endsWith(".patch"))
      .sort()

    for (const file of patches) {
      const patch = path.join(target.patches, file)
      yield* Console.log(`Applying ${target.name} patch: ${path.relative(repositoryRoot, patch)}`)
      yield* runGit(checkout, ["apply", "--check", patch], true)
      yield* runGit(checkout, ["apply", patch])
    }
  }
})

export const generateSubmoduleArtifacts = Effect.fnUntraced(function*(
  repositoryRoot: string,
  compiler: TypeScriptSource,
  shimOverlayRoots: ReadonlyArray<string> = []
) {
  const path = yield* Path.Path
  const checkout = path.join(repositoryRoot, compiler.checkoutDir)
  const moduleRoot = path.join(checkout, compiler.moduleDir)
  const providerShimOverlay = path.join(repositoryRoot, compiler.shimOverlayDir)
  if (compiler.provider === "typescript-go") {
    yield* runGit(checkout, ["submodule", "sync", "--recursive"])
    yield* runGit(checkout, [
      "submodule",
      "update",
      "--init",
      "--force",
      "--depth",
      "1",
      "_submodules/TypeScript"
    ])
  }
  yield* Console.log("Generating diagnostics")
  yield* runCommand("go", path.join(moduleRoot, "internal", "diagnostics"), [
    "run",
    "generate.go",
    "-diagnostics",
    "./diagnostics_generated.go",
    "-loc",
    "./loc_generated.go",
    "-locdir",
    "./loc"
  ], false, { GOWORK: "off" })
  yield* Console.log("Generating shims")
  yield* runCommand("go", path.join(repositoryRoot, "_tools", "gen_shims"), [
    "run",
    ".",
    "--repository-root",
    repositoryRoot,
    "--source-root",
    moduleRoot,
    "--module-prefix",
    compiler.modulePrefix,
    "--provider-shim-prefix",
    compiler.providerShimPrefix,
    ...[providerShimOverlay, ...shimOverlayRoots].flatMap((root) => ["--extra-shim-root", root])
  ], false, { GOWORK: "off" })
})
