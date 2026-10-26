export const proxyTools = ["tsc", "tsrs", "oxlint"] as const

export type ProxyTool = typeof proxyTools[number]

export interface ProxyInvocation {
  readonly tool: ProxyTool
  // The version or upstream tag after `<tool>@`, undefined without one.
  readonly version: string | undefined
  // Every argument after the tool, passed to it verbatim.
  readonly args: ReadonlyArray<string>
}

const isProxyTool = (value: string): value is ProxyTool => (proxyTools as ReadonlyArray<string>).includes(value)

// Recognizes `effect-tsgo <tool>[@<version>] [args...]` from the arguments after the script path. Anything else is
// left to the Effect CLI, so the subcommands and `--help`/`--version` keep working.
export const parseProxyInvocation = (argv: ReadonlyArray<string>): ProxyInvocation | undefined => {
  const [first, ...args] = argv
  if (first === undefined) return undefined
  const separator = first.indexOf("@")
  const tool = separator === -1 ? first : first.slice(0, separator)
  if (!isProxyTool(tool)) return undefined
  return { tool, version: separator === -1 ? undefined : first.slice(separator + 1), args }
}
