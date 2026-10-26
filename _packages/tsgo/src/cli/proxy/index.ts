import * as NodeServices from "@effect/platform-node/NodeServices"
import * as Cause from "effect/Cause"
import * as Effect from "effect/Effect"
import * as Exit from "effect/Exit"
import { isGlibc, resolvePlatformPackageDirectory } from "../../patcher/index.js"
import type { ProxyInvocation } from "./argv.js"
import { type ExecHost, execProxyCommand, nodeExecHost } from "./exec.js"
import { ProxyError, resolveProxyCommand } from "./resolve.js"

export * from "./argv.js"

// Runs a packaged tool for `effect-tsgo <tool>[@<version>] [args...]`. Resolution runs outside NodeRuntime.runMain so
// that its signal handling does not interfere with the signals forwarded to the tool.
export const runProxy = (invocation: ProxyInvocation, host: ExecHost = nodeExecHost): Promise<void> => {
  const program = Effect.gen(function*() {
    const packageDirectory = yield* resolvePlatformPackageDirectory.pipe(
      Effect.mapError(({ reason }) => new ProxyError({ reason }))
    )
    return yield* resolveProxyCommand(invocation, {
      cwd: process.cwd(),
      env: process.env,
      execPath: process.execPath,
      platform: process.platform,
      arch: process.arch,
      isGlibc,
      packageDirectory
    })
  }).pipe(Effect.provide(NodeServices.layer))
  return Effect.runPromiseExit(program).then((exit) => {
    if (Exit.isSuccess(exit)) return execProxyCommand(exit.value, host)
    const error = Cause.squash(exit.cause)
    host.reportError(`effect-tsgo ${invocation.tool}: ${error instanceof Error ? error.message : String(error)}`)
    host.parent.exitCode = 1
  })
}
