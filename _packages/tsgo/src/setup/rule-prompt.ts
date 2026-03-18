import * as Arr from "effect/Array"
import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import * as Terminal from "effect/Terminal"
import * as Prompt from "effect/unstable/cli/Prompt"
import type { RuleInfo, RuleSeverity } from "./rule-info.js"
import { cycleSeverity, getSeverityShortName, MAX_SEVERITY_LENGTH } from "./rule-info.js"
import {
  ansi,
  BEEP,
  BG_BLACK_BRIGHT,
  BG_BLUE,
  BG_CYAN,
  BG_RED,
  BG_YELLOW,
  BOLD,
  CURSOR_HIDE,
  CURSOR_LEFT,
  CURSOR_TO_0,
  CYAN_BRIGHT,
  DIM,
  ERASE_LINE,
  GREEN,
  WHITE
} from "../ansi.js"

function eraseLines(count: number): string {
  let result = ""
  for (let i = 0; i < count; i++) {
    if (i > 0) result += "\x1b[1A"
    result += ERASE_LINE
  }
  if (count > 0) result += CURSOR_LEFT
  return result
}

const Action = Data.taggedEnum<Prompt.ActionDefinition>()
const NEWLINE_REGEX = /\r?\n/

function eraseText(text: string, columns: number): string {
  if (columns === 0) {
    return ERASE_LINE + CURSOR_TO_0
  }
  let rows = 0
  const lines = text.split(/\r?\n/)
  for (const line of lines) {
    rows += 1 + Math.floor(Math.max(line.length - 1, 0) / columns)
  }
  return eraseLines(rows)
}

function entriesToDisplay(
  cursor: number,
  total: number,
  maxVisible?: number
): { readonly startIndex: number; readonly endIndex: number } {
  const max = maxVisible === undefined ? total : maxVisible
  let startIndex = Math.min(total - max, cursor - Math.floor(max / 2))
  if (startIndex < 0) {
    startIndex = 0
  }
  const endIndex = Math.min(startIndex + max, total)
  return { startIndex, endIndex }
}

const figuresValue = {
  arrowUp: "\u2191",
  arrowDown: "\u2193",
  tick: "\u2714",
  pointerSmall: "\u203A"
}

type Figures = typeof figuresValue

interface State {
  readonly index: number
  readonly severities: Record<string, RuleSeverity>
}

interface RulePromptOptions {
  readonly message: string
  readonly rules: ReadonlyArray<RuleInfo>
  readonly maxPerPage: number
}

function getSeverityStyle(severity: RuleSeverity): string {
  const styles: Record<RuleSeverity, string> = {
    off: WHITE + BG_BLACK_BRIGHT,
    suggestion: WHITE + BG_CYAN,
    message: WHITE + BG_BLUE,
    warning: WHITE + BG_YELLOW,
    error: WHITE + BG_RED
  }
  return styles[severity]
}

function renderOutput(leadingSymbol: string, trailingSymbol: string, options: RulePromptOptions): string {
  const annotateLine = (line: string): string => ansi(line, BOLD)
  const prefix = leadingSymbol + " "
  return Arr.match(options.message.split(NEWLINE_REGEX), {
    onEmpty: () => `${prefix}${trailingSymbol}`,
    onNonEmpty: (promptLines) => {
      const lines = Arr.map(promptLines, (line) => annotateLine(line))
      return `${prefix}${lines.join("\n  ")} ${trailingSymbol} `
    }
  })
}

function renderRules(state: State, options: RulePromptOptions, figs: Figures, columns: number): string {
  const toDisplay = entriesToDisplay(state.index, options.rules.length, options.maxPerPage)
  const documents: Array<string> = []

  for (let index = toDisplay.startIndex; index < toDisplay.endIndex; index++) {
    const rule = options.rules[index]
    const isHighlighted = state.index === index
    const currentSeverity = state.severities[rule.name] ?? rule.defaultSeverity
    const hasChanged = currentSeverity !== rule.defaultSeverity

    let prefix = " "
    if (index === toDisplay.startIndex && toDisplay.startIndex > 0) {
      prefix = figs.arrowUp
    } else if (index === toDisplay.endIndex - 1 && toDisplay.endIndex < options.rules.length) {
      prefix = figs.arrowDown
    }

    const shortName = getSeverityShortName(currentSeverity)
    const paddedSeverity = shortName.padEnd(MAX_SEVERITY_LENGTH, " ")
    const severityStr = ansi(` ${paddedSeverity} `, getSeverityStyle(currentSeverity))
    const nameText = hasChanged ? `${rule.name}*` : rule.name
    const nameStr = isHighlighted ? ansi(nameText, CYAN_BRIGHT) : nameText
    const mainLine = `${prefix} ${severityStr} ${nameStr}`

    if (isHighlighted && rule.description) {
      const indentWidth = 1 + 1 + (MAX_SEVERITY_LENGTH + 2) + 1
      const indent = " ".repeat(indentWidth)
      const availableWidth = columns - indentWidth
      const truncatedDescription = availableWidth > 0 && rule.description.length > availableWidth
        ? rule.description.substring(0, availableWidth - 1) + "\u2026"
        : rule.description
      documents.push(mainLine + "\n" + ansi(indent + truncatedDescription, DIM))
    } else {
      documents.push(mainLine)
    }
  }

  return documents.join("\n")
}

