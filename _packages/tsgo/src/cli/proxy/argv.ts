export const proxyTools = ["tsc", "tsrs", "oxlint", "vp"] as const

export type ProxyTool = typeof proxyTools[number]

// `--tsrs-lint[=<version|tag>]` in front of the tool: Oxlint runs its type-aware rules on the packaged tsrs
// (`tsrs --lint`) instead of the packaged tsgolint.
export const tsrsLintFlag = "--tsrs-lint"

export interface TsrsLintSelection {
  // The tsrs version or upstream tag after `--tsrs-lint=`, undefined without one.
  readonly version: string | undefined
}

export interface ProxyInvocation {
  readonly tool: ProxyTool
  // The version or upstream tag after `<tool>@`, undefined without one.
  readonly version: string | undefined
  // Set by a `--tsrs-lint[=<version|tag>]` in front of the tool, undefined without it.
  readonly tsrsLint?: TsrsLintSelection | undefined
  // Every argument after the tool, passed to it verbatim.
  readonly args: ReadonlyArray<string>
}

const isProxyTool = (value: string): value is ProxyTool => (proxyTools as ReadonlyArray<string>).includes(value)

const parseTsrsLintFlag = (argument: string): TsrsLintSelection | undefined =>
  argument === tsrsLintFlag
    ? { version: undefined }
    : argument.startsWith(`${tsrsLintFlag}=`)
    ? { version: argument.slice(tsrsLintFlag.length + 1) }
    : undefined

// Recognizes `effect-tsgo [--tsrs-lint[=<version|tag>]] <tool>[@<version>] [args...]` from the arguments after the
// script path. Anything else is left to the Effect CLI, so the subcommands and `--help`/`--version` keep working.
export const parseProxyInvocation = (argv: ReadonlyArray<string>): ProxyInvocation | undefined => {
  const tsrsLint = argv[0] === undefined ? undefined : parseTsrsLintFlag(argv[0])
  const [first, ...args] = tsrsLint === undefined ? argv : argv.slice(1)
  if (first === undefined) return undefined
  const separator = first.indexOf("@")
  const tool = separator === -1 ? first : first.slice(0, separator)
  if (!isProxyTool(tool)) return undefined
  return { tool, version: separator === -1 ? undefined : first.slice(separator + 1), tsrsLint, args }
}
