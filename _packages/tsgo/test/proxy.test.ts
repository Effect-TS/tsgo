import * as NodeServices from "@effect/platform-node/NodeServices"
import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"
import * as Path from "effect/Path"
import { spawnSync } from "node:child_process"
import { EventEmitter } from "node:events"
import { chmod, mkdir, mkdtemp, rm, stat, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { dirname, join } from "node:path"
import { fileURLToPath } from "node:url"
import { afterEach, describe, expect, it, vi } from "vitest"
import * as pkgJson from "../package.json" with { type: "json" }
import upstreamJson from "../upstream.json" with { type: "json" }
import { parseProxyInvocation } from "../src/cli/proxy/argv.js"
import { type ExecHost, execProxyCommand, forwardedSignals, type ProxyCommand } from "../src/cli/proxy/exec.js"
import {
  executablePath,
  findOxlintInstallation,
  oxlintBridgeEnv,
  oxlintCommand,
  preloadPath,
  type ProxyContext,
  resolveProxyCommand,
  selectVersion,
  tscExecutable,
  tsrsExecutable
} from "../src/cli/proxy/resolve.js"

const temporaryDirectories: Array<string> = []

const run = <A, E>(effect: Effect.Effect<A, E, FileSystem.FileSystem | Path.Path>) =>
  Effect.runPromise(effect.pipe(Effect.provide(NodeServices.layer)))

const makeTemporaryDirectory = async () => {
  const directory = await mkdtemp(join(tmpdir(), "effect-tsgo-proxy-"))
  temporaryDirectories.push(directory)
  return directory
}

const writePackage = async (directory: string, packageName: string, packageJson: Record<string, unknown>) => {
  const packageDirectory = join(directory, "node_modules", ...packageName.split("/"))
  await mkdir(packageDirectory, { recursive: true })
  await writeFile(join(packageDirectory, "package.json"), JSON.stringify({ name: packageName, ...packageJson }))
  return packageDirectory
}

const writeArtifact = async (filePath: string, mode = 0o755) => {
  await mkdir(dirname(filePath), { recursive: true })
  await writeFile(filePath, "artifact")
  await chmod(filePath, mode)
  return filePath
}

const makeContext = async (overrides: Partial<ProxyContext> = {}): Promise<ProxyContext> => ({
  cwd: await makeTemporaryDirectory(),
  env: { PATH: "/usr/bin" },
  execPath: "/usr/bin/node",
  platform: "linux",
  arch: "x64",
  isGlibc: () => true,
  packageDirectory: await makeTemporaryDirectory(),
  oxlintBridge: "/pkg/lib/oxlint-bridge.cjs",
  ...overrides
})

const bridgeEnv = (preload: string, artifacts: { readonly binding: string; readonly tsgolint?: string }) => ({
  NODE_OPTIONS: `--require "${preload}"`,
  BUN_OPTIONS: `"--preload=${preload}"`,
  EFFECT_TSGO_OXLINT_BINDING: artifacts.binding,
  ...(artifacts.tsgolint === undefined ? {} : { EFFECT_TSGO_TSGOLINT_PATH: artifacts.tsgolint })
})

const typescriptTags = upstreamJson.tags.typescript
const typescriptVersions = Object.keys(upstreamJson.components.typescript).sort()
const olderTypescript = typescriptVersions.find((version) => version !== typescriptTags.latest)!
const tsrsVersion = upstreamJson.tags.tsrs.latest
const oxlintVersion = upstreamJson.tags.oxlint.latest
const tsgolintVersion = upstreamJson.tags["oxlint-tsgolint"].latest

afterEach(async () => {
  vi.restoreAllMocks()
  await Promise.all(temporaryDirectories.splice(0).map((directory) => rm(directory, { recursive: true, force: true })))
})

describe("proxy argv dispatch", () => {
  it("recognizes the proxied tools with an optional version or tag", () => {
    expect(parseProxyInvocation(["tsc"])).toEqual({ tool: "tsc", version: undefined, args: [] })
    expect(parseProxyInvocation(["tsc@7.0.2", "-p", "tsconfig.json"])).toEqual({
      tool: "tsc",
      version: "7.0.2",
      args: ["-p", "tsconfig.json"]
    })
    expect(parseProxyInvocation(["tsc@next"])).toEqual({ tool: "tsc", version: "next", args: [] })
    expect(parseProxyInvocation(["tsrs@latest", "--noEmit"])).toEqual({
      tool: "tsrs",
      version: "latest",
      args: ["--noEmit"]
    })
    expect(parseProxyInvocation(["oxlint", "--type-aware"])).toEqual({
      tool: "oxlint",
      version: undefined,
      args: ["--type-aware"]
    })
    expect(parseProxyInvocation(["vp", "check", "--fix"])).toEqual({ tool: "vp", version: undefined, args: ["check", "--fix"] })
    expect(parseProxyInvocation(["tsc@"])).toEqual({ tool: "tsc", version: "", args: [] })
  })

  it("passes every argument after the tool through verbatim, including effect-tsgo's own flags", () => {
    const args = ["--version", "-p", "tsconfig.json", "--lsp", "--stdio", "--help", "--build", "tsc", "patch", ""]
    expect(parseProxyInvocation(["tsc", ...args])?.args).toEqual(args)
    expect(parseProxyInvocation(["oxlint", ...args])?.args).toEqual(args)
  })

  it("leaves subcommands and global flags to the Effect CLI", () => {
    expect(parseProxyInvocation([])).toBeUndefined()
    expect(parseProxyInvocation(["--version"])).toBeUndefined()
    expect(parseProxyInvocation(["--help", "tsc"])).toBeUndefined()
    expect(parseProxyInvocation(["patch", "--tsrs"])).toBeUndefined()
    expect(parseProxyInvocation(["get-exe-path"])).toBeUndefined()
    expect(parseProxyInvocation(["tscx"])).toBeUndefined()
    expect(parseProxyInvocation(["@tsc"])).toBeUndefined()
    expect(parseProxyInvocation(["TSC"])).toBeUndefined()
  })
})

describe("proxy version selection", () => {
  it("prefers a requested version or tag", async () => {
    expect(await run(selectVersion("tsc", "typescript", olderTypescript, typescriptTags.latest))).toBe(olderTypescript)
    expect(await run(selectVersion("tsc", "typescript", "beta", undefined))).toBe(typescriptTags.beta)
    expect(await run(selectVersion("tsc", "typescript", "next", undefined))).toBe(typescriptTags.next)
    expect(await run(selectVersion("tsc", "typescript", "latest", olderTypescript))).toBe(typescriptTags.latest)
    expect(await run(selectVersion("tsrs", "tsrs", "latest", undefined))).toBe(tsrsVersion)
  })

  it("uses the installed version when a build of it ships, else the latest one", async () => {
    expect(await run(selectVersion("tsc", "typescript", undefined, olderTypescript))).toBe(olderTypescript)
    expect(await run(selectVersion("tsc", "typescript", undefined, "7.9.9"))).toBe(typescriptTags.latest)
    expect(await run(selectVersion("tsc", "typescript", undefined, undefined))).toBe(typescriptTags.latest)
    expect(await run(selectVersion("tsrs", "tsrs", undefined, "0.0.1"))).toBe(tsrsVersion)
  })

  it("rejects an unknown requested version with the supported versions and tags", async () => {
    await expect(run(selectVersion("tsc", "typescript", "7.9.9", undefined))).rejects.toMatchObject({
      _tag: "UnsupportedProxyVersionError",
      message: `tsc@7.9.9 is not supported by @effect/tsgo version ${pkgJson.version}. `
        + `Supported versions: ${typescriptVersions.join(", ")}; tags: `
        + `latest (${typescriptTags.latest}), beta (${typescriptTags.beta}), next (${typescriptTags.next}).`
    })
    await expect(run(selectVersion("tsrs", "tsrs", "beta", undefined))).rejects.toMatchObject({
      _tag: "UnsupportedProxyVersionError",
      packageName: "tsrs",
      version: "beta"
    })
    await expect(run(selectVersion("tsc", "typescript", "", undefined))).rejects.toMatchObject({
      _tag: "ProxyError",
      reason: "Expected a version or tag after tsc@."
    })
  })
})

describe("proxy executable paths", () => {
  it("runs the latest TypeScript from lib and other versions from artifacts", async () => {
    const context = await makeContext({ packageDirectory: "/platform" })
    expect(await run(tscExecutable(context, typescriptTags.latest))).toBe(join("/platform", "lib", "tsc"))
    expect(await run(tscExecutable(context, olderTypescript))).toBe(
      join("/platform", "artifacts", "typescript", olderTypescript, "tsc")
    )
    expect(await run(tscExecutable({ ...context, platform: "win32" }, olderTypescript))).toBe(
      join("/platform", "artifacts", "typescript", olderTypescript, "tsc.exe")
    )
    expect(await run(tsrsExecutable(context, tsrsVersion))).toBe(join("/platform", "artifacts", "tsrs", tsrsVersion, "tsrs"))
    expect(await run(tsrsExecutable({ ...context, platform: "win32" }, tsrsVersion))).toBe(
      join("/platform", "artifacts", "tsrs", tsrsVersion, "tsrs.exe")
    )
  })

  it("prefixes long Windows paths", () => {
    const long = `C:\\${"a".repeat(250)}\\tsc.exe`
    expect(executablePath(long, "win32")).toBe(`\\\\?\\${long}`)
    expect(executablePath(`\\\\?\\${long}`, "win32")).toBe(`\\\\?\\${long}`)
    const unc = `\\\\server\\share\\${"a".repeat(250)}\\tsc.exe`
    expect(executablePath(unc, "win32")).toBe(`\\\\?\\UNC\\server\\share\\${"a".repeat(250)}\\tsc.exe`)
    expect(executablePath("C:\\short\\tsc.exe", "win32")).toBe("C:\\short\\tsc.exe")
    expect(executablePath(`/${"a".repeat(250)}/tsc`, "linux")).toBe(`/${"a".repeat(250)}/tsc`)
  })
})

describe("proxy tsc and tsrs resolution", () => {
  it("runs the build of the installed native TypeScript with the arguments and environment", async () => {
    const context = await makeContext()
    await writePackage(context.cwd, "typescript", { version: olderTypescript })
    const executable = await writeArtifact(join(context.packageDirectory, "artifacts", "typescript", olderTypescript, "tsc"))
    expect(await run(resolveProxyCommand({ tool: "tsc", version: undefined, args: ["-p", "."] }, context))).toEqual({
      file: executable,
      args: ["-p", "."],
      env: context.env
    })
  })

  it("ignores a non-native TypeScript install and falls back to the latest build", async () => {
    const context = await makeContext()
    await writePackage(context.cwd, "typescript", { version: "5.9.2" })
    await writePackage(context.cwd, "@typescript/native", { version: "7.9.9" })
    const executable = await writeArtifact(join(context.packageDirectory, "lib", "tsc"))
    expect((await run(resolveProxyCommand({ tool: "tsc", version: undefined, args: [] }, context))).file).toBe(executable)
  })

  it("reports a supported version without its packaged artifact", async () => {
    const context = await makeContext()
    await expect(run(resolveProxyCommand({ tool: "tsc", version: "latest", args: [] }, context))).rejects.toMatchObject({
      _tag: "ProxyError",
      reason: `Missing packaged artifact ${join(context.packageDirectory, "lib", "tsc")}.`
    })
  })

  it("marks a packaged binary executable only when it is not", async () => {
    const context = await makeContext()
    const executable = await writeArtifact(join(context.packageDirectory, "lib", "tsc"), 0o644)
    await run(resolveProxyCommand({ tool: "tsc", version: undefined, args: [] }, context))
    expect((await stat(executable)).mode & 0o777).toBe(0o755)

    await chmod(executable, 0o500)
    await run(resolveProxyCommand({ tool: "tsc", version: undefined, args: [] }, context))
    expect((await stat(executable)).mode & 0o777).toBe(0o500)
  })

  // root may execute any file with an execute bit, so the owner-denied case only applies to other users.
  it.skipIf(process.platform === "win32" || process.getuid?.() === 0)(
    "marks a packaged binary executable when only other users may execute it",
    async () => {
      const context = await makeContext()
      const executable = await writeArtifact(join(context.packageDirectory, "lib", "tsc"), 0o654)
      await run(resolveProxyCommand({ tool: "tsc", version: undefined, args: [] }, context))
      expect((await stat(executable)).mode & 0o777).toBe(0o755)
    }
  )

  it("runs the build of the installed tsrs version, which needs lib.d.ts next to it", async () => {
    const context = await makeContext()
    await writePackage(context.cwd, "tsrs", { version: tsrsVersion })
    const executable = await writeArtifact(join(context.packageDirectory, "artifacts", "tsrs", tsrsVersion, "tsrs"))
    const invocation = { tool: "tsrs", version: undefined, args: ["--noEmit"] } as const
    await expect(run(resolveProxyCommand(invocation, context))).rejects.toMatchObject({
      _tag: "ProxyError",
      reason: expect.stringContaining(`Missing ${join(dirname(executable), "lib.d.ts")}: tsrs needs its lib*.d.ts files`)
    })
    await writeFile(join(dirname(executable), "lib.d.ts"), "lib")
    expect(await run(resolveProxyCommand(invocation, context))).toEqual({
      file: executable,
      args: ["--noEmit"],
      env: context.env
    })
  })

  it("rejects tsrs on hosts without a packaged build", async () => {
    const musl = await makeContext({ isGlibc: () => false })
    await expect(run(resolveProxyCommand({ tool: "tsrs", version: undefined, args: [] }, musl))).rejects.toMatchObject({
      reason: "Linux musl is not supported by the packaged tsrs integration; it is built for "
        + "darwin-arm64, darwin-x64, linux-x64-gnu, linux-arm64-gnu, win32-x64."
    })
    const windows = await makeContext({ platform: "win32", arch: "arm64" })
    await expect(run(resolveProxyCommand({ tool: "tsrs", version: undefined, args: [] }, windows))).rejects.toMatchObject({
      reason: "The packaged tsrs integration is not available for win32-arm64; it is built for "
        + "darwin-arm64, darwin-x64, linux-x64-gnu, linux-arm64-gnu, win32-x64."
    })
  })
})

describe("proxy oxlint resolution", () => {
  const installOxlint = async (directory: string, options: { readonly tsgolint?: boolean } = {}) => {
    const oxlint = await writePackage(directory, "oxlint", { version: oxlintVersion, bin: { oxlint: "bin/oxlint" } })
    if (options.tsgolint) await writePackage(directory, "oxlint-tsgolint", { version: tsgolintVersion })
    return join(oxlint, "bin", "oxlint")
  }

  it("finds oxlint installed directly or through vite-plus", async () => {
    const direct = await makeTemporaryDirectory()
    await installOxlint(direct, { tsgolint: true })
    const found = await run(findOxlintInstallation(direct))
    expect(found?.oxlint.version).toBe(oxlintVersion)
    expect(found?.tsgolint?.version).toBe(tsgolintVersion)

    const nested = await makeTemporaryDirectory()
    const vitePlus = await writePackage(nested, "vite-plus", { version: "1.4.0" })
    await installOxlint(vitePlus)
    const throughVitePlus = await run(findOxlintInstallation(nested))
    expect(throughVitePlus?.oxlint.packageJsonPath).toBe(join(vitePlus, "node_modules", "oxlint", "package.json"))
    expect(throughVitePlus?.tsgolint).toBeUndefined()

    expect(await run(findOxlintInstallation(await makeTemporaryDirectory()))).toBeUndefined()
  })

  it("runs the installed Oxlint CLI with the packaged binding and tsgolint", async () => {
    const context = await makeContext()
    const bin = await installOxlint(context.cwd, { tsgolint: true })
    const binding = await writeArtifact(
      join(context.packageDirectory, "artifacts", "oxlint", oxlintVersion, "oxlint.linux-x64-gnu.node"),
      0o644
    )
    const tsgolint = await writeArtifact(
      join(context.packageDirectory, "artifacts", "oxlint-tsgolint", tsgolintVersion, "tsgolint"),
      0o644
    )
    const command = await run(resolveProxyCommand({ tool: "oxlint", version: undefined, args: ["--type-aware"] }, context))
    expect(command).toEqual({
      file: "/usr/bin/node",
      args: [bin, "--type-aware"],
      env: { PATH: "/usr/bin", ...bridgeEnv("/pkg/lib/oxlint-bridge.cjs", { binding, tsgolint }) }
    })
    expect((await stat(tsgolint)).mode & 0o777).toBe(0o755)
  })

  it("leaves the tsgolint variables alone without oxlint-tsgolint and preloads Windows paths with forward slashes", async () => {
    const context = await makeContext({
      platform: "win32",
      arch: "arm64",
      env: { OXLINT_TSGOLINT_PATH: "custom" },
      oxlintBridge: "C:\\Program Files\\pkg\\lib\\oxlint-bridge.cjs"
    })
    const oxlint = await writePackage(context.cwd, "oxlint", { version: oxlintVersion, bin: "bin/cli.js" })
    const binding = await writeArtifact(
      join(context.packageDirectory, "artifacts", "oxlint", oxlintVersion, "oxlint.win32-arm64-msvc.node")
    )
    const installation = (await run(findOxlintInstallation(context.cwd)))!
    expect(await run(oxlintCommand(context, [], installation))).toEqual({
      file: "/usr/bin/node",
      args: [join(oxlint, "bin", "cli.js")],
      env: { OXLINT_TSGOLINT_PATH: "custom", ...bridgeEnv("C:/Program Files/pkg/lib/oxlint-bridge.cjs", { binding }) }
    })
  })

  it("rejects unsupported installs, versions and hosts", async () => {
    const context = await makeContext()
    await writePackage(context.cwd, "oxlint", { version: "1.0.0", bin: { oxlint: "bin/oxlint" } })
    await expect(run(resolveProxyCommand({ tool: "oxlint", version: undefined, args: [] }, context))).rejects
      .toMatchObject({
        _tag: "UnsupportedProxyVersionError",
        message: `Installed oxlint version 1.0.0 is not supported by @effect/tsgo version ${pkgJson.version}. `
          + "Either change @effect/tsgo to a compatible version or set oxlint to one of the supported versions: "
          + `${Object.keys(upstreamJson.components.oxlint).sort().join(", ")}.`
      })
    await expect(run(resolveProxyCommand({ tool: "oxlint", version: oxlintVersion, args: [] }, context))).rejects
      .toMatchObject({ reason: expect.stringContaining(`oxlint@${oxlintVersion} is not supported`) })
    await expect(run(resolveProxyCommand({ tool: "oxlint", version: undefined, args: [] }, {
      ...context,
      isGlibc: () => false
    }))).rejects.toMatchObject({ reason: "Linux musl is not supported by the packaged Oxlint integration." })
    const empty = await makeContext()
    await expect(run(resolveProxyCommand({ tool: "oxlint", version: undefined, args: [] }, empty))).rejects
      .toMatchObject({ reason: `Unable to resolve oxlint from ${empty.cwd}. Install oxlint (or vite-plus) in the project.` })
  })
})

describe("proxy oxlint bridge environment", () => {
  const artifacts = { binding: "/pkg/oxlint.node", tsgolint: "/pkg/tsgolint" }

  it("puts the bridge in front of the caller's NODE_OPTIONS and BUN_OPTIONS, once", async () => {
    const context = await makeContext({
      env: { NODE_OPTIONS: "--max-old-space-size=4096", BUN_OPTIONS: "--smol" }
    })
    const env = await run(oxlintBridgeEnv(context, artifacts))
    expect(env.NODE_OPTIONS).toBe(`--require "/pkg/lib/oxlint-bridge.cjs" --max-old-space-size=4096`)
    expect(env.BUN_OPTIONS).toBe(`"--preload=/pkg/lib/oxlint-bridge.cjs" --smol`)
    expect(env.EFFECT_TSGO_OXLINT_BINDING).toBe("/pkg/oxlint.node")
    expect(env.EFFECT_TSGO_TSGOLINT_PATH).toBe("/pkg/tsgolint")

    const nested = await run(oxlintBridgeEnv({ ...context, env }, artifacts))
    expect(nested.NODE_OPTIONS).toBe(env.NODE_OPTIONS)
    expect(nested.BUN_OPTIONS).toBe(env.BUN_OPTIONS)
  })

  it("rejects a bridge path that the options cannot carry", async () => {
    const context = await makeContext({ oxlintBridge: `/odd"dir/lib/oxlint-bridge.cjs` })
    await expect(run(oxlintBridgeEnv(context, artifacts))).rejects.toMatchObject({
      reason: `Unable to preload /odd"dir/lib/oxlint-bridge.cjs: NODE_OPTIONS and BUN_OPTIONS cannot carry a path with quotes or backslashes.`
    })
    expect(await run(preloadPath("C:\\pkg\\lib\\oxlint-bridge.cjs", "win32"))).toBe("C:/pkg/lib/oxlint-bridge.cjs")
  })
})

describe("proxy oxlint bridge", () => {
  const bridge = fileURLToPath(new URL("../lib/oxlint-bridge.cjs", import.meta.url))

  // Records the filename each process.dlopen call reaches after the bridge, in a fresh Node.js process.
  const loadThroughBridge = (filenames: ReadonlyArray<string>, env: NodeJS.ProcessEnv) => {
    const script = `
      const seen = [];
      process.dlopen = (module, filename, ...rest) => seen.push({ filename, rest: rest.length });
      require(${JSON.stringify(bridge)});
      for (const filename of ${JSON.stringify(filenames)}) process.dlopen({ exports: {} }, filename);
      process.dlopen({ exports: {} }, ${JSON.stringify(filenames[0])}, 1);
      process.stdout.write(JSON.stringify(seen));
    `
    const result = spawnSync(process.execPath, ["-e", script], { encoding: "utf8", env: { ...env, NODE_OPTIONS: "" } })
    expect(result.status, result.stderr).toBe(0)
    return JSON.parse(result.stdout) as ReadonlyArray<{ readonly filename: string; readonly rest: number }>
  }

  it("maps only the stock Oxlint binding to the patched one and forwards the flags it is given", () => {
    const stock = "/project/node_modules/@oxlint/binding-linux-x64-gnu/oxlint.linux-x64-gnu.node"
    const windowsStock = "\\\\?\\C:\\project\\node_modules\\@oxlint\\binding-win32-x64-msvc\\oxlint.win32-x64-msvc.node"
    const other = "/project/node_modules/@voidzero-dev/vite-plus-linux-x64-gnu/vite-plus.linux-x64-gnu.node"
    const seen = loadThroughBridge([stock, windowsStock, other], { EFFECT_TSGO_OXLINT_BINDING: "/pkg/oxlint.node" })
    expect(seen).toEqual([
      { filename: "/pkg/oxlint.node", rest: 0 },
      { filename: "/pkg/oxlint.node", rest: 0 },
      { filename: other, rest: 0 },
      { filename: "/pkg/oxlint.node", rest: 1 }
    ])
  })

  it("does nothing without a patched binding", () => {
    const stock = "/project/node_modules/@oxlint/binding-linux-x64-gnu/oxlint.linux-x64-gnu.node"
    expect(loadThroughBridge([stock], {}).map(({ filename }) => filename)).toEqual([stock, stock])
  })
})

describe("proxy vite-plus resolution", () => {
  const installVitePlus = async (directory: string) => {
    const vitePlus = await writePackage(directory, "vite-plus", { version: "1.4.0", bin: { vp: "./bin/vp" } })
    await writePackage(vitePlus, "oxlint", { version: oxlintVersion, bin: { oxlint: "bin/oxlint" } })
    await writePackage(vitePlus, "oxlint-tsgolint", { version: tsgolintVersion })
    return join(vitePlus, "bin", "vp")
  }

  it("runs the vp CLI with the patched binding and tsgolint of its oxlint", async () => {
    const context = await makeContext({ execPath: "/usr/bin/bun" })
    const vp = await installVitePlus(context.cwd)
    const binding = await writeArtifact(
      join(context.packageDirectory, "artifacts", "oxlint", oxlintVersion, "oxlint.linux-x64-gnu.node"),
      0o644
    )
    const tsgolint = await writeArtifact(
      join(context.packageDirectory, "artifacts", "oxlint-tsgolint", tsgolintVersion, "tsgolint")
    )
    const command = await run(resolveProxyCommand({ tool: "vp", version: undefined, args: ["check", "--fix"] }, context))
    expect(command).toEqual({
      file: "/usr/bin/bun",
      args: [vp, "check", "--fix"],
      env: { PATH: "/usr/bin", ...bridgeEnv("/pkg/lib/oxlint-bridge.cjs", { binding, tsgolint }) }
    })
  })

  it("rejects missing installs, versions and musl", async () => {
    const empty = await makeContext()
    await expect(run(resolveProxyCommand({ tool: "vp", version: undefined, args: [] }, empty))).rejects
      .toMatchObject({ reason: `Unable to resolve vite-plus from ${empty.cwd}. Install vite-plus in the project.` })
    const withoutOxlint = await makeContext()
    await writePackage(withoutOxlint.cwd, "vite-plus", { version: "1.4.0", bin: { vp: "./bin/vp" } })
    await expect(run(resolveProxyCommand({ tool: "vp", version: undefined, args: [] }, withoutOxlint))).rejects
      .toMatchObject({ reason: `Unable to resolve the oxlint package of vite-plus from ${withoutOxlint.cwd}.` })
    await expect(run(resolveProxyCommand({ tool: "vp", version: "1.4.0", args: [] }, withoutOxlint))).rejects
      .toMatchObject({ reason: expect.stringContaining("vp@1.4.0 is not supported") })
    await expect(run(resolveProxyCommand({ tool: "vp", version: undefined, args: [] }, {
      ...withoutOxlint,
      isGlibc: () => false
    }))).rejects.toMatchObject({ reason: "Linux musl is not supported by the packaged Oxlint integration." })
  })
})

class FakeChild extends EventEmitter {
  readonly kill = vi.fn((_signal: NodeJS.Signals) => true)
}

class FakeParent extends EventEmitter {
  readonly pid = 4242
  exitCode: number | string | null | undefined = undefined
  readonly kill = vi.fn((_pid: number, _signal: NodeJS.Signals) => true)
}

const makeHost = (overrides: Partial<ExecHost> = {}) => {
  const child = new FakeChild()
  const parent = new FakeParent()
  const errors: Array<string> = []
  const host: ExecHost = {
    platform: "linux",
    execve: undefined,
    spawn: vi.fn(() => child),
    parent,
    reportError: (message) => errors.push(message),
    ...overrides
  }
  return { child, parent, errors, host }
}

const command: ProxyCommand = { file: "/platform/lib/tsc", args: ["-p", "."], env: { FOO: "bar" } }

describe("proxy exec", () => {
  it("replaces the process with execve when available", () => {
    const execve = vi.fn(() => {
      throw new Error("replaced")
    })
    const { host } = makeHost({ execve: execve as unknown as ExecHost["execve"] })
    execProxyCommand(command, host)
    expect(execve).toHaveBeenCalledWith("/platform/lib/tsc", ["/platform/lib/tsc", "-p", "."], { FOO: "bar" })
    // execve threw, so the tool runs as a child instead.
    expect(host.spawn).toHaveBeenCalledWith("/platform/lib/tsc", ["-p", "."], { stdio: "inherit", env: { FOO: "bar" } })
  })

  it("never uses execve on Windows", () => {
    const execve = vi.fn()
    const { host } = makeHost({ platform: "win32", execve: execve as unknown as ExecHost["execve"] })
    execProxyCommand(command, host)
    expect(execve).not.toHaveBeenCalled()
    expect(host.spawn).toHaveBeenCalled()
  })

  it("propagates the exit code and stops forwarding signals", () => {
    const { child, host, parent } = makeHost()
    execProxyCommand(command, host)
    for (const signal of forwardedSignals("linux")) expect(parent.listenerCount(signal)).toBe(1)
    parent.emit("SIGINT", "SIGINT")
    parent.emit("SIGTERM", "SIGTERM")
    expect(child.kill.mock.calls).toEqual([["SIGINT"], ["SIGTERM"]])
    child.emit("exit", 3, null)
    expect(parent.exitCode).toBe(3)
    expect(parent.kill).not.toHaveBeenCalled()
    for (const signal of forwardedSignals("linux")) expect(parent.listenerCount(signal)).toBe(0)
  })

  it("re-raises the signal that terminated the child", () => {
    const { child, host, parent } = makeHost()
    execProxyCommand(command, host)
    child.emit("exit", null, "SIGTERM")
    expect(parent.kill).toHaveBeenCalledWith(4242, "SIGTERM")
    expect(parent.listenerCount("SIGTERM")).toBe(0)
  })

  it("reports spawn errors once and exits with 1", () => {
    const { child, errors, host, parent } = makeHost()
    execProxyCommand(command, host)
    child.emit("error", new Error("spawn EACCES"))
    child.emit("exit", -13, null)
    expect(errors).toEqual(["Failed to run /platform/lib/tsc: spawn EACCES"])
    expect(parent.exitCode).toBe(1)
    expect(parent.listenerCount("SIGINT")).toBe(0)

    const thrown = makeHost({
      spawn: () => {
        throw new Error("spawn EINVAL")
      }
    })
    execProxyCommand(command, thrown.host)
    expect(thrown.errors).toEqual(["Failed to run /platform/lib/tsc: spawn EINVAL"])
    expect(thrown.parent.exitCode).toBe(1)
    expect(thrown.parent.listenerCount("SIGINT")).toBe(0)
  })

  it("forwards the signals each platform can listen to", () => {
    expect(forwardedSignals("linux")).toEqual(["SIGINT", "SIGTERM", "SIGHUP", "SIGQUIT"])
    expect(forwardedSignals("win32")).toEqual(["SIGINT", "SIGTERM", "SIGHUP", "SIGBREAK"])
  })
})
