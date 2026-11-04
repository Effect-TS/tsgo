import * as NodeFileSystem from "@effect/platform-node/NodeFileSystem"
import * as NodePath from "@effect/platform-node/NodePath"
import * as nodeModule from "node:module"
import * as Cause from "effect/Cause"
import * as Effect from "effect/Effect"
import * as Exit from "effect/Exit"
import * as Layer from "effect/Layer"
import { isGlibc, resolvePlatformPackageDirectory } from "../../patcher/index.js"
import type { ProxyInvocation } from "./argv.js"
import { type ExecHost, execProxyCommand, nodeExecHost } from "./exec.js"
import { ProxyError, resolveProxyCommand } from "./resolve.js"

export * from "./argv.js"

// Only the services resolution needs. NodeServices.layer would also build NodeTerminal, which reads process.stdin and
// process.stdout; on pipes that sets O_NONBLOCK, which execve hands to the tool and truncates its output.
const proxyLayer = Layer.mergeAll(NodeFileSystem.layer, NodePath.layer)

// Runs a packaged tool for `effect-tsgo <tool>[@<version>] [args...]`. Resolution runs outside NodeRuntime.runMain so
// that its signal handling does not interfere with the signals forwarded to the tool.
export const runProxy = (invocation: ProxyInvocation, host: ExecHost = nodeExecHost): Promise<void> => {
  const program = Effect.gen(function*() {
    const packageDirectory = yield* resolvePlatformPackageDirectory.pipe(
      Effect.mapError(({ reason }) => new ProxyError({ reason }))
    )
    const oxlintBridge = yield* Effect.try({
      try: () => nodeModule.createRequire(import.meta.url).resolve("@effect/tsgo/oxlint-bridge"),
      catch: () => new ProxyError({ reason: "Unable to resolve @effect/tsgo/oxlint-bridge; reinstall @effect/tsgo." })
    })
    return yield* resolveProxyCommand(invocation, {
      cwd: process.cwd(),
      env: process.env,
      execPath: process.execPath,
      platform: process.platform,
      arch: process.arch,
      isGlibc,
      packageDirectory,
      oxlintBridge
    })
  }).pipe(Effect.provide(proxyLayer))
  return Effect.runPromiseExit(program).then((exit) => {
    if (Exit.isSuccess(exit)) return execProxyCommand(exit.value, host)
    const error = Cause.squash(exit.cause)
    host.reportError(`effect-tsgo ${invocation.tool}: ${error instanceof Error ? error.message : String(error)}`)
    host.parent.exitCode = 1
  })
}