function renderNextFrame(state: State, options: RulePromptOptions) {
  return Effect.gen(function*() {
    const terminal = yield* Terminal.Terminal
    const columns = yield* terminal.columns
    const rulesStr = renderRules(state, options, figuresValue, columns)
    const promptMsg = renderOutput(ansi("?", CYAN_BRIGHT), figuresValue.pointerSmall, options)
    const helpText = ansi("Use \u2191/\u2193 to navigate, \u2190/\u2192 to change severity, Enter to finish", DIM)
    return CURSOR_HIDE + promptMsg + "\n" + helpText + "\n" + rulesStr
  })
}

function renderSubmission(state: State, options: RulePromptOptions) {
  return Effect.gen(function*() {
    const changedCount = Object.entries(state.severities).filter(([name, severity]) => {
      const rule = options.rules.find((current) => current.name === name)
      return rule && severity !== rule.defaultSeverity
    }).length
    const result = ansi(`${changedCount} rule${changedCount === 1 ? "" : "s"} configured`, WHITE)
    const promptMsg = renderOutput(ansi(figuresValue.tick, GREEN), "", options)
    return promptMsg + " " + result + "\n"
  })
}

function processCursorUp(state: State, totalCount: number) {
  const newIndex = state.index === 0 ? totalCount - 1 : state.index - 1
  return Effect.succeed(Action.NextFrame({ state: { ...state, index: newIndex } }))
}

function processCursorDown(state: State, totalCount: number) {
  const newIndex = (state.index + 1) % totalCount
  return Effect.succeed(Action.NextFrame({ state: { ...state, index: newIndex } }))
}

function processSeverityChange(state: State, options: RulePromptOptions, direction: "left" | "right") {
  const rule = options.rules[state.index]
  const currentSeverity = state.severities[rule.name] ?? rule.defaultSeverity
  const newSeverity = cycleSeverity(currentSeverity, direction)
  return Effect.succeed(Action.NextFrame({
    state: {
      ...state,
      severities: { ...state.severities, [rule.name]: newSeverity }
    }
  }))
}

function handleProcess(options: RulePromptOptions) {
  return (input: Terminal.UserInput, state: State) => {
    const totalCount = options.rules.length
    switch (input.key.name) {
      case "k":
      case "up":
        return processCursorUp(state, totalCount)
      case "j":
      case "down":
        return processCursorDown(state, totalCount)
      case "left":
        return processSeverityChange(state, options, "left")
      case "right":
        return processSeverityChange(state, options, "right")
      case "enter":
      case "return":
        return Effect.succeed(Action.Submit({ value: state.severities }))
      default:
        return Effect.succeed(Action.Beep())
    }
  }
}

function handleClear(options: RulePromptOptions) {
  return Effect.gen(function*() {
    const terminal = yield* Terminal.Terminal
    const columns = yield* terminal.columns
    const visibleCount = Math.min(options.rules.length, options.maxPerPage)
    const text = "\n".repeat(visibleCount + 2) + options.message
    return eraseText(text, columns) + ERASE_LINE + CURSOR_LEFT
  })
}

function handleRender(options: RulePromptOptions) {
  return (
    state: State,
    action: Prompt.Action<State, Record<string, RuleSeverity>>
  ) => Action.$match(action, {
    Beep: () => Effect.succeed(BEEP),
    NextFrame: ({ state }) => renderNextFrame(state, options),
    Submit: () => renderSubmission(state, options)
  })
}

export function createRulePrompt(
  rules: ReadonlyArray<RuleInfo>,
  initialSeverities: Record<string, RuleSeverity>
): Prompt.Prompt<Record<string, RuleSeverity>> {
  const options: RulePromptOptions = {
    message: "Configure Rule Severities",
    rules,
    maxPerPage: 10
  }
  return Prompt.custom(
    { index: 0, severities: initialSeverities },
    {
      render: handleRender(options),
      process: handleProcess(options),
      clear: () => handleClear(options)
    }
  )
}
