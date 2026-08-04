import type { Integration } from "./types.js"

interface Range {
  readonly start: number
  readonly end: number
}

interface CommandSegment extends Range {
  readonly operator?: string
}

interface CommandRange extends Range {
  readonly patchEnd: number
  readonly integrationFlags: ReadonlyArray<Range & { readonly value: string }>
}

const integrationFlagNames = new Set([
  "--typescript",
  "--no-typescript",
  "--oxlint",
  "--no-oxlint"
])

export const getPatchCommand = (integrations: ReadonlyArray<Integration>): string | undefined => {
  const typescript = integrations.includes("typescript")
  const oxlint = integrations.includes("oxlint")
  if (!typescript && !oxlint) return undefined

  return `effect-tsgo patch ${typescript ? "--typescript" : "--no-typescript"} ${
    oxlint ? "--oxlint" : "--no-oxlint"
  }`
}

const splitCommands = (script: string): ReadonlyArray<CommandSegment> => {
  const ranges: Array<CommandSegment> = []
  let start = 0
  let quote: "'" | "\"" | undefined
  let escaped = false
  let nesting = 0

  for (let index = 0; index < script.length; index++) {
    const char = script[index]
    if (escaped) {
      escaped = false
      continue
    }
    if (char === "\\" && quote !== "'") {
      escaped = true
      continue
    }
    if (quote !== undefined) {
      if (char === quote) quote = undefined
      continue
    }
    if (char === "'" || char === "\"") {
      quote = char
      continue
    }
    if (char === "(") {
      nesting++
      continue
    }
    if (char === ")" && nesting > 0) {
      nesting--
      continue
    }
    if (nesting > 0) continue

    const operatorLength = char === ";"
      ? 1
      : (char === "&" || char === "|") && script[index + 1] === char
      ? 2
      : char === "|" || char === "&"
      ? 1
      : 0
    if (operatorLength === 0) continue

    ranges.push({ start, end: index, operator: script.slice(index, index + operatorLength) })
    start = index + operatorLength
    index += operatorLength - 1
  }

  ranges.push({ start, end: script.length })
  return ranges
}

const tokenize = (script: string, range: Range): ReadonlyArray<{ value: string; start: number; end: number }> | undefined => {
  const tokens: Array<{ value: string; start: number; end: number }> = []
  let index = range.start

  while (index < range.end) {
    while (index < range.end && /\s/.test(script[index])) index++
    if (index >= range.end) break

    const start = index
    let value = ""
    let quote: "'" | "\"" | undefined
    let escaped = false

    for (; index < range.end; index++) {
      const char = script[index]
      if (escaped) {
        value += char
        escaped = false
        continue
      }
      if (char === "\\" && quote !== "'") {
        escaped = true
        continue
      }
      if (quote !== undefined) {
        if (char === quote) quote = undefined
        else value += char
        continue
      }
      if (char === "'" || char === "\"") {
        quote = char
        continue
      }
      if (/\s/.test(char)) break
      if ("<>`".includes(char) || (char === "$" && script[index + 1] === "(")) return undefined
      value += char
    }

    if (quote !== undefined || escaped) return undefined
    tokens.push({ value, start, end: index })
  }

  return tokens
}

const findPatchCommand = (script: string, range: Range): CommandRange | undefined => {
  const tokens = tokenize(script, range)
  if (tokens === undefined || tokens.length < 2) return undefined

  let commandIndex = 0
  while (/^[A-Za-z_][A-Za-z0-9_]*=/.test(tokens[commandIndex]?.value ?? "")) commandIndex++

  if (tokens[commandIndex]?.value === "pnpm" && tokens[commandIndex + 1]?.value === "exec") commandIndex += 2
  else if (tokens[commandIndex]?.value === "npm" && tokens[commandIndex + 1]?.value === "exec") {
    commandIndex += tokens[commandIndex + 2]?.value === "--" ? 3 : 2
  } else if (tokens[commandIndex]?.value === "npx") commandIndex++

  if (tokens[commandIndex]?.value !== "effect-tsgo" || tokens[commandIndex + 1]?.value !== "patch") {
    return undefined
  }

  return {
    start: range.start,
    end: range.end,
    patchEnd: tokens[commandIndex + 1].end,
    integrationFlags: tokens.slice(commandIndex + 2)
      .filter((token) => integrationFlagNames.has(token.value))
      .map(({ value, start, end }) => ({ value, start, end }))
  }
}

export const hasPatchCommand = (script: string): boolean =>
  splitCommands(script).some((range) => findPatchCommand(script, range) !== undefined)

export const getPatchIntegrations = (script: string): ReadonlyArray<Integration> => {
  const match = splitCommands(script).flatMap((range) => {
    const command = findPatchCommand(script, range)
    return command === undefined ? [] : [command]
  })[0]
  if (match === undefined) return []

  let typescript = true
  let oxlint = false
  for (const flag of match.integrationFlags) {
    if (flag.value === "--typescript") typescript = true
    else if (flag.value === "--no-typescript") typescript = false
    else if (flag.value === "--oxlint") oxlint = true
    else if (flag.value === "--no-oxlint") oxlint = false
  }
  return [
    ...(typescript ? ["typescript" as const] : []),
    ...(oxlint ? ["oxlint" as const] : [])
  ]
}

export const updatePatchCommand = (
  script: string,
  integrations: ReadonlyArray<Integration>
): { readonly script: string; readonly found: boolean } => {
  const command = getPatchCommand(integrations)
  const ranges = splitCommands(script)
  const matches = ranges.flatMap((range) => {
    const match = findPatchCommand(script, range)
    return match === undefined ? [] : [match]
  })
  if (matches.length === 0) return { script, found: false }

  if (command !== undefined) {
    const flags = command.slice("effect-tsgo patch".length)
    let updated = script
    for (const match of [...matches].reverse()) {
      let suffix = updated.slice(match.patchEnd, match.end)
      for (const flag of [...match.integrationFlags].reverse()) {
        const relativeStart = flag.start - match.patchEnd
        const relativeEnd = flag.end - match.patchEnd
        const whitespaceStart = suffix.slice(0, relativeStart).search(/\s+$/)
        suffix = suffix.slice(0, whitespaceStart < 0 ? relativeStart : whitespaceStart) + suffix.slice(relativeEnd)
      }
      updated = updated.slice(0, match.patchEnd) + flags + suffix + updated.slice(match.end)
    }
    return { script: updated, found: true }
  }

  if (ranges.some((range) => range.operator === "||" || range.operator === "|" || range.operator === "&")) {
    return { script, found: false }
  }

  let updated = script
  for (const match of [...matches].reverse()) {
    const before = updated.slice(0, match.start)
    const after = updated.slice(match.end)
    const previousOperator = /\s*(?:&&|\|\||[;|&])\s*$/.exec(before)
    const nextOperator = /^\s*(?:&&|\|\||[;|&])\s*/.exec(after)
    if (previousOperator !== null && nextOperator?.[0].trim() !== "&&") {
      updated = before.slice(0, previousOperator.index) + after
    } else if (nextOperator !== null) {
      const leadingWhitespace = /^\s*/.exec(updated.slice(match.start, match.end))?.[0] ?? ""
      updated = before + leadingWhitespace + after.slice(nextOperator[0].length)
    } else {
      updated = before + after
    }
  }
  return { script: updated.trim(), found: true }
}
