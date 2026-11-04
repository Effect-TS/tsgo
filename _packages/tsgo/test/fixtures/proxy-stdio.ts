import * as Effect from "effect/Effect"
import { execProxyCommand, type ProxyCommand } from "../../src/cli/proxy/exec.js"
import { proxyLayer } from "../../src/cli/proxy/index.js"

// Does what runProxy does around resolution: builds the proxy's services, then replaces the process with the command.
export const runWithProxyServices = (command: ProxyCommand): Promise<void> =>
  Effect.runPromise(Effect.void.pipe(Effect.provide(proxyLayer))).then(() => execProxyCommand(command))
